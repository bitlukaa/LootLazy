use crate::{feed, presence::Presence, storage};
use chrono::Utc;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}
fn stamp() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn clock(s: i64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}
fn alive(state: &str) -> bool {
    ["queued", "starting", "running", "waiting", "ready_next"].contains(&state)
}
pub struct Queue {
    pub items: Vec<Value>,
    pub phase: String,
    pub index: usize,
    pub waiting: Option<i64>,
    wait_deadline: Option<Instant>,
    pub error: Option<String>,
}
impl Queue {
    fn new() -> Self {
        Self {
            items: vec![],
            phase: "idle".into(),
            index: 0,
            waiting: None,
            wait_deadline: None,
            error: None,
        }
    }
    pub fn active(&self) -> bool {
        ["starting", "running", "waiting", "ready_next"].contains(&self.phase.as_str())
    }
    pub fn status(&self) -> Value {
        json!({"items":self.items,"phase":self.phase,"active":self.active(),"index":if self.items.is_empty(){-1}else{self.index as i64},"automatic":true,"error":self.error,"waitUntil":self.waiting,"waitSecondsRemaining":self.wait_deadline.map(|d|((d.saturating_duration_since(Instant::now()).as_millis()+999)/1000) as u64)})
    }
}
pub struct Backend {
    pub root: PathBuf,
    pub catalog: Vec<Value>,
    pub meta: Value,
    pub overrides: Value,
    pub curated: Value,
    pub quests: Value,
    pub closed: Value,
    pub history: Vec<Value>,
    pub queue: Queue,
    pub quest_queue: Queue,
    pub session: Option<Value>,
    child: Option<Child>,
    presence: Presence,
    allow_rpc: bool,
    started_at: Option<Instant>,
}
impl Backend {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("data")).map_err(|e| e.to_string())?;
        let read =
            |name: &str, fallback: Value| storage::read(&root.join("data").join(name), fallback);
        let mut overrides = read("questra_overrides.json", storage::defaults());
        if !overrides.is_object() {
            overrides = storage::defaults();
        }
        for key in ["custom_executables", "settings"] {
            if !overrides[key].is_object() {
                overrides[key] = json!({});
            }
        }
        if !overrides["saved_games"].is_array() {
            overrides["saved_games"] = json!([]);
        }
        let closed = read("active_quests.json.states.json", json!({}));
        let closed = if closed.is_object() {
            closed
        } else {
            json!({})
        };
        let mut queue = Queue::new();
        queue.items = read("queue.json", json!({}))["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| validated_item(item.clone()).ok())
            .take(30)
            .collect();
        Ok(Self {
            catalog: read("detectable_cache.json", json!([]))
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(valid_game)
                .collect(),
            meta: read("catalog_meta.json", json!({})),
            overrides,
            curated: read("questra_catalog.json", json!({"games":{}})),
            quests: read(
                "active_quests.json",
                json!({"quests":[],"last_synced":null}),
            ),
            closed,
            history: read("history.json", json!({}))["sessions"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(Value::is_object)
                .take(50)
                .collect(),
            root,
            queue,
            quest_queue: Queue::new(),
            session: None,
            child: None,
            presence: Presence::new(),
            allow_rpc: true,
            started_at: None,
        })
    }
    fn save(&self, name: &str, data: &Value) -> Result<(), String> {
        storage::save(&self.root.join("data").join(name), data)
    }
    fn save_queue(&self) -> Result<(), String> {
        self.save("queue.json",&json!({"items":self.queue.items.iter().filter(|v|!["completed","cancelled"].contains(&text(v,"state").as_str())).map(|v|{let mut v=v.clone();v.as_object_mut().unwrap().remove("state");v}).collect::<Vec<_>>(),"automatic":true}))
    }
    pub fn resolve(&self, game: &Value) -> Value {
        let app = text(game, "id");
        let custom = self
            .overrides
            .pointer(&format!("/custom_executables/{app}"))
            .and_then(Value::as_str);
        let official: Vec<_> = game["executables"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["os"].is_null() || e["os"] == "win32")
            .filter_map(|e| e["name"].as_str().and_then(|s| storage::executable(s).ok()))
            .collect();
        let (exe, choices, status, reliable) =
            if let Some(custom) = custom.filter(|c| storage::executable(c).is_ok()) {
                (
                    Some(custom.to_string()),
                    vec![custom.to_string()],
                    "custom_override",
                    false,
                )
            } else if !official.is_empty() {
                (
                    Some(official[0].clone()),
                    official,
                    "official_verified",
                    true,
                )
            } else if self.overrides.pointer("/settings/scan_local_steam") != Some(&json!(false)) {
                if let Some(found) = steam_executables(game, &self.overrides) {
                    (
                        found.first().cloned(),
                        found,
                        "local_steam_discovered",
                        true,
                    )
                } else {
                    let choices = self
                        .curated
                        .pointer(&format!("/games/{app}/executables"))
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str().and_then(|v| storage::executable(v).ok()))
                        .collect::<Vec<_>>();
                    let status = if choices.is_empty() {
                        "unresolved"
                    } else {
                        "unverified_catalog"
                    };
                    (None, choices, status, false)
                }
            } else {
                (None, vec![], "unresolved", false)
            };
        json!({"appId":app,"gameName":game["name"],"primaryExecutable":exe,"executables":choices,"status":status,"isReliable":reliable})
    }
    fn game(&self, id: &str) -> Option<Value> {
        self.catalog.iter().find(|g| g["id"] == id).cloned()
    }
    fn enriched(&self, mut game: Value) -> Value {
        game["resolution"] = self.resolve(&game);
        game
    }
    pub fn status(&self) -> Value {
        let mut session = self.session.clone();
        if let Some(s) = session.as_mut() {
            let elapsed = self.elapsed_seconds(s);
            let target = s["targetDurationSeconds"].as_i64();
            let remaining = target.map(|t| (t - elapsed).max(0));
            s["elapsedSeconds"] = json!(elapsed);
            s["elapsedFormatted"] = json!(clock(elapsed));
            s["remainingSeconds"] = json!(remaining);
            s["remainingFormatted"] = json!(remaining.map(clock));
            s["progressPercent"] = json!(target.map(|t| (elapsed * 100 / t).min(100)));
            s["presence"] = self.presence.status();
            s["discordDetected"] = Value::Null;
        }
        json!({"simulation":{"active":session.is_some(),"session":session},"queue":self.queue.status(),"questQueue":self.quest_queue.status()})
    }
    fn elapsed_seconds(&self, session: &Value) -> i64 {
        self.started_at
            .map(|t| t.elapsed().as_secs() as i64)
            .unwrap_or_else(|| {
                ((Utc::now().timestamp_millis() - session["startTime"].as_i64().unwrap_or(0))
                    / 1000)
                    .max(0)
            })
    }
    fn close_quest(&mut self, id: &str, reason: &str) -> Result<(), String> {
        self.closed[id] = json!({"reason":reason,"closedAt":stamp()});
        self.save("active_quests.json.states.json", &self.closed)
    }
    pub fn disable_presence_for_diagnostics(&mut self) {
        self.allow_rpc = false;
    }
    pub fn missing_game_executable(&self, id: &str) -> bool {
        storage::id(id).is_ok()
            && self.resolve(&self.game(id).unwrap_or(json!({"id":id,"name":"Game"})))
                ["primaryExecutable"]
                .as_str()
                .is_none()
    }
    pub fn quest_application(&self, id: &str) -> Option<String> {
        self.available_quest(id)
            .and_then(|q| q["appId"].as_str().and_then(|id| storage::id(id).ok()))
    }
    fn available_quest(&self, id: &str) -> Option<Value> {
        self.quests["quests"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|q| quest_ids(q).iter().any(|v| v == id) && feed::current(q))
            .cloned()
    }
    fn closed_reason(&self, quest: &Value) -> Option<&str> {
        quest_ids(quest)
            .iter()
            .filter_map(|id| self.closed.get(id))
            .filter(|v| v["reason"].as_str().is_some())
            .max_by_key(|v| v["closedAt"].as_str())
            .and_then(|v| v["reason"].as_str())
    }
    fn quest_finished(&self, quest: &Value) -> bool {
        if let Some(reason) = self.closed_reason(quest) {
            return reason != "open";
        }
        let ids = quest_ids(quest);
        self.history
            .iter()
            .any(|s| ids.iter().any(|id| s["questId"] == *id) && s["reason"] == "idle_timeout")
    }
    fn start_process(&mut self, item: &Value) -> Result<(), String> {
        if self.session.is_some() {
            return Err("A game is already running".into());
        }
        let id = storage::id(&text(item, "appId"))?;
        let duration = item["durationMinutes"].as_f64().unwrap_or(0.0);
        if !duration.is_finite() || !(0.0..=1440.0).contains(&duration) {
            return Err("Invalid duration".into());
        }
        let exe = item["executableName"].as_str().filter(|s| !s.is_empty());
        if exe.is_none() && item["activityEnabled"] != false {
            return Err("missing_executable".into());
        }
        let mut child = None;
        let mut destination = None;
        if let Some(exe) = exe {
            let target = storage::target(&self.root.join("games"), &id, exe)?;
            ensure_owned_path(&self.root.join("games"), &target)?;
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            write_helper(&target)?;
            let mut command = Command::new(&target);
            command.args([
                "--title",
                item["gameName"].as_str().unwrap_or("Questro"),
                "--native-window",
            ]);
            command
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            child = Some(command.spawn().map_err(|e| e.to_string())?);
            destination = Some(target.to_string_lossy().to_string());
        }
        let activity = item["activityEnabled"] != false;
        let s = json!({"appId":id,"questId":item["questId"],"gameName":item["gameName"],"executableName":exe,"executablePath":destination,"iconHash":item["iconHash"],"pid":child.as_ref().map(Child::id).unwrap_or(std::process::id()),"startTime":Utc::now().timestamp_millis(),"targetDurationSeconds":if duration>0.0{Some((duration*60.0).round() as i64)}else{None},"mode":if child.is_some(){if activity{"process_and_rpc"}else{"process_only"}}else{"rpc_only"}});
        self.child = child;
        self.session = Some(s.clone());
        self.started_at = Some(Instant::now());
        if activity && self.allow_rpc {
            self.presence.start(s);
        }
        Ok(())
    }
    fn stop_process(&mut self, reason: &str) -> Result<(), String> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                child.kill().map_err(|e| e.to_string())?;
                child.wait().map_err(|e| e.to_string())?;
            }
        }
        self.child = None;
        self.presence.stop();
        if let Some(mut session) = self.session.take() {
            let duration = self.elapsed_seconds(&session);
            self.started_at = None;
            session["durationSeconds"] = json!(duration);
            session["durationFormatted"] = json!(clock(duration));
            session["playedAt"] = json!(stamp());
            session["reason"] = json!(reason);
            let quest = text(&session, "questId");
            self.history.insert(0, session);
            self.history.truncate(50);
            let history_result = self.save("history.json", &json!({"sessions":self.history}));
            if !quest.is_empty() && ["idle_timeout", "quest_cancelled"].contains(&reason) {
                let quest_result = self.close_quest(
                    &quest,
                    if reason == "idle_timeout" {
                        "finished"
                    } else {
                        "cancelled"
                    },
                );
                history_result?;
                quest_result?;
            } else {
                history_result?;
            }
        }
        Ok(())
    }
    fn launch(&mut self, quest: bool) -> Result<(), String> {
        if quest {
            loop {
                let Some(item) = self.quest_queue.items.get(self.quest_queue.index) else {
                    return Err("Queue is empty".into());
                };
                let id = text(item, "questId");
                if self.available_quest(&id).is_some() {
                    break;
                }
                self.quest_queue.items[self.quest_queue.index]["state"] = json!("cancelled");
                let _ = self.close_quest(&id, "cancelled");
                self.quest_queue.index += 1;
                if self.quest_queue.index >= self.quest_queue.items.len() {
                    self.quest_queue.phase = "completed".into();
                    self.quest_queue.waiting = None;
                    self.quest_queue.wait_deadline = None;
                    return Ok(());
                }
            }
        }
        let q = if quest {
            &self.quest_queue
        } else {
            &self.queue
        };
        let item = q.items.get(q.index).ok_or("Queue is empty")?.clone();
        let result = self.start_process(&item);
        let q = if quest {
            &mut self.quest_queue
        } else {
            &mut self.queue
        };
        q.waiting = None;
        q.wait_deadline = None;
        match result {
            Ok(()) => {
                q.phase = "running".into();
                q.items[q.index]["state"] = json!("running");
                Ok(())
            }
            Err(e) => {
                q.phase = "error".into();
                q.items[q.index]["state"] = json!("error");
                q.error = Some("launch_failed".into());
                Err(e)
            }
        }
    }
    fn advance(&mut self, quest: bool) {
        let q = if quest {
            &mut self.quest_queue
        } else {
            &mut self.queue
        };
        let mut next = q.index + 1;
        while next < q.items.len() && q.items[next]["state"] == "cancelled" {
            next += 1;
        }
        if next >= q.items.len() {
            q.phase = "completed".into();
            q.waiting = None;
            q.wait_deadline = None;
            return;
        }
        q.index = next;
        q.phase = "waiting".into();
        q.items[next]["state"] = json!("waiting");
        let seconds = rand::random_range(5..=10);
        q.waiting = Some(Utc::now().timestamp_millis() + seconds * 1000);
        q.wait_deadline = Some(Instant::now() + Duration::from_secs(seconds as u64));
    }
    fn finish_current(&mut self, reason: &str) -> Result<(), String> {
        let quest = self.quest_queue.phase == "running";
        let queued = quest || self.queue.phase == "running";
        let stopped = self.stop_process(reason);
        if self.session.is_some() {
            return stopped;
        }
        if queued {
            if reason == "item_cancelled" && !quest {
                self.queue.items.remove(self.queue.index);
                if self.queue.index == 0 {
                    self.queue.index = usize::MAX;
                } else {
                    self.queue.index -= 1;
                }
            } else {
                let q = if quest {
                    &mut self.quest_queue
                } else {
                    &mut self.queue
                };
                q.items[q.index]["state"] = json!(if reason == "idle_timeout" {
                    "completed"
                } else {
                    "cancelled"
                });
            }
            self.advance_safe(quest);
            if !quest {
                let saved = self.save_queue();
                stopped?;
                saved?;
                return Ok(());
            }
        }
        stopped
    }
    fn advance_safe(&mut self, quest: bool) {
        let q = if quest {
            &mut self.quest_queue
        } else {
            &mut self.queue
        };
        if q.index == usize::MAX {
            q.index = 0;
            if q.items.is_empty() {
                q.phase = "completed".into();
                q.waiting = None;
                q.wait_deadline = None;
            } else {
                q.phase = "waiting".into();
                q.items[0]["state"] = json!("waiting");
                let seconds = rand::random_range(5..=10);
                q.waiting = Some(Utc::now().timestamp_millis() + seconds * 1000);
                q.wait_deadline = Some(Instant::now() + Duration::from_secs(seconds as u64));
            }
            return;
        }
        self.advance(quest);
    }
    pub fn tick(&mut self) {
        if let Some(session) = &self.session {
            let timed = session["targetDurationSeconds"]
                .as_i64()
                .is_some_and(|duration| self.elapsed_seconds(session) >= duration);
            let exited = self
                .child
                .as_mut()
                .is_some_and(|c| matches!(c.try_wait(), Ok(Some(_))));
            if timed {
                let _ = self.finish_current("idle_timeout");
            } else if exited {
                let quest = self.quest_queue.phase == "running";
                let queued = quest || self.queue.phase == "running";
                let _ = self.stop_process("process_exited");
                if queued {
                    let q = if quest {
                        &mut self.quest_queue
                    } else {
                        &mut self.queue
                    };
                    q.phase = "error".into();
                    q.error = Some("process_ended_early".into());
                    q.items[q.index]["state"] = json!("error");
                }
            }
        }
        for quest in [false, true] {
            let q = if quest {
                &self.quest_queue
            } else {
                &self.queue
            };
            if q.phase == "waiting" && q.wait_deadline.is_some_and(|t| t <= Instant::now()) {
                let _ = self.launch(quest);
            }
        }
    }
    fn append(&mut self, item: Value, quest: bool) -> Result<Value, String> {
        let item = validated_item(item)?;
        if (quest && self.queue.active()) || (!quest && self.quest_queue.active()) {
            return Err("Another queue is running".into());
        }
        let q = if quest {
            &mut self.quest_queue
        } else {
            &mut self.queue
        };
        if q.active() {
            if q.items.len() >= 30 {
                return Err("Maximum 30 games".into());
            }
            if !q.items.iter().skip(q.index).any(|v| {
                v["appId"] == item["appId"]
                    && executable_key(&v["executableName"])
                        == executable_key(&item["executableName"])
                    && v["questId"] == item["questId"]
                    && alive(&text(v, "state"))
            }) {
                q.items.push(item);
            }
        } else {
            if self.session.is_some() {
                return Err("Stop the current session first".into());
            }
            q.items = vec![item];
            q.index = 0;
            q.error = None;
            self.launch(quest)?;
        }
        if !quest {
            self.save_queue()?;
        }
        Ok(self.action_status())
    }
    fn action_status(&self) -> Value {
        let status = self.status();
        json!({"success":true,"active":status["simulation"]["active"],"session":status["simulation"]["session"],"queue":status["queue"],"questQueue":status["questQueue"]})
    }
    fn cancel_queue(&mut self, quest: bool) -> Result<(), String> {
        let q = if quest {
            &self.quest_queue
        } else {
            &self.queue
        };
        let ids: Vec<String> = q
            .items
            .iter()
            .filter(|i| alive(&text(i, "state")))
            .map(|i| text(i, "questId"))
            .filter(|id| !id.is_empty())
            .collect();
        let stopped = if q.phase == "running" {
            self.stop_process("queue_cancelled")
        } else {
            Ok(())
        };
        if self.session.is_some()
            && if quest {
                self.quest_queue.phase == "running"
            } else {
                self.queue.phase == "running"
            }
        {
            return stopped;
        }
        let q = if quest {
            &mut self.quest_queue
        } else {
            &mut self.queue
        };
        q.phase = "cancelled".into();
        q.waiting = None;
        q.wait_deadline = None;
        for i in &mut q.items {
            if alive(&text(i, "state")) {
                i["state"] = json!("cancelled");
            }
        }
        let mut saved = Ok(());
        if quest {
            for id in ids {
                let result = self.close_quest(&id, "cancelled");
                if saved.is_ok() {
                    saved = result;
                }
            }
        } else {
            saved = self.save_queue();
        }
        stopped?;
        saved
    }
    pub fn shutdown(&mut self) {
        self.queue.waiting = None;
        self.queue.wait_deadline = None;
        self.quest_queue.waiting = None;
        self.quest_queue.wait_deadline = None;
        let _ = self.stop_process("app_closed");
    }
    pub fn install_catalog(&mut self, data: Value) -> Result<(), String> {
        let list = data.as_array().ok_or("Invalid catalog")?;
        if list.is_empty() || list.iter().any(|g| !valid_game(g)) {
            return Err("Invalid catalog metadata".into());
        }
        self.save("detectable_cache.json", &data)?;
        self.catalog = list.clone();
        self.meta = json!({"lastUpdated":stamp(),"count":self.catalog.len()});
        self.save("catalog_meta.json", &self.meta)
    }
    pub fn install_quests(&mut self, data: Value) -> Result<(), String> {
        let quests = feed::normalize(&data, Utc::now().timestamp_millis())?;
        let value = json!({"source":"community_feed","sourceUrl":feed::SOURCE,"last_synced":stamp(),"quests":quests});
        self.save("active_quests.json", &value)?;
        self.quests = value;
        Ok(())
    }
    pub fn route(&mut self, route: &str, body: Value) -> Result<Value, String> {
        self.tick();
        let parsed =
            url::Url::parse(&format!("http://localhost{route}")).map_err(|e| e.to_string())?;
        let query: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();
        let get = |key: &str| query.get(key).map(String::as_str).unwrap_or("");
        let post = !body.is_null();
        match parsed.path() {
            "/api/status" => Ok(self.status()),
            "/api/queue" => Ok(self.queue.status()),
            "/api/history" => Ok(json!({"history":self.history})),
            "/api/games" => {
                let q = get("q").trim().to_lowercase();
                let limit = get("limit").parse::<usize>().unwrap_or(50).clamp(1, 100);
                let offset = get("offset").parse::<usize>().unwrap_or(0);
                let mut games: Vec<_> = self
                    .catalog
                    .iter()
                    .filter(|g| {
                        q.is_empty()
                            || text(g, "name").to_lowercase().contains(&q)
                            || text(g, "id").contains(&q)
                            || g["aliases"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|a| a.as_str().unwrap_or("").to_lowercase().contains(&q))
                    })
                    .collect();
                if q.is_empty() {
                    let priority = [
                        "363445589247131668",
                        "1158877933042143272",
                        "1402418703554842694",
                        "542075586886107149",
                        "762434991303950386",
                        "1121201675240210523",
                        "356875221078245376",
                        "357008169131606016",
                    ];
                    games.sort_by_key(|g| !priority.contains(&text(g, "id").as_str()));
                }
                let total = games.len();
                Ok(
                    json!({"total":total,"hasMore":offset+limit<total,"games":games.into_iter().skip(offset).take(limit).map(|g|self.enriched(g.clone())).collect::<Vec<_>>()}),
                )
            }
            "/api/catalog-stats" => Ok(
                json!({"totalGames":self.catalog.len(),"emptyExecutablesCount":self.catalog.iter().filter(|g|g["executables"].as_array().is_none_or(|a|a.is_empty())).count(),"solvedCatalogCount":self.curated["games"].as_object().map(|o|o.len()).unwrap_or(0),"userOverridesCount":self.overrides["custom_executables"].as_object().map(|o|o.len()).unwrap_or(0),"lastUpdated":self.meta["lastUpdated"]}),
            ),
            "/api/refresh-catalog" => Ok(
                json!({"success":true,"count":self.catalog.len(),"lastUpdated":self.meta["lastUpdated"]}),
            ),
            "/api/resolve" => Ok(self.resolve(
                &self
                    .game(get("id"))
                    .unwrap_or(json!({"id":get("id"),"name":"Game"})),
            )),
            "/api/settings" => {
                if !post {
                    return Ok(self.overrides["settings"].clone());
                }
                let mut proposed = self.overrides.clone();
                if let Some(b) = body["scan_local_steam"].as_bool() {
                    proposed["settings"]["scan_local_steam"] = json!(b);
                }
                if let Some(paths) = body.get("steam_paths") {
                    let list = paths.as_array().ok_or("Invalid Steam folders")?;
                    if list.len() > 8 {
                        return Err("Maximum eight Steam folders".into());
                    }
                    for p in list {
                        let p = PathBuf::from(p.as_str().ok_or("Invalid folder")?);
                        if !p.is_absolute() || !p.join("steamapps").is_dir() {
                            return Err("Choose a Steam folder containing steamapps".into());
                        }
                    }
                    proposed["settings"]["steam_paths"] = paths.clone();
                }
                self.save("questra_overrides.json", &proposed)?;
                self.overrides = proposed;
                Ok(json!({"success":true}))
            }
            "/api/override" => {
                let mut proposed = self.overrides.clone();
                if body["resetAll"] == true {
                    proposed["custom_executables"] = json!({});
                } else {
                    let app = storage::id(&text(&body, "appId"))?;
                    let exe = storage::executable(&text(&body, "executableName"))?;
                    proposed["custom_executables"][&app] = json!(exe);
                }
                self.save("questra_overrides.json", &proposed)?;
                self.overrides = proposed;
                let game = self
                    .game(&text(&body, "appId"))
                    .unwrap_or(json!({"id":body["appId"]}));
                Ok(json!({"success":true,"resolution":self.resolve(&game)}))
            }
            "/api/play" => {
                if self.quest_queue.active() {
                    return Err("Stop the quest session first".into());
                }
                let item = if body["customProcess"] == true {
                    let exe = storage::executable(&text(&body, "executableName"))?;
                    json!({"appId":"999999999999999999","gameName":Path::new(&exe).file_stem().unwrap().to_string_lossy(),"executableName":exe,"durationMinutes":body["durationMinutes"],"activityEnabled":false})
                } else {
                    let game = self
                        .game(&text(&body, "appId"))
                        .ok_or("Game is not in the catalog")?;
                    let resolution = self.resolve(&game);
                    json!({"appId":game["id"],"gameName":game["name"],"executableName":body["executableName"].as_str().filter(|s|!s.is_empty()).map(json_value).unwrap_or(resolution["primaryExecutable"].clone()),"durationMinutes":body["durationMinutes"],"iconHash":game["icon_hash"]})
                };
                if item["durationMinutes"].as_f64() == Some(0.0) {
                    if self.queue.active() {
                        return Err("Continuous sessions cannot be queued".into());
                    }
                    self.start_process(&item)?;
                    Ok(self.action_status())
                } else {
                    self.append(item, false)
                }
            }
            "/api/stop-current" => {
                if self.quest_queue.active() {
                    return Err("Use the quest card to stop".into());
                }
                self.finish_current("item_cancelled")?;
                Ok(self.action_status())
            }
            "/api/stop" => {
                self.cancel_queue(false)?;
                self.cancel_queue(true)?;
                self.stop_process("user_requested")?;
                Ok(self.action_status())
            }
            "/api/queue/cancel" => {
                self.cancel_queue(false)?;
                Ok(self.action_status())
            }
            "/api/queue/start" => {
                if self.queue.active() || self.quest_queue.active() || self.session.is_some() {
                    return Err("A game is already running".into());
                }
                if self.queue.items.is_empty() {
                    return Err("Queue is empty".into());
                }
                for item in &mut self.queue.items {
                    item["state"] = json!("queued");
                }
                self.queue.index = 0;
                self.queue.error = None;
                self.launch(false)?;
                Ok(self.action_status())
            }
            "/api/queue/items" => {
                let action = text(&body, "action");
                if action == "clear" {
                    if self.queue.active() {
                        return Err("Only upcoming games can be edited".into());
                    }
                    self.queue.items.clear();
                } else {
                    let index = self
                        .queue
                        .items
                        .iter()
                        .position(|i| i["id"] == body["id"])
                        .ok_or("Queue item not found")?;
                    let active = self.queue.active();
                    if active
                        && (index <= self.queue.index
                            || self.queue.items[index]["state"] != "queued")
                    {
                        return Err("Only upcoming games can be edited".into());
                    }
                    match action.as_str() {
                        "remove" => {
                            self.queue.items.remove(index);
                        }
                        "duration" => {
                            let d = body["durationMinutes"].as_f64().ok_or("Invalid duration")?;
                            validate_duration(d)?;
                            self.queue.items[index]["durationMinutes"] = json!(d);
                        }
                        "move" => {
                            let next = if body["direction"] == "up" {
                                index.checked_sub(1)
                            } else if body["direction"] == "down" {
                                Some(index + 1)
                            } else {
                                None
                            }
                            .ok_or("Invalid position")?;
                            if next >= self.queue.items.len() || active && next <= self.queue.index
                            {
                                return Err("Invalid position".into());
                            }
                            self.queue.items.swap(index, next);
                        }
                        _ => return Err("Invalid queue action".into()),
                    }
                }
                self.save_queue()?;
                Ok(self.action_status())
            }
            "/api/quests" => {
                let mut quests:Vec<Value>=self.quests["quests"].as_array().into_iter().flatten().filter(|q|feed::current(q)).map(|q|{let mut q=q.clone();let app=text(&q,"appId");let game=self.game(&app).unwrap_or(json!({"id":app,"name":q["gameName"],"executables":[],"catalogMissing":true}));q["game"]=self.enriched(game);q["localClosedReason"]=json!(self.closed_reason(&q).filter(|r|*r!="open"));q["localFinished"]=json!(self.quest_finished(&q));q}).collect();
                quests.sort_by_key(|q| q["localFinished"].as_bool().unwrap_or(false));
                Ok(
                    json!({"quests":quests,"last_synced":self.quests["last_synced"],"source":self.quests["source"],"sourceUrl":feed::SOURCE}),
                )
            }
            "/api/quests/start" => {
                let id = storage::id(&text(&body, "questId"))?;
                let quest = self
                    .available_quest(&id)
                    .ok_or("Quest is no longer available")?;
                if self.quest_finished(&quest) {
                    return Err("Quest is locally closed".into());
                }
                let game = self.game(&text(&quest, "appId")).unwrap_or(
                    json!({"id":quest["appId"],"name":quest["gameName"],"executables":[]}),
                );
                let resolution = self.resolve(&game);
                if resolution["primaryExecutable"].as_str().is_none() {
                    return Err("missing_executable".into());
                }
                self.append(json!({"appId":game["id"],"gameName":game["name"],"executableName":resolution["primaryExecutable"],"durationMinutes":15.5,"iconHash":game["icon_hash"],"questId":id}),true)
            }
            "/api/quests/cancel" => {
                let id = storage::id(&text(&body, "questId"))?;
                let ids = self
                    .available_quest(&id)
                    .map(|q| quest_ids(&q))
                    .unwrap_or_else(|| vec![id.clone()]);
                if self.available_quest(&id).is_none()
                    && !self.quest_queue.items.iter().any(|i| {
                        ids.iter().any(|id| i["questId"] == *id) && alive(&text(i, "state"))
                    })
                {
                    return Err("Quest is no longer available".into());
                }
                if let Some(index) = self.quest_queue.items.iter().position(|i| {
                    ids.iter().any(|id| i["questId"] == *id) && alive(&text(i, "state"))
                }) {
                    if self.quest_queue.active() && index == self.quest_queue.index {
                        if self.quest_queue.phase == "running" {
                            self.finish_current("quest_cancelled")?;
                        } else {
                            self.quest_queue.items[index]["state"] = json!("cancelled");
                            self.advance(true);
                        }
                    } else {
                        self.quest_queue.items[index]["state"] = json!("cancelled");
                    }
                }
                self.close_quest(&id, "cancelled")?;
                Ok(self.action_status())
            }
            "/api/quests/reopen" => {
                let id = storage::id(&text(&body, "questId"))?;
                let quest = self
                    .available_quest(&id)
                    .ok_or("Quest is no longer available")?;
                let ids = quest_ids(&quest);
                if self.quest_queue.active()
                    && self.quest_queue.items.iter().any(|i| {
                        ids.iter().any(|id| i["questId"] == *id) && alive(&text(i, "state"))
                    })
                {
                    return Err("Quest is already running".into());
                }
                self.close_quest(&id, "open")?;
                Ok(json!({"success":true}))
            }
            _ => Err("Unknown application command".into()),
        }
    }
}
fn executable_key(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("")
        .replace('\\', "/")
        .to_ascii_lowercase()
}
fn quest_ids(quest: &Value) -> Vec<String> {
    let mut ids = quest["variantIds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .chain(quest["id"].as_str())
        .filter_map(|id| storage::id(id).ok())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}
fn json_value(s: &str) -> Value {
    json!(s)
}
fn valid_game(game: &Value) -> bool {
    game.is_object()
        && storage::id(&text(game, "id")).is_ok()
        && game["name"]
            .as_str()
            .is_some_and(|name| !name.trim().is_empty())
}
fn validate_duration(duration: f64) -> Result<(), String> {
    if duration.is_finite() && (1.0..=1440.0).contains(&duration) {
        Ok(())
    } else {
        Err("Duration must be 1–1440 minutes".into())
    }
}
fn validated_item(mut item: Value) -> Result<Value, String> {
    storage::id(&text(&item, "appId"))?;
    if let Some(exe) = item["executableName"].as_str() {
        item["executableName"] = json!(storage::executable(exe)?);
    }
    validate_duration(item["durationMinutes"].as_f64().ok_or("Invalid duration")?)?;
    item["id"] = json!(format!(
        "{}-{}",
        Utc::now().timestamp_millis(),
        rand::random::<u64>()
    ));
    item["state"] = json!("queued");
    Ok(item)
}
fn ensure_owned_path(root: &Path, target: &Path) -> Result<(), String> {
    let mut path = Some(target);
    while let Some(p) = path {
        if p.exists() {
            let m = fs::symlink_metadata(p).map_err(|e| e.to_string())?;
            let mut linked = m.file_type().is_symlink();
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                linked |= m.file_attributes() & 0x400 != 0;
            }
            if linked {
                return Err("Game directory cannot contain links or junctions".into());
            }
        }
        if p == root {
            break;
        }
        path = p.parent();
    }
    Ok(())
}
fn write_helper(target: &Path) -> Result<(), String> {
    use std::io::Write;
    let temporary = target.with_extension(format!("{}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(include_bytes!("../../runner/src-win.exe"))
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temporary, target).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn steam_executables(game: &Value, overrides: &Value) -> Option<Vec<String>> {
    let sku = game["third_party_skus"]
        .as_array()?
        .iter()
        .find(|s| s["distributor"] == "steam")?;
    let id = sku["id"].as_str()?;
    if !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut roots: Vec<PathBuf> = overrides
        .pointer("/settings/steam_paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().map(PathBuf::from))
        .collect();
    for p in [
        "C:/Program Files (x86)/Steam",
        "C:/Program Files/Steam",
        "D:/Steam",
        "D:/SteamLibrary",
        "E:/Steam",
        "E:/SteamLibrary",
    ] {
        roots.push(p.into());
    }
    let initial = roots.clone();
    for root in initial {
        if let Ok(vdf) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in vdf.lines() {
                let parts: Vec<_> = line.split('"').collect();
                if parts.get(1) == Some(&"path") {
                    if let Some(p) = parts.get(3) {
                        roots.push(PathBuf::from(p.replace("\\\\", "\\")));
                    }
                }
            }
        }
    }
    for root in roots {
        let Ok(manifest) = fs::read_to_string(root.join(format!("steamapps/appmanifest_{id}.acf")))
        else {
            continue;
        };
        for line in manifest.lines() {
            let parts: Vec<_> = line.split('"').collect();
            if parts.get(1) != Some(&"installdir") {
                continue;
            }
            let folder = parts.get(3)?;
            if folder.contains(['/', '\\', ':']) || *folder == ".." {
                continue;
            }
            let dir = root.join("steamapps/common").join(folder);
            let mut out = vec![];
            scan_exes(&dir, "", 0, &mut out);
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}
fn scan_exes(root: &Path, prefix: &str, depth: u8, out: &mut Vec<String>) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file()
            && lower.ends_with(".exe")
            && !lower.contains("crash")
            && !lower.contains("unins")
            && !lower.contains("redist")
        {
            if let Ok(exe) = storage::executable(&format!("{prefix}{name}")) {
                out.push(exe);
            }
        } else if kind.is_dir() && ["bin", "win64", "bin64", "binaries"].contains(&lower.as_str()) {
            scan_exes(&entry.path(), &format!("{prefix}{name}/"), depth + 1, out);
        }
    }
}
pub fn fetch_json(url: &str) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    use std::io::Read;
    (&mut response)
        .take(12 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 12 * 1024 * 1024 {
        return Err("Response too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(Backend);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("questro-rust-{}", rand::random::<u64>()));
            let mut b = Backend::new(root).unwrap();
            b.allow_rpc = false;
            Self(b)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.0.shutdown();
            let _ = fs::remove_dir_all(&self.0.root);
        }
    }
    fn item(id: &str, time: f64, quest: bool) -> Value {
        json!({"appId":id,"gameName":"Test","durationMinutes":time,"executableName":null,"activityEnabled":false,"questId":if quest{Some(id)}else{None}})
    }
    #[test]
    fn resolver_preserves_windows_paths_and_never_guesses() {
        let f = Fixture::new();
        let g = json!({"id":"123456","name":"Game","executables":[{"name":"linux-game","os":"linux"},{"name":"win64/game.exe","os":"win32"}]});
        let r = f.0.resolve(&g);
        assert_eq!(r["primaryExecutable"], "win64/game.exe");
        assert_eq!(r["status"], "official_verified");
        assert!(
            f.0.resolve(&json!({"id":"123456","name":"Unknown"}))["primaryExecutable"].is_null()
        );
    }
    #[test]
    fn play_next_preserves_current_and_duration() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 15., false), false).unwrap();
        let first = f.0.session.clone();
        f.0.append(item("234567", 95., false), false).unwrap();
        assert_eq!(f.0.session, first);
        assert_eq!(f.0.queue.items[1]["durationMinutes"], 95.);
        f.0.finish_current("idle_timeout").unwrap();
        let wait = f.0.queue.waiting.unwrap() - Utc::now().timestamp_millis();
        assert!((4900..=10000).contains(&wait));
        assert!(f.0.session.is_none());
        f.0.queue.waiting = Some(0);
        f.0.queue.wait_deadline = Some(Instant::now());
        f.0.tick();
        assert_eq!(f.0.session.as_ref().unwrap()["appId"], "234567");
        assert_eq!(f.0.session.as_ref().unwrap()["targetDurationSeconds"], 5700);
    }
    #[test]
    fn stop_removes_current_and_preserves_next() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 15., false), false).unwrap();
        f.0.append(item("234567", 30., false), false).unwrap();
        f.0.route("/api/stop-current", json!({})).unwrap();
        assert_eq!(f.0.queue.items.len(), 1);
        assert_eq!(f.0.queue.items[0]["appId"], "234567");
        assert_eq!(f.0.queue.phase, "waiting");
        let persisted = storage::read(&f.0.root.join("data/queue.json"), Value::Null);
        assert_eq!(persisted["items"].as_array().unwrap().len(), 1);
        f.0.queue.waiting = Some(0);
        f.0.queue.wait_deadline = Some(Instant::now());
        f.0.tick();
        f.0.route("/api/stop-current", json!({})).unwrap();
        assert!(f.0.queue.items.is_empty());
        assert!(!f.0.queue.active());
    }
    #[test]
    fn upcoming_edits_do_not_change_the_current_game() {
        let mut f = Fixture::new();
        for id in ["123456", "234567", "345678"] {
            f.0.append(item(id, 15., false), false).unwrap();
        }
        let id = f.0.queue.items[2]["id"].clone();
        f.0.route(
            "/api/queue/items",
            json!({"action":"move","id":id,"direction":"up"}),
        )
        .unwrap();
        assert_eq!(f.0.queue.items[1]["appId"], "345678");
        let id = f.0.queue.items[1]["id"].clone();
        f.0.route(
            "/api/queue/items",
            json!({"action":"duration","id":id,"durationMinutes":110}),
        )
        .unwrap();
        assert_eq!(f.0.session.as_ref().unwrap()["appId"], "123456");
        assert!(f
            .0
            .route(
                "/api/queue/items",
                json!({"action":"remove","id":f.0.queue.items[0]["id"]})
            )
            .is_err());
    }
    #[test]
    fn quest_cancel_is_individual_and_reopen_is_persistent() {
        let mut f = Fixture::new();
        f.0.catalog = vec![
            json!({"id":"123456","name":"Test","executables":[{"os":"win32","name":"audit-first.exe"}]}),
            json!({"id":"234567","name":"Test2","executables":[{"os":"win32","name":"audit-second.exe"}]}),
        ];
        let expiry = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        f.0.quests = json!({"quests":[{"id":"123456","appId":"123456","gameName":"Test","kind":"play","durationSeconds":900,"expiresAt":expiry},{"id":"234567","appId":"234567","gameName":"Test2","kind":"play","durationSeconds":900,"expiresAt":expiry}]});
        f.0.save("active_quests.json", &f.0.quests).unwrap();
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/start", json!({"questId":"234567"}))
            .unwrap();
        assert_eq!(f.0.session.as_ref().unwrap()["targetDurationSeconds"], 930);
        f.0.route("/api/quests/cancel", json!({"questId":"234567"}))
            .unwrap();
        assert_eq!(f.0.session.as_ref().unwrap()["questId"], "123456");
        assert_eq!(f.0.closed["234567"]["reason"], "cancelled");
        f.0.route("/api/quests/reopen", json!({"questId":"234567"}))
            .unwrap();
        let reopened = Backend::new(f.0.root.clone()).unwrap();
        assert_eq!(reopened.closed["234567"]["reason"], "open");
    }
    #[test]
    fn timer_runs_without_frontend_polling() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 1., false), false).unwrap();
        f.0.session.as_mut().unwrap()["startTime"] = json!(Utc::now().timestamp_millis() - 61000);
        f.0.started_at = Some(Instant::now() - Duration::from_secs(61));
        f.0.tick();
        assert!(f.0.session.is_none());
        assert_eq!(f.0.history[0]["reason"], "idle_timeout");
    }
    #[test]
    fn cancelled_gap_cannot_restart() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 15., false), false).unwrap();
        f.0.append(item("234567", 15., false), false).unwrap();
        f.0.finish_current("idle_timeout").unwrap();
        f.0.cancel_queue(false).unwrap();
        f.0.tick();
        assert!(f.0.session.is_none());
        assert_eq!(f.0.queue.phase, "cancelled");
    }
    #[test]
    fn corrupt_saved_shapes_are_safe() {
        let f = Fixture::new();
        storage::save(&f.0.root.join("data/questra_overrides.json"), &json!(12)).unwrap();
        storage::save(
            &f.0.root.join("data/active_quests.json.states.json"),
            &json!([]),
        )
        .unwrap();
        storage::save(
            &f.0.root.join("data/detectable_cache.json"),
            &json!([{},"bad",{"id":"123456","name":"Game"}]),
        )
        .unwrap();
        let mut state = Backend::new(f.0.root.clone()).unwrap();
        assert_eq!(state.catalog.len(), 1);
        state
            .route(
                "/api/override",
                json!({"appId":"123456","executableName":"game.exe"}),
            )
            .unwrap();
        assert_eq!(state.overrides["custom_executables"]["123456"], "game.exe");
    }
    #[test]
    fn completed_games_do_not_reappear_in_saved_queue() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 1., false), false).unwrap();
        f.0.append(item("234567", 1., false), false).unwrap();
        f.0.finish_current("idle_timeout").unwrap();
        let restarted = Backend::new(f.0.root.clone()).unwrap();
        assert_eq!(restarted.queue.items.len(), 1);
        assert_eq!(restarted.queue.items[0]["appId"], "234567");
    }
    #[test]
    fn failed_history_save_cannot_leave_a_stopped_queue_running() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 1., false), false).unwrap();
        f.0.append(item("234567", 1., false), false).unwrap();
        fs::create_dir(f.0.root.join("data/history.json")).unwrap();
        assert!(f.0.finish_current("idle_timeout").is_err());
        assert!(f.0.session.is_none());
        assert_eq!(f.0.queue.phase, "waiting");
    }
    #[test]
    fn stop_all_remains_stopped_after_a_write_error() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 1., false), false).unwrap();
        fs::create_dir(f.0.root.join("data/history.json")).unwrap();
        assert!(f.0.cancel_queue(false).is_err());
        assert!(f.0.session.is_none());
        assert!(!f.0.queue.active());
        assert!(f.0.queue.wait_deadline.is_none());
    }
    #[test]
    fn settings_and_names_roll_back_when_saving_fails() {
        let mut f = Fixture::new();
        let before = f.0.overrides.clone();
        fs::create_dir(f.0.root.join("data/questra_overrides.json")).unwrap();
        assert!(f
            .0
            .route(
                "/api/override",
                json!({"appId":"123456","executableName":"game.exe"})
            )
            .is_err());
        assert_eq!(f.0.overrides, before);
        assert!(f
            .0
            .route("/api/settings", json!({"scan_local_steam":false}))
            .is_err());
        assert_eq!(f.0.overrides, before);
    }
    #[test]
    fn system_clock_changes_do_not_end_or_start_games_early() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 1., false), false).unwrap();
        f.0.append(item("234567", 1., false), false).unwrap();
        f.0.session.as_mut().unwrap()["startTime"] = json!(0);
        f.0.tick();
        assert!(f.0.session.is_some());
        f.0.finish_current("idle_timeout").unwrap();
        f.0.queue.waiting = Some(0);
        f.0.tick();
        assert!(f.0.session.is_none());
        assert!(f.0.queue.status()["waitSecondsRemaining"].as_u64().unwrap() <= 10);
    }
    #[test]
    fn finished_history_requires_reopen_and_invalid_quests_are_rejected() {
        let mut f = Fixture::new();
        f.0.catalog = vec![
            json!({"id":"123456","name":"Test","executables":[{"os":"win32","name":"audit-first.exe"}]}),
        ];
        let expiry = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        f.0.quests = json!({"quests":[{"id":"123456","appId":"123456","gameName":"Test","kind":"play","durationSeconds":900,"expiresAt":expiry}]});
        f.0.history = vec![json!({"questId":"123456","reason":"idle_timeout"})];
        assert!(f
            .0
            .route("/api/quests/start", json!({"questId":"123456"}))
            .is_err());
        f.0.route("/api/quests/reopen", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        assert!(f
            .0
            .route("/api/quests/cancel", json!({"questId":"999999"}))
            .is_err());
        assert!(f
            .0
            .route("/api/quests/cancel", json!({"questId":"invalid"}))
            .is_err());
    }
    #[test]
    fn replacing_a_helper_does_not_modify_an_external_hardlink() {
        let f = Fixture::new();
        let outside = f.0.root.join("data/original.bin");
        fs::write(&outside, b"original user file").unwrap();
        let target = f.0.root.join("games/123456/hardlink-test.exe");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::hard_link(&outside, &target).unwrap();
        write_helper(&target).unwrap();
        assert_eq!(fs::read(outside).unwrap(), b"original user file");
        assert_eq!(
            fs::read(target).unwrap(),
            include_bytes!("../../runner/src-win.exe")
        );
    }
}
