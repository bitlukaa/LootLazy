use crate::{artwork, feed, presence::Presence, storage};
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
fn has_discord_artwork(game: &Value) -> bool {
    ["icon_hash", "cover_image_hash"].iter().any(|key| {
        game[*key]
            .as_str()
            .is_some_and(|hash| hash.len() >= 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
    })
}
fn stamp() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn clock(s: i64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}
fn alive(state: &str) -> bool {
    [
        "queued",
        "starting",
        "running",
        "waiting",
        "ready_next",
        "paused",
    ]
    .contains(&state)
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
    executable_resolver: crate::executables::Resolver,
    pub meta: Value,
    pub overrides: Value,
    pub curated: Value,
    executable_metadata: Value,
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
    warmup_deadline: Option<Instant>,
    quest_progress: Value,
    progress_warning: Option<String>,
    progress_saved_at: Instant,
    queue_saved_at: Instant,
    last_tick_at: Instant,
    cache_cleanup_at: Instant,
    pub last_finished: Option<Value>,
    pub discord_mock: Option<bool>,
    pub variety: crate::rotation::Rotation,
    pub steam_idle: crate::rotation::SteamIdle,
    steam_runtime: Option<PathBuf>,
    steam_search: Vec<Value>,
}
impl Backend {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("data")).map_err(|e| e.to_string())?;
        let read =
            |name: &str, fallback: Value| storage::read(&root.join("data").join(name), fallback);
        let mut overrides = if root.join("data/questro_overrides.json").exists() {
            read("questro_overrides.json", storage::defaults())
        } else {
            read("questra_overrides.json", storage::defaults())
        };
        if !overrides.is_object() {
            overrides = storage::defaults();
        }
        for key in ["custom_executables", "settings", "stats"] {
            if !overrides[key].is_object() {
                overrides[key] = json!({});
            }
        }
        if overrides["stats"]["total_playtime_seconds"]
            .as_i64()
            .is_none()
        {
            overrides["stats"]["total_playtime_seconds"] = json!(0);
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
        let mut queued_keys = std::collections::HashSet::new();
        queue.items = read("queue.json", json!({}))["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| validated_item(item.clone()).ok())
            .filter(|item| {
                queued_keys.insert(format!(
                    "{}:{}:{}",
                    text(item, "appId"),
                    executable_key(&item["executableName"]),
                    text(item, "questId")
                ))
            })
            .take(30)
            .collect();
        if !queue.items.is_empty() {
            queue.phase = "paused".into();
        }
        let saved_progress = read("quest_progress.json", json!({}));
        let learned = read("executable_metadata.json", json!({}));
        let mut quest_queue = Queue::new();
        let mut quest_keys = std::collections::HashSet::new();
        quest_queue.items = read("quest_queue.json", json!({}))["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| validated_item(item.clone()).ok())
            .filter(|item| {
                storage::id(&text(item, "questId")).is_ok()
                    && quest_keys.insert(text(item, "questId"))
            })
            .filter(|item| {
                closed
                    .get(text(item, "questId"))
                    .is_none_or(|state| state["reason"].is_null() || state["reason"] == "open")
            })
            .take(30)
            .collect();
        if !quest_queue.items.is_empty() {
            quest_queue.phase = "paused".into();
            quest_queue.items[0]["state"] = json!("paused");
        }
        let catalog = unique_catalog(
            read("detectable_cache.json", json!([]))
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(valid_game)
                .collect(),
        );
        let mut curated: Value =
            serde_json::from_str(include_str!("../../public/executable-candidates.json"))
                .unwrap_or(json!({"games":{}}));
        let local_curated = if root.join("data/questro_catalog.json").exists() {
            read("questro_catalog.json", json!({}))
        } else {
            read("questra_catalog.json", json!({}))
        };
        if let Some(games) = local_curated["games"].as_object() {
            for (id, entry) in games {
                curated["games"][id] = entry.clone();
            }
        }
        let executable_resolver = crate::executables::Resolver::new(&catalog);
        let backend = Self {
            catalog,
            executable_resolver,
            meta: read("catalog_meta.json", json!({})),
            overrides,
            curated,
            executable_metadata: if learned.is_object() {
                learned
            } else {
                json!({})
            },
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
            variety: crate::rotation::Rotation::new(root.join("data/variety_queue.json")),
            steam_idle: crate::rotation::SteamIdle::new(&root),
            steam_runtime: None,
            steam_search: read("steam_search.json", json!([]))
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(400)
                .collect(),
            root,
            queue,
            quest_queue,
            session: None,
            child: None,
            presence: Presence::new(),
            allow_rpc: true,
            started_at: None,
            warmup_deadline: None,
            quest_progress: if saved_progress.is_object() {
                saved_progress
            } else {
                json!({})
            },
            progress_saved_at: Instant::now(),
            queue_saved_at: Instant::now(),
            progress_warning: None,
            last_tick_at: Instant::now(),
            cache_cleanup_at: Instant::now(),
            last_finished: None,
            discord_mock: None,
        };
        backend.prune_quest_cache(&[]);
        Ok(backend)
    }
    fn prune_quest_cache(&self, extra: &[String]) {
        let quests: Vec<&Value> = self.quests["quests"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        let mut previous: Vec<String> = quests
            .iter()
            .filter_map(|q| q["image"].as_str().map(str::to_string))
            .collect();
        previous.extend_from_slice(extra);
        let mut keep: Vec<String> = quests
            .iter()
            .filter(|q| feed::current(q))
            .filter_map(|q| q["image"].as_str().map(str::to_string))
            .collect();
        if let Some(image) = self.session.as_ref().and_then(|s| s["questImage"].as_str()) {
            previous.push(image.to_string());
            keep.push(image.to_string());
        }
        let _ = artwork::prune_quests(&self.root, &previous, &keep);
    }
    fn save(&self, name: &str, data: &Value) -> Result<(), String> {
        storage::save(&self.root.join("data").join(name), data)
    }
    fn save_queue(&self) -> Result<(), String> {
        let items: Vec<Value> = self
            .queue
            .items
            .iter()
            .filter(|item| !["completed", "cancelled"].contains(&text(item, "state").as_str()))
            .map(|item| {
                let mut saved = item.clone();
                if self.queue.phase == "running"
                    && self
                        .queue
                        .items
                        .get(self.queue.index)
                        .map(|current| current["id"] == item["id"])
                        .unwrap_or(false)
                {
                    if let Some(session) = &self.session {
                        saved["resumeElapsedSeconds"] = json!(self.elapsed_seconds(session));
                        saved["resumeCountedSeconds"] = session["resumeBaselineSeconds"].clone();
                    }
                }
                saved["state"] = json!("paused");
                saved
            })
            .collect();
        self.save("queue.json", &json!({"items":items,"automatic":true}))
    }
    fn save_quest_queue(&self) -> Result<(), String> {
        let items: Vec<Value> = self
            .quest_queue
            .items
            .iter()
            .filter(|item| !["completed", "cancelled"].contains(&text(item, "state").as_str()))
            .map(|item| {
                let mut item = item.clone();
                let id = text(&item, "questId");
                let max_clamp = self.quest_target_duration_seconds().saturating_sub(1);
                let elapsed = self.quest_progress[&id]["elapsedSeconds"]
                    .as_i64()
                    .unwrap_or(item["resumeElapsedSeconds"].as_i64().unwrap_or(0))
                    .clamp(0, max_clamp);
                item["resumeElapsedSeconds"] = json!(elapsed);
                item
            })
            .collect();
        self.save(
            "quest_queue.json",
            &json!({"items":items,"savedAt":stamp()}),
        )
        .map_err(|error| {
            crate::diagnostics::record(&self.root, "quest_queue/save", &error);
            format!("save_failed: {error}")
        })
    }
    pub fn resolve(&self, game: &Value) -> Value {
        self.executable_resolver.resolve(
            game,
            &self.curated,
            &self.overrides,
            &self.executable_metadata,
        )
    }
    pub fn metadata_lookup(&self, id: &str) -> Option<String> {
        if !self.allow_rpc {
            return None;
        }
        let game = self.game(id)?;
        let resolution = self.resolve(&game);
        if [
            "official_verified",
            "official_steam_match",
            "custom_override",
            "local_steam_discovered",
        ]
        .contains(&text(&resolution, "status").as_str())
        {
            return None;
        }
        let cached = &self.executable_metadata[id];
        if let Some(checked) = cached["checkedAt"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        {
            let age = Utc::now().timestamp_millis() - checked.timestamp_millis();
            let ttl = if cached["executables"]
                .as_array()
                .is_some_and(|names| !names.is_empty())
            {
                7 * 86400000
            } else {
                86400000
            };
            if age >= 0 && age < ttl {
                return None;
            }
        }
        game["third_party_skus"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|sku| sku["distributor"] == "steam")
            .and_then(|sku| sku["id"].as_str())
            .map(str::to_owned)
    }
    pub fn remember_metadata(&mut self, id: &str, steam_id: &str, names: Vec<String>) {
        self.executable_metadata[id] =
            json!({"steamId":steam_id,"executables":names,"checkedAt":stamp()});
        if self.executable_metadata.as_object().unwrap().len() > 128 {
            let oldest = self
                .executable_metadata
                .as_object()
                .unwrap()
                .iter()
                .filter(|(key, _)| key.as_str() != id)
                .min_by_key(|(_, entry)| entry["checkedAt"].as_str().unwrap_or(""))
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.executable_metadata
                    .as_object_mut()
                    .unwrap()
                    .remove(&oldest);
            }
        }
        if let Err(error) = self.save("executable_metadata.json", &self.executable_metadata) {
            crate::diagnostics::record(&self.root, "executable_metadata/save", &error);
        }
    }
    fn game(&self, id: &str) -> Option<Value> {
        self.catalog.iter().find(|g| g["id"] == id).cloned()
    }
    fn enriched(&self, mut game: Value) -> Value {
        game["resolution"] = self.resolve(&game);
        game
    }
    pub fn quest_target_duration_seconds(&self) -> i64 {
        self.overrides["settings"]["quest_duration_seconds"]
            .as_i64()
            .unwrap_or(900)
            .clamp(60, 3600)
    }
    pub fn total_playtime_seconds(&self) -> i64 {
        let stored = self.overrides["stats"]["total_playtime_seconds"]
            .as_i64()
            .unwrap_or(0);
        let live = self
            .session
            .as_ref()
            .map(|s| {
                (self.elapsed_seconds(s) - s["resumeBaselineSeconds"].as_i64().unwrap_or(0)).max(0)
            })
            .unwrap_or(0);
        stored + live
    }
    pub fn discord_desktop_running(&self) -> bool {
        self.discord_mock
            .unwrap_or_else(crate::presence::is_discord_desktop_running)
    }
    pub fn status(&self) -> Value {
        let mut session = self.session.clone();
        let discord_connected = self.discord_desktop_running();
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
            s["discordDetected"] = json!(discord_connected);
            if let Some(deadline) = self.warmup_deadline {
                if Instant::now() < deadline {
                    let rem = (deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis() as f64
                        / 1000.0)
                        .ceil() as i64;
                    s["timerWarmup"] = json!(true);
                    s["warmupSeconds"] = json!(rem.max(1));
                    s["timerWaiting"] = json!(false);
                }
            }
        }
        json!({"discordConnected":discord_connected,"steamRunning":crate::steam_client::running(),"steamConnected":self.steam_idle.items.iter().any(|item|item["state"]=="running"),"simulation":{"active":session.is_some(),"session":session},"queue":self.queue.status(),"questQueue":self.quest_queue.status(),"storageWarning":self.progress_warning,"lastFinished":self.last_finished,"totalPlaytimeSeconds":self.total_playtime_seconds(),"steamPlaytimeSeconds":self.steam_idle.total_seconds(),"variety":self.variety.status(),"steamIdle":{"rotation":self.steam_idle.status(),"runtimeReady":self.steam_runtime.is_some()}})
    }
    fn elapsed_seconds(&self, session: &Value) -> i64 {
        session["elapsedBeforeSegment"].as_i64().unwrap_or(0)
            + self
                .started_at
                .map(|t| t.elapsed().as_secs() as i64)
                .unwrap_or(0)
    }
    fn resumed_seconds(&self, quest: &Value) -> i64 {
        let ids = quest_ids(quest);
        ids.iter()
            .filter_map(|id| {
                self.quest_progress
                    .get(id)
                    .and_then(|p| p["elapsedSeconds"].as_i64())
            })
            .chain(
                self.quest_queue
                    .items
                    .iter()
                    .filter(|item| {
                        ids.contains(&text(item, "questId"))
                            && !["completed", "cancelled"].contains(&text(item, "state").as_str())
                    })
                    .filter_map(|item| item["resumeElapsedSeconds"].as_i64()),
            )
            .max()
            .unwrap_or(0)
            .clamp(0, self.quest_target_duration_seconds().saturating_sub(1))
    }
    fn checkpoint_quest(&mut self) -> Result<(), String> {
        if let Some(session) = &self.session {
            if let Some(id) = session["questId"].as_str() {
                let elapsed = self
                    .elapsed_seconds(session)
                    .clamp(0, self.quest_target_duration_seconds().saturating_sub(1));
                self.quest_progress[id] = json!({"elapsedSeconds":elapsed,"savedAt":stamp()});
                if let Err(error) = self.save("quest_progress.json", &self.quest_progress) {
                    self.progress_warning = Some("save_failed".into());
                    crate::diagnostics::record(&self.root, "quest_progress/save", &error);
                    return Err(format!("save_failed: {error}"));
                }
                self.progress_warning = None;
                self.progress_saved_at = Instant::now();
            }
        }
        Ok(())
    }
    fn close_quest(&mut self, id: &str, reason: &str) -> Result<(), String> {
        let ids = self
            .available_quest(id)
            .map(|q| quest_ids(&q))
            .unwrap_or_else(|| vec![id.to_string()]);
        for id in &ids {
            self.quest_progress.as_object_mut().unwrap().remove(id);
        }
        for item in &mut self.quest_queue.items {
            if ids.contains(&text(item, "questId")) {
                item["resumeElapsedSeconds"] = json!(0);
            }
        }
        self.save("quest_progress.json", &self.quest_progress)?;
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
    pub fn active_steam_library(&self) -> PathBuf {
        self.root.join("games")
    }
    fn start_process(&mut self, item: &Value) -> Result<(), String> {
        if item["syncSteam"] != true {
            return self.start_game_process(item);
        }
        let game = self
            .game(&text(item, "appId"))
            .ok_or("steam_sync_unavailable")?;
        let steam_id = game["third_party_skus"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|sku| {
                sku["distributor"] == "steam"
                    && sku["id"].as_str().is_some_and(|id| {
                        !id.is_empty() && id.len() <= 10 && id.bytes().all(|b| b.is_ascii_digit())
                    })
            })
            .and_then(|sku| sku["id"].as_str())
            .ok_or("steam_sync_unavailable")?
            .to_string();
        if self.steam_idle.items.iter().any(|entry| {
            entry["steamId"] == steam_id
                && ["starting", "running"].contains(&text(entry, "state").as_str())
        }) {
            return Err("steam_sync_already_running".into());
        }
        self.refresh_steam_runtime();
        let dll = self.steam_runtime.clone().ok_or("steam_runtime_missing")?;
        crate::steam_client::ensure_open()?;
        self.start_game_process(item)?;
        let mut steam_game = game;
        steam_game["steamId"] = json!(steam_id);
        steam_game["id"] = json!(steam_id);
        if let Err(error) = self.steam_idle.start(&self.root, &dll, &steam_game, 0) {
            let _ = self.stop_process("sync_start_failed");
            return Err(error);
        }
        if let Some(session) = self.session.as_mut() {
            session["pairedSteamId"] = json!(steam_id);
            session["timerWaiting"] = json!(true);
        }
        self.started_at = None;
        Ok(())
    }
    fn start_game_process(&mut self, item: &Value) -> Result<(), String> {
        if self.session.is_some() {
            return Err("A game is already running".into());
        }
        let id = storage::id(&text(item, "appId"))?;
        let duration = item["durationMinutes"].as_f64().unwrap_or(0.0);
        if !duration.is_finite() || !(0.0..=1440.0).contains(&duration) {
            return Err("Invalid duration".into());
        }
        let auto_rpc = self.overrides["settings"]["auto_discord_rpc"]
            .as_bool()
            .unwrap_or(true);
        if self.allow_rpc && auto_rpc {
            crate::presence::open_desktop_if_missing()?;
        }
        let exe_str = item["executableName"].as_str().unwrap_or("");
        let fallback = format!(
            "{}.exe",
            item["gameName"]
                .as_str()
                .unwrap_or("game")
                .replace(['/', '\\', ':'], "")
        );
        let exe = if exe_str.is_empty() {
            &fallback
        } else {
            exe_str
        };
        let clean_game = item["gameName"]
            .as_str()
            .unwrap_or("Game")
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "");
        let games_root = self.root.join("games");
        let game_dir = games_root.join(&clean_game);
        let rel_exe = storage::executable(exe)?;
        let target = game_dir.join(&rel_exe);
        ensure_owned_path(&games_root, &target)?;
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        write_helper(&target)?;
        let steam_app_id = self.game(&id).and_then(|g| {
            g["third_party_skus"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|sku| sku["distributor"] == "steam")
                .and_then(|sku| sku["id"].as_str())
                .map(str::to_owned)
        });
        if let Some(ref aid) = steam_app_id {
            if let Some(parent) = target.parent() {
                let _ = fs::write(parent.join("steam_appid.txt"), aid);
            }
            let _ = fs::write(game_dir.join("steam_appid.txt"), aid);
        }
        let mut command = Command::new(&target);
        command.args([
            "--title",
            item["gameName"].as_str().unwrap_or("Questro"),
            "--native-window",
            "--parent",
            &std::process::id().to_string(),
        ]);
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let child = Some(
            command
                .spawn()
                .map_err(|e| format!("process_launch_failed: {e}"))?,
        );
        let destination = Some(target.to_string_lossy().to_string());
        let activity = item["activityEnabled"] != false && auto_rpc;
        let mut s = json!({"appId":id,"questId":item["questId"],"questImage":item["questImage"],"gameName":item["gameName"],"executableName":exe,"executablePath":destination,"iconHash":item["iconHash"],"pid":child.as_ref().map(Child::id).unwrap_or(std::process::id()),"startTime":Utc::now().timestamp_millis(),"targetDurationSeconds":if duration>0.0{Some((duration*60.0).round() as i64)}else{None},"mode":if child.is_some(){if activity{"process_and_rpc"}else{"process_only"}}else{"rpc_only"}});
        let resume = item["resumeElapsedSeconds"].as_i64().unwrap_or(0).max(0);
        let resume = if duration > 0.0 {
            resume.min(((duration * 60.0).round() as i64).saturating_sub(1))
        } else {
            resume
        };
        s["elapsedBeforeSegment"] = json!(resume);
        s["resumeBaselineSeconds"] = json!(item["resumeCountedSeconds"]
            .as_i64()
            .unwrap_or(resume)
            .clamp(0, resume));
        s["timerWaiting"] = json!(self.allow_rpc && auto_rpc);
        self.child = child;
        self.session = Some(s.clone());
        self.started_at = if self.allow_rpc && auto_rpc {
            None
        } else {
            Some(Instant::now())
        };
        self.warmup_deadline = None;
        let _ = self.checkpoint_quest();
        if activity && self.allow_rpc && auto_rpc {
            self.presence.start(s);
        }
        Ok(())
    }
    fn stop_process(&mut self, reason: &str) -> Result<(), String> {
        let _ = self.checkpoint_quest();
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                child.kill().map_err(|e| e.to_string())?;
                child.wait().map_err(|e| e.to_string())?;
            }
        }
        self.child = None;
        self.presence.stop();
        if let Some(mut session) = self.session.take() {
            if let Some(id) = session["pairedSteamId"].as_str() {
                let _ = self.steam_idle.pause(Some(id));
            }
            let image = session["questImage"].as_str().map(str::to_string);
            self.prune_quest_cache(&image.into_iter().collect::<Vec<_>>());
            let duration = self.elapsed_seconds(&session);
            if duration > 0 {
                let cur = self.overrides["stats"]["total_playtime_seconds"]
                    .as_i64()
                    .unwrap_or(0);
                self.overrides["stats"]["total_playtime_seconds"] = json!(
                    cur + (duration - session["resumeBaselineSeconds"].as_i64().unwrap_or(0))
                        .max(0)
                );
                let _ = self.save("questro_overrides.json", &self.overrides);
            }
            self.started_at = None;
            self.warmup_deadline = None;
            session["durationSeconds"] = json!(duration);
            session["durationFormatted"] = json!(clock(duration));
            session["playedAt"] = json!(stamp());
            session["reason"] = json!(reason);
            let quest = text(&session, "questId");
            if ["idle_timeout", "variety_completed"].contains(&reason) {
                self.last_finished = Some(json!({
                    "gameName": session["gameName"],
                    "questId": quest,
                    "reason": reason,
                    "timestamp": stamp()
                }));
            }
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
                q.error = Some(crate::diagnostics::code(&e).into());
                crate::diagnostics::record(&self.root, "game/launch", &e);
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
    fn tick_inner(&mut self) {
        let gap = self.last_tick_at.elapsed();
        self.last_tick_at = Instant::now();
        if gap > Duration::from_secs(15)
            && self.quest_queue.phase == "running"
            && self.started_at.is_some()
        {
            if let Some(session) = &self.session {
                let elapsed = self
                    .elapsed_seconds(session)
                    .saturating_sub(gap.as_secs() as i64);
                self.started_at = None;
                if let Some(session) = self.session.as_mut() {
                    session["elapsedBeforeSegment"] = json!(elapsed.max(0));
                }
                let id = self
                    .session
                    .as_ref()
                    .map(|s| text(s, "questId"))
                    .unwrap_or_default();
                let _ = self.pause_quest(&id);
                return;
            }
        }
        if self.session.is_some() {
            let presence = self.presence.status();
            let rpc_only = self
                .session
                .as_ref()
                .is_some_and(|s| s["mode"] == "rpc_only");
            let discord_running = self.discord_desktop_running();
            let ready = !self.allow_rpc
                || !self.overrides["settings"]["auto_discord_rpc"]
                    .as_bool()
                    .unwrap_or(true)
                || (discord_running
                    && (if rpc_only {
                        presence["activityConfirmed"] == true
                            || presence["desktopReady"] == true
                            || presence["state"] == "ready"
                    } else {
                        true
                    }));
            let paired_id = self
                .session
                .as_ref()
                .and_then(|session| session["pairedSteamId"].as_str())
                .map(str::to_string);
            let steam_ready = if let Some(id) = &paired_id {
                let paired = self
                    .steam_idle
                    .items
                    .iter()
                    .find(|entry| entry["steamId"] == *id);
                if paired.is_none()
                    || paired.is_some_and(|entry| {
                        ["error", "paused", "completed"].contains(&text(entry, "state").as_str())
                    })
                {
                    let error = paired
                        .map(|entry| text(entry, "error"))
                        .filter(|error| !error.is_empty())
                        .unwrap_or("steam_connection_lost".into());
                    if self.queue.active() {
                        let _ = self.pause_game_queue();
                        self.queue.error = Some(error);
                    } else {
                        let _ = self.stop_process("sync_connection_lost");
                        self.queue.error = Some(error);
                    }
                    self.queue.phase = "error".into();
                    return;
                }
                paired.is_some_and(|entry| entry["state"] == "running")
            } else {
                true
            };
            let ready = ready && steam_ready;
            let is_quest = self
                .session
                .as_ref()
                .is_some_and(|s| s["questId"].as_str().is_some());
            if ready && self.started_at.is_none() {
                if is_quest {
                    match self.warmup_deadline {
                        None => {
                            self.warmup_deadline = Some(Instant::now() + Duration::from_secs(10));
                            if let Some(s) = self.session.as_mut() {
                                s["timerWaiting"] = json!(false);
                                s["timerWarmup"] = json!(true);
                                s["warmupSeconds"] = json!(10);
                            }
                        }
                        Some(deadline) if Instant::now() < deadline => {
                            let rem = (deadline
                                .saturating_duration_since(Instant::now())
                                .as_millis() as f64
                                / 1000.0)
                                .ceil() as i64;
                            if let Some(s) = self.session.as_mut() {
                                s["timerWaiting"] = json!(false);
                                s["timerWarmup"] = json!(true);
                                s["warmupSeconds"] = json!(rem.max(1));
                            }
                        }
                        Some(_) => {
                            self.warmup_deadline = None;
                            self.started_at = Some(Instant::now());
                            if let Some(s) = self.session.as_mut() {
                                s["timerWaiting"] = json!(false);
                                s["timerWarmup"] = json!(false);
                                s["warmupSeconds"] = json!(0);
                            }
                        }
                    }
                } else {
                    self.started_at = Some(Instant::now());
                    if let Some(s) = self.session.as_mut() {
                        s["timerWaiting"] = json!(false);
                        s["timerWarmup"] = json!(false);
                        s["warmupSeconds"] = json!(0);
                    }
                }
            } else if !ready {
                self.warmup_deadline = None;
                if self.started_at.is_some() {
                    let elapsed = self.elapsed_seconds(self.session.as_ref().unwrap());
                    self.started_at = None;
                    if let Some(s) = self.session.as_mut() {
                        s["elapsedBeforeSegment"] = json!(elapsed);
                        s["timerWaiting"] = json!(true);
                        s["timerWarmup"] = json!(false);
                        s["warmupSeconds"] = json!(0);
                    }
                    if is_quest {
                        let _ = self.checkpoint_quest();
                    }
                } else if let Some(s) = self.session.as_mut() {
                    s["timerWaiting"] = json!(true);
                    s["timerWarmup"] = json!(false);
                    s["warmupSeconds"] = json!(0);
                }
            } else if self.started_at.is_some() {
                if let Some(s) = self.session.as_mut() {
                    s["timerWaiting"] = json!(false);
                    s["timerWarmup"] = json!(false);
                    s["warmupSeconds"] = json!(0);
                }
            }
            if self
                .session
                .as_ref()
                .is_some_and(|s| s["questId"].as_str().is_some())
                && self.progress_saved_at.elapsed() >= Duration::from_secs(5)
            {
                let _ = self.checkpoint_quest();
            }
        }
        if self.cache_cleanup_at.elapsed() >= Duration::from_secs(60) {
            self.prune_quest_cache(&[]);
            self.cache_cleanup_at = Instant::now();
        }
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
    pub fn tick(&mut self) {
        let before = (
            self.quest_queue.phase.clone(),
            self.quest_queue.index,
            self.quest_queue.items.len(),
        );
        self.tick_inner();
        if self.queue.active() && self.queue_saved_at.elapsed() >= Duration::from_secs(5) {
            let _ = self.save_queue();
            self.queue_saved_at = Instant::now();
        }
        self.tick_variety();
        self.steam_idle.tick();
        if before
            != (
                self.quest_queue.phase.clone(),
                self.quest_queue.index,
                self.quest_queue.items.len(),
            )
        {
            let _ = self.save_quest_queue();
        }
        if self.session.is_some() {
            let presence = self.presence.status();
            if ["unavailable", "rejected"].contains(&presence["state"].as_str().unwrap_or("")) {
                crate::diagnostics::record(
                    &self.root,
                    "discord/activity",
                    &format!("{} {}", text(&presence, "state"), presence["errorCode"]),
                );
            }
        }
    }
    fn pause_quest(&mut self, id: &str) -> Result<Value, String> {
        if self.quest_queue.phase != "running"
            || self.session.as_ref().is_none_or(|s| s["questId"] != id)
        {
            return Err("Quest is not running".into());
        }
        let stopped = self.stop_process("quest_paused");
        self.quest_queue.phase = "paused".into();
        self.quest_queue.waiting = None;
        self.quest_queue.wait_deadline = None;
        self.quest_queue.items[self.quest_queue.index]["state"] = json!("paused");
        self.quest_queue.error = None;
        let saved = self.save_quest_queue();
        stopped?;
        saved?;
        Ok(self.action_status())
    }
    fn resume_quest_queue(&mut self) -> Result<Value, String> {
        if self.session.is_some() || self.queue.active() || self.quest_queue.active() {
            return Err("A game is already running".into());
        }
        let allowed: std::collections::HashSet<String> = self.quests["quests"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|q| feed::current(q) && !self.quest_finished(q))
            .flat_map(quest_ids)
            .collect();
        self.quest_queue.items.retain(|item| {
            allowed.contains(&text(item, "questId"))
                && !["completed", "cancelled"].contains(&text(item, "state").as_str())
        });
        if self.quest_queue.items.is_empty() {
            return Err("Saved quests are no longer available".into());
        }
        for index in 0..self.quest_queue.items.len() {
            let quest = self
                .available_quest(&text(&self.quest_queue.items[index], "questId"))
                .ok_or("Quest is no longer available")?;
            let app = text(&quest, "appId");
            let game = self
                .game(&app)
                .unwrap_or(json!({"id":app,"name":quest["gameName"],"executables":[]}));
            let resolution = self.resolve(&game);
            let elapsed = self.resumed_seconds(&quest);
            let target_duration = (self.quest_target_duration_seconds() as f64) / 60.0;
            let item = &mut self.quest_queue.items[index];
            item["questId"] = quest["id"].clone();
            item["appId"] = game["id"].clone();
            item["gameName"] = game["name"].clone();
            item["executableName"] = resolution["primaryExecutable"].clone();
            item["durationMinutes"] = json!(target_duration);
            item["resumeElapsedSeconds"] = json!(elapsed);
            item["state"] = json!("queued");
        }
        self.quest_queue.index = 0;
        self.quest_queue.error = None;
        self.launch(true)?;
        Ok(self.action_status())
    }
    fn append(&mut self, item: Value, quest: bool) -> Result<Value, String> {
        let item = validated_item(item)?;
        if (quest && self.queue.active()) || (!quest && self.quest_queue.active()) {
            return Err("Another queue is running".into());
        }
        if quest && !self.quest_queue.active() {
            if self.session.is_some() {
                return Err("Stop the current session first".into());
            }
            let ids = self
                .available_quest(&text(&item, "questId"))
                .map(|q| quest_ids(&q))
                .unwrap_or_else(|| vec![text(&item, "questId")]);
            self.quest_queue.items.retain(|old| {
                !["completed", "cancelled"].contains(&text(old, "state").as_str())
                    && !ids.contains(&text(old, "questId"))
            });
            if self.quest_queue.items.len() >= 30 {
                return Err("Maximum 30 games".into());
            }
            self.quest_queue.items.insert(0, item);
            self.quest_queue.index = 0;
            self.quest_queue.error = None;
            self.launch(true)?;
            return Ok(self.action_status());
        }
        let q = if quest {
            &mut self.quest_queue
        } else {
            &mut self.queue
        };
        if q.active() || (!quest && q.phase == "paused" && !q.items.is_empty()) {
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
    fn start_variety(&mut self) -> Result<(), String> {
        if self.session.is_some() || self.queue.active() || self.quest_queue.active() {
            return Err("A game is already running".into());
        }
        let mut item = self.variety.prepare()?;
        if let Some(game) = self.game(&text(&item, "appId")) {
            item["executableName"] = self.resolve(&game)["primaryExecutable"].clone();
        } else {
            self.variety.fail("Game is not in the catalog");
            return Err("Game is not in the catalog".into());
        }
        item["durationMinutes"] = json!(0);
        match self.start_process(&item) {
            Ok(()) => {
                if let Some(s) = self.session.as_mut() {
                    s["varietySession"] = json!(true);
                }
                self.variety.running();
                Ok(())
            }
            Err(e) => {
                self.variety.fail(&e);
                Err(e)
            }
        }
    }
    fn tick_variety(&mut self) {
        if self.variety.suspended() {
            if let Some(session) = self.session.as_mut() {
                session["elapsedBeforeSegment"] = json!(self.variety.elapsed());
            }
            self.started_at = None;
            let _ = self.stop_process("variety_suspended");
            return;
        }
        if self.variety.phase() == "running" {
            if !self
                .session
                .as_ref()
                .is_some_and(|s| s["varietySession"] == true)
            {
                self.variety.fail("process_ended_early");
                return;
            }
            if self.variety.remaining() == 0 {
                if let Err(e) = self.stop_process("variety_completed") {
                    self.variety.fail(&e);
                    return;
                }
                match self.variety.complete() {
                    Ok(true) => {
                        if let Err(e) = self.start_variety() {
                            self.variety.fail(&e);
                        }
                    }
                    Ok(false) => {}
                    Err(e) => self.variety.fail(&e),
                }
            }
        }
        self.variety.checkpoint();
        if self.variety.phase() == "error"
            && self
                .session
                .as_ref()
                .is_some_and(|s| s["varietySession"] == true)
        {
            let _ = self.stop_process("variety_error");
        }
    }
    fn shuffle_variety(&mut self, body: &Value) -> Result<Value, String> {
        use rand::seq::SliceRandom;
        let count = body["count"].as_u64().unwrap_or(100) as usize;
        if !(1..=crate::rotation::MAX_ITEMS).contains(&count) {
            return Err("Choose between 1 and 500 games".into());
        }
        let duration = crate::rotation::minutes(body["durationMinutes"].as_u64().unwrap_or(10))?;
        if self.variety.active() {
            return Err("A game is already running".into());
        }
        let previous: std::collections::HashSet<_> = self.variety.data["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|i| i["id"].as_str())
            .collect();
        let mut seen = std::collections::HashSet::new();
        let all: Vec<_> = self
            .catalog
            .iter()
            .filter(|g| {
                g["type"].as_i64().unwrap_or(1) == 1
                    && has_discord_artwork(g)
                    && storage::id(&text(g, "id")).is_ok()
                    && seen.insert(text(g, "id"))
            })
            .collect();
        let mut pool: Vec<_> = all
            .iter()
            .filter(|g| !previous.contains(g["id"].as_str().unwrap_or("")))
            .copied()
            .collect();
        if pool.len() < count {
            pool = all;
        }
        pool.shuffle(&mut rand::rng());
        let pool: Vec<Value> = pool
            .into_iter()
            .take(crate::rotation::MAX_ITEMS)
            .map(|g| crate::rotation::item(g, duration))
            .collect();
        let items = pool.iter().take(count).cloned().collect();
        self.variety.replace(items)?;
        self.variety.data["pool"] = json!(pool);
        self.variety.data["defaultMinutes"] = json!(duration);
        self.variety.save()?;
        Ok(self.variety.status())
    }
    fn refresh_variety_cards(&mut self) -> Result<(), String> {
        if self.variety.active() {
            return Ok(());
        }
        let count = self.variety.data["items"].as_array().unwrap().len();
        if count == 0 {
            return Ok(());
        }
        let pictures: std::collections::HashMap<String, (Value, Value)> = self
            .catalog
            .iter()
            .map(|g| {
                (
                    text(g, "id"),
                    (g["icon_hash"].clone(), g["cover_image_hash"].clone()),
                )
            })
            .collect();
        for field in ["items", "pool"] {
            if let Some(items) = self.variety.data[field].as_array_mut() {
                for item in items {
                    if let Some((icon, cover)) = pictures.get(&text(item, "appId")) {
                        item["icon_hash"] = icon.clone();
                        item["cover_image_hash"] = cover.clone();
                    }
                }
            }
        }
        let minutes = self.variety.data["defaultMinutes"].as_u64().unwrap_or(10);
        for field in ["items", "pool"] {
            if let Some(items) = self.variety.data[field].as_array_mut() {
                for item in items {
                    item["durationMinutes"] = json!(minutes);
                }
            }
        }
        self.resize_variety(&json!({"count":count}))
    }
    fn sync_variety_pool(&mut self) {
        let items = self.variety.data["items"].as_array().unwrap().clone();
        if let Some(pool) = self.variety.data["pool"].as_array_mut() {
            for item in items {
                if let Some(saved) = pool.iter_mut().find(|g| g["id"] == item["id"]) {
                    *saved = item;
                }
            }
        }
    }
    fn resize_variety(&mut self, body: &Value) -> Result<(), String> {
        let count = body["count"].as_u64().unwrap_or(0) as usize;
        if !(1..=crate::rotation::MAX_ITEMS).contains(&count) {
            return Err("Choose between 1 and 500 games".into());
        }
        if self.variety.active() {
            return Err("A game is already running".into());
        }
        self.variety.data["items"]
            .as_array_mut()
            .unwrap()
            .retain(has_discord_artwork);
        self.sync_variety_pool();
        let mut pool = self.variety.data["pool"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| self.variety.data["items"].as_array().unwrap().clone());
        pool.retain(has_discord_artwork);
        let duration = self.variety.data["defaultMinutes"].as_u64().unwrap_or(10);
        let mut seen: std::collections::HashSet<String> =
            pool.iter().map(|i| text(i, "id")).collect();
        if pool.len() < count {
            use rand::seq::SliceRandom;
            let mut more: Vec<_> = self
                .catalog
                .iter()
                .filter(|g| {
                    g["type"].as_i64().unwrap_or(1) == 1
                        && storage::id(&text(g, "id")).is_ok()
                        && seen.insert(text(g, "id"))
                })
                .collect();
            more.shuffle(&mut rand::rng());
            pool.extend(
                more.into_iter()
                    .take(count - pool.len())
                    .map(|g| crate::rotation::item(g, duration)),
            );
        }
        let mut items = Vec::new();
        for current in self.variety.data["items"].as_array().unwrap() {
            if items.len() < count {
                items.push(current.clone());
            }
        }
        let mut chosen: std::collections::HashSet<String> =
            items.iter().map(|g| text(g, "id")).collect();
        for item in &pool {
            if items.len() >= count {
                break;
            }
            if chosen.insert(text(item, "id")) {
                items.push(item.clone());
            }
        }
        self.variety.replace(items)?;
        self.variety.data["pool"] = json!(pool);
        self.variety.data["defaultMinutes"] = json!(duration);
        self.variety.save()
    }
    pub fn remember_steam_search(&mut self, games: Vec<Value>) {
        for game in games {
            if let Some(old) = self
                .steam_search
                .iter_mut()
                .find(|g| g["steamId"] == game["steamId"])
            {
                *old = game;
            } else {
                self.steam_search.push(game);
            }
        }
        if self.steam_search.len() > 400 {
            self.steam_search.drain(..self.steam_search.len() - 400);
        }
        let _ = self.save("steam_search.json", &json!(self.steam_search));
    }
    fn refresh_steam_runtime(&mut self) {
        if self.steam_runtime.as_ref().is_some_and(|p| p.is_file()) {
            return;
        }
        let extra: Vec<PathBuf> = self.overrides["settings"]["steam_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
        self.steam_runtime = crate::rotation::discover_runtime(&crate::rotation::libraries(&extra));
    }
    fn steam_games(&self) -> Vec<Value> {
        let extra: Vec<PathBuf> = self.overrides["settings"]["steam_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(PathBuf::from))
            .collect();
        let mut games = crate::rotation::installed_games(&crate::rotation::libraries(&extra));
        let mut seen: std::collections::HashSet<String> = games
            .iter()
            .filter_map(|g| g["steamId"].as_str().map(str::to_string))
            .collect();
        for item in &self.steam_idle.items {
            if seen.insert(text(item, "steamId")) {
                let mut game = item.clone();
                game["name"] = item["gameName"].clone();
                game["installed"] = json!(false);
                games.push(game);
            }
        }
        for game in &self.steam_search {
            if seen.insert(text(game, "steamId")) {
                games.push(game.clone());
            }
        }
        for game in &self.catalog {
            for sku in game["third_party_skus"].as_array().into_iter().flatten() {
                if sku["distributor"] == "steam" {
                    if let Some(id) = sku["id"].as_str().filter(|id| {
                        !id.is_empty() && id.len() <= 10 && id.bytes().all(|b| b.is_ascii_digit())
                    }) {
                        if seen.insert(id.to_string()) {
                            let mut game = game.clone();
                            game["steamId"] = json!(id);
                            game["id"] = json!(id);
                            game["installed"] = json!(false);
                            games.push(game);
                        }
                    }
                }
            }
        }
        games
    }
    fn pause_game_queue(&mut self) -> Result<(), String> {
        if !self.queue.active() {
            return Ok(());
        }
        if self.queue.phase == "running" {
            if let Some(session) = &self.session {
                let elapsed = self.elapsed_seconds(session);
                if let Some(item) = self.queue.items.get_mut(self.queue.index) {
                    item["resumeElapsedSeconds"] = json!(elapsed);
                    item["resumeCountedSeconds"] = json!(elapsed);
                    item["state"] = json!("paused");
                }
            }
            self.stop_process("queue_paused")?;
        }
        self.queue.phase = "paused".into();
        self.queue.waiting = None;
        self.queue.wait_deadline = None;
        self.save_queue()
    }
    pub fn shutdown(&mut self) {
        let _ = self.pause_game_queue();
        let _ = self.variety.pause();
        let _ = self.steam_idle.pause(None);
        self.queue.waiting = None;
        self.queue.wait_deadline = None;
        self.quest_queue.waiting = None;
        self.quest_queue.wait_deadline = None;
        let _ = self.stop_process("app_closed");
        let _ = self.save_quest_queue();
    }
    pub fn install_catalog(&mut self, data: Value) -> Result<(), String> {
        let list = data.as_array().ok_or("Invalid catalog")?;
        if list.is_empty() || list.iter().any(|g| !valid_game(g)) {
            return Err("Invalid catalog metadata".into());
        }
        let catalog = unique_catalog(list.clone());
        self.save("detectable_cache.json", &json!(catalog))?;
        self.executable_resolver = crate::executables::Resolver::new(&catalog);
        self.catalog = catalog;
        self.meta = json!({"lastUpdated":stamp(),"count":self.catalog.len()});
        self.save("catalog_meta.json", &self.meta)
    }
    pub fn apply_catalog_refresh(&mut self, download: CatalogDownload) -> Result<(), String> {
        self.executable_resolver.clear_local();
        let changed = download.data.is_some();
        if let Some(data) = download.data {
            self.install_catalog(data)?;
        }
        if let Some(etag) = download.etag {
            self.meta["etag"] = json!(etag);
        }
        if let Some(modified) = download.last_modified {
            self.meta["lastModified"] = json!(modified);
        }
        self.meta["lastChecked"] = json!(stamp());
        self.meta["changed"] = json!(changed);
        self.save("catalog_meta.json", &self.meta)
    }
    pub fn install_quests(&mut self, data: Value) -> Result<(), String> {
        let quests = feed::normalize(&data, Utc::now().timestamp_millis())?;
        let previous: Vec<String> = self.quests["quests"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|q| q["image"].as_str().map(str::to_string))
            .collect();
        let value = json!({"source":"community_feed","sourceUrl":feed::SOURCE,"last_synced":stamp(),"quests":quests});
        self.save("active_quests.json", &value)?;
        self.quests = value;
        let mut keep: std::collections::HashSet<String> = self.quests["quests"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(quest_ids)
            .collect();
        if let Some(id) = self.session.as_ref().and_then(|s| s["questId"].as_str()) {
            keep.insert(id.to_string());
        }
        self.quest_progress
            .as_object_mut()
            .unwrap()
            .retain(|id, _| keep.contains(id));
        self.save("quest_progress.json", &self.quest_progress)?;
        self.prune_quest_cache(&previous);
        Ok(())
    }
    pub fn route(&mut self, route: &str, body: Value) -> Result<Value, String> {
        let result = self.route_inner(route, body);
        if route.starts_with("/api/quests/") || route == "/api/stop" {
            self.save_quest_queue()?;
        }
        result
    }
    fn route_inner(&mut self, route: &str, body: Value) -> Result<Value, String> {
        self.tick();
        let parsed =
            url::Url::parse(&format!("http://localhost{route}")).map_err(|e| e.to_string())?;
        let query: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();
        let get = |key: &str| query.get(key).map(String::as_str).unwrap_or("");
        let post = !body.is_null();
        if self.variety.active()
            && [
                "/api/play",
                "/api/quests/start",
                "/api/quests/resume-queue",
                "/api/queue/start",
            ]
            .contains(&parsed.path())
        {
            return Err("A game is already running".into());
        }
        if self.variety.active() && ["/api/stop-current", "/api/stop"].contains(&parsed.path()) {
            self.variety.pause()?;
            self.stop_process("variety_paused")?;
            return Ok(self.action_status());
        }
        match parsed.path() {
            "/api/variety" => {
                self.refresh_variety_cards()?;
                Ok(self.variety.status())
            }
            "/api/variety/shuffle" => self.shuffle_variety(&body),
            "/api/variety/resize" => {
                self.resize_variety(&body)?;
                Ok(self.variety.status())
            }
            "/api/variety/start" => {
                self.start_variety()?;
                Ok(self.variety.status())
            }
            "/api/variety/pause" | "/api/variety/stop" => {
                if self.variety.active() {
                    self.variety.pause()?;
                    self.stop_process("variety_paused")?;
                }
                Ok(self.variety.status())
            }
            "/api/variety/items" => {
                self.variety.edit(&body)?;
                self.sync_variety_pool();
                self.variety.save()?;
                Ok(self.variety.status())
            }
            "/api/variety/duration" => {
                let minutes =
                    crate::rotation::minutes(body["durationMinutes"].as_u64().unwrap_or(0))?;
                let index = self.variety.index();
                let active = self.variety.active();
                for (i, item) in self.variety.data["items"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .enumerate()
                {
                    if item["state"] != "completed" && (!active || i > index) {
                        item["durationMinutes"] = json!(minutes);
                        item["elapsedSeconds"] = json!(0);
                    }
                }
                self.variety.data["defaultMinutes"] = json!(minutes);
                self.sync_variety_pool();
                if let Some(pool) = self.variety.data["pool"].as_array_mut() {
                    for item in pool {
                        if item["state"] != "completed" {
                            item["durationMinutes"] = json!(minutes);
                        }
                    }
                }
                self.variety.save()?;
                Ok(self.variety.status())
            }
            "/api/steam/status" => Ok(
                json!({"rotation":self.steam_idle.status(),"runtimeReady":self.steam_runtime.is_some()}),
            ),
            "/api/steam/games" => {
                self.refresh_steam_runtime();
                let query = get("q").to_lowercase();
                let games = self.steam_games();
                let games: Vec<_> = games
                    .into_iter()
                    .filter(|g| {
                        if query.is_empty() {
                            g["installed"] == true
                        } else {
                            text(g, "name").to_lowercase().contains(&query)
                                || text(g, "steamId") == query
                        }
                    })
                    .collect();
                Ok(
                    json!({"total":games.len(),"games":games.into_iter().take(50).collect::<Vec<_>>(),"runtimeReady":self.steam_runtime.is_some()}),
                )
            }
            "/api/steam/start" => {
                self.refresh_steam_runtime();
                let dll = self.steam_runtime.clone().ok_or("steam_runtime_missing")?;
                crate::steam_client::ensure_open()?;
                let sessions: Vec<(String, u64)> =
                    if let Some(sessions) = body["sessions"].as_array() {
                        if sessions.len() > 32 {
                            return Err("steam_session_limit".into());
                        }
                        sessions
                            .iter()
                            .map(|item| {
                                Ok((
                                    item["steamId"]
                                        .as_str()
                                        .ok_or("Invalid Steam game")?
                                        .to_string(),
                                    item["durationMinutes"]
                                        .as_u64()
                                        .filter(|n| *n <= 1440)
                                        .ok_or("Invalid duration")?,
                                ))
                            })
                            .collect::<Result<_, String>>()?
                    } else {
                        let duration = body["durationMinutes"]
                            .as_u64()
                            .filter(|n| *n <= 1440)
                            .ok_or("Invalid duration")?;
                        if let Some(ids) = body["steamIds"].as_array() {
                            if ids.len() > 32 {
                                return Err("steam_session_limit".into());
                            }
                            ids.iter()
                                .map(|id| {
                                    id.as_str()
                                        .map(|id| (id.to_string(), duration))
                                        .ok_or("Invalid Steam game".to_string())
                                })
                                .collect::<Result<_, _>>()?
                        } else {
                            vec![(text(&body, "steamId"), duration)]
                        }
                    };
                if sessions.is_empty() {
                    return Err("No games selected".into());
                }
                let games = self.steam_games();
                let selected: Vec<(Value, u64)> = sessions
                    .iter()
                    .map(|(id, duration)| {
                        games
                            .iter()
                            .find(|game| game["steamId"] == id.as_str())
                            .cloned()
                            .map(|game| (game, *duration))
                            .ok_or("Steam game not found".to_string())
                    })
                    .collect::<Result<_, _>>()?;
                for (game, duration) in selected {
                    self.steam_idle.start(&self.root, &dll, &game, duration)?;
                }
                Ok(self.steam_idle.status())
            }
            "/api/steam/pause" => {
                self.steam_idle.pause(body["steamId"].as_str())?;
                Ok(self.steam_idle.status())
            }
            "/api/steam/stop" => {
                self.steam_idle.stop(body["steamId"].as_str())?;
                Ok(self.steam_idle.status())
            }
            "/api/status" => Ok(self.status()),
            "/api/queue" => Ok(self.queue.status()),
            "/api/history" => Ok(json!({"history":self.history})),
            "/api/games" => {
                let q = get("q").trim().to_lowercase();
                let limit = get("limit").parse::<usize>().unwrap_or(50).clamp(1, 50);
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
                json!({"success":true,"count":self.catalog.len(),"lastUpdated":self.meta["lastUpdated"],"changed":self.meta["changed"]}),
            ),
            "/api/resolve" => Ok(self.resolve(
                &self
                    .game(get("id"))
                    .unwrap_or(json!({"id":get("id"),"name":"Game"})),
            )),
            "/api/settings" => {
                if !post {
                    let mut s = self.overrides["settings"].clone();
                    if !s.is_object() {
                        s = json!({});
                    }
                    let saved_pref = s["run_on_startup"].as_bool().unwrap_or(false);
                    s["run_on_startup"] = json!(is_startup_enabled(saved_pref));
                    s["start_minimized"] = json!(s["start_minimized"].as_bool().unwrap_or(false));
                    s["minimize_to_tray"] = json!(s["minimize_to_tray"].as_bool().unwrap_or(false));
                    s["auto_discord_rpc"] = json!(s["auto_discord_rpc"].as_bool().unwrap_or(true));
                    s["desktop_notifications"] =
                        json!(s["desktop_notifications"].as_bool().unwrap_or(true));
                    s["sound_alerts"] = json!(s["sound_alerts"].as_bool().unwrap_or(true));
                    s["quest_duration_seconds"] = json!(self.quest_target_duration_seconds());
                    s["total_playtime_seconds"] = json!(self.total_playtime_seconds());
                    s["steam_playtime_seconds"] = json!(self.steam_idle.total_seconds());
                    s["active_steam_library"] =
                        json!(self.active_steam_library().to_string_lossy().to_string());
                    return Ok(s);
                }
                let mut proposed = self.overrides.clone();
                if let Some(secs) = body["quest_duration_seconds"].as_i64() {
                    proposed["settings"]["quest_duration_seconds"] = json!(secs.clamp(60, 3600));
                }
                if let Some(b) = body["sound_alerts"].as_bool() {
                    proposed["settings"]["sound_alerts"] = json!(b);
                }
                if let Some(b) = body["scan_local_steam"].as_bool() {
                    proposed["settings"]["scan_local_steam"] = json!(b);
                }
                if let Some(b) = body["start_minimized"].as_bool() {
                    proposed["settings"]["start_minimized"] = json!(b);
                }
                if let Some(b) = body["minimize_to_tray"].as_bool() {
                    proposed["settings"]["minimize_to_tray"] = json!(b);
                }
                if let Some(b) = body["auto_discord_rpc"].as_bool() {
                    proposed["settings"]["auto_discord_rpc"] = json!(b);
                }
                if let Some(b) = body["desktop_notifications"].as_bool() {
                    proposed["settings"]["desktop_notifications"] = json!(b);
                }
                if let Some(b) = body["run_on_startup"].as_bool() {
                    proposed["settings"]["run_on_startup"] = json!(b);
                }
                if body.get("run_on_startup").is_some() || body.get("start_minimized").is_some() {
                    let run_enabled = proposed["settings"]["run_on_startup"]
                        .as_bool()
                        .unwrap_or(false);
                    let min_enabled = proposed["settings"]["start_minimized"]
                        .as_bool()
                        .unwrap_or(false);
                    set_startup_enabled(run_enabled, min_enabled)?;
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
                if let Err(error) = self.save("questro_overrides.json", &proposed) {
                    if body.get("run_on_startup").is_some() || body.get("start_minimized").is_some()
                    {
                        let _ = set_startup_enabled(
                            self.overrides["settings"]["run_on_startup"]
                                .as_bool()
                                .unwrap_or(false),
                            self.overrides["settings"]["start_minimized"]
                                .as_bool()
                                .unwrap_or(false),
                        );
                    }
                    return Err(error);
                }
                self.overrides = proposed;
                self.executable_resolver.clear_local();
                let saved_pref = self.overrides["settings"]["run_on_startup"]
                    .as_bool()
                    .unwrap_or(false);
                Ok(json!({
                    "success": true,
                    "run_on_startup": is_startup_enabled(saved_pref),
                    "start_minimized": self.overrides["settings"]["start_minimized"].as_bool().unwrap_or(false),
                    "minimize_to_tray": self.overrides["settings"]["minimize_to_tray"].as_bool().unwrap_or(false),
                    "auto_discord_rpc": self.overrides["settings"]["auto_discord_rpc"].as_bool().unwrap_or(true),
                    "desktop_notifications": self.overrides["settings"]["desktop_notifications"].as_bool().unwrap_or(true),
                    "sound_alerts": self.overrides["settings"]["sound_alerts"].as_bool().unwrap_or(true),
                    "quest_duration_seconds": self.quest_target_duration_seconds(),
                    "total_playtime_seconds": self.total_playtime_seconds(),
                    "active_steam_library": self.active_steam_library().to_string_lossy().to_string()
                }))
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
                self.save("questro_overrides.json", &proposed)?;
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
                    json!({"appId":game["id"],"gameName":game["name"],"executableName":if body["executableAutomatic"]==true{resolution["primaryExecutable"].clone()}else{body["executableName"].as_str().filter(|s|!s.is_empty()).map(json_value).unwrap_or(resolution["primaryExecutable"].clone())},"durationMinutes":body["durationMinutes"],"iconHash":game["icon_hash"],"syncSteam":body["syncSteam"]==true})
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
            "/api/queue/pause" => {
                self.pause_game_queue()?;
                Ok(self.action_status())
            }
            "/api/queue/start" => {
                if self.queue.active() || self.quest_queue.active() || self.session.is_some() {
                    return Err("A game is already running".into());
                }
                self.queue.items.retain(|item| {
                    !["completed", "cancelled"].contains(&text(item, "state").as_str())
                });
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
                let mut quests:Vec<Value>=self.quests["quests"].as_array().into_iter().flatten().filter(|q|feed::current(q)).map(|q|{let mut q=q.clone();let app=text(&q,"appId");let game=self.game(&app).unwrap_or(json!({"id":app,"name":q["gameName"],"executables":[],"catalogMissing":true}));q["game"]=self.enriched(game);q["localClosedReason"]=json!(self.closed_reason(&q).filter(|r|*r!="open"));q["localFinished"]=json!(self.quest_finished(&q));q["resumeElapsedSeconds"]=json!(if self.quest_finished(&q){0}else{self.resumed_seconds(&q)});q}).collect();
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
                self.append(json!({"appId":game["id"],"gameName":game["name"],"executableName":resolution["primaryExecutable"],"durationMinutes":(self.quest_target_duration_seconds() as f64)/60.0,"resumeElapsedSeconds":self.resumed_seconds(&quest),"iconHash":game["icon_hash"],"questId":id,"questImage":quest["image"]}),true)
            }
            "/api/quests/pause" => {
                let id = storage::id(&text(&body, "questId"))?;
                self.pause_quest(&id)
            }
            "/api/quests/resume-queue" => self.resume_quest_queue(),
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
fn unique_catalog(games: Vec<Value>) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    games
        .into_iter()
        .filter(|game| seen.insert(text(game, "id")))
        .collect()
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
    let bytes = include_bytes!("../../runner/src-win.exe");
    if target.exists() {
        if let Ok(existing) = fs::read(target) {
            if existing == bytes {
                return Ok(());
            }
        }
        let _ = fs::remove_file(target);
    }
    let temporary = target.with_extension(format!("{}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        drop(file);
        if target.exists() {
            let _ = fs::remove_file(target);
        }
        fs::rename(&temporary, target).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[cfg(windows)]
fn is_startup_enabled(fallback: bool) -> bool {
    use std::os::windows::process::CommandExt;
    let output = Command::new("reg")
        .args([
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            "/v",
            "Questro",
        ])
        .creation_flags(0x08000000)
        .output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => fallback,
    }
}
#[cfg(not(windows))]
fn is_startup_enabled(fallback: bool) -> bool {
    fallback
}
#[cfg(windows)]
fn set_startup_enabled(enable: bool, minimized: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    if enable {
        if let Ok(exe) = std::env::current_exe() {
            let exe_str = exe.to_string_lossy();
            let data = if minimized {
                format!("\"{exe_str}\" --minimized")
            } else {
                format!("\"{exe_str}\"")
            };
            let result = Command::new("reg")
                .args([
                    "add",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    "Questro",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &data,
                    "/f",
                ])
                .creation_flags(0x08000000)
                .status()
                .map_err(|_| "Could not update Windows startup")?;
            if !result.success() {
                return Err("Could not update Windows startup".into());
            }
        } else {
            return Err("Could not locate Questro executable".into());
        }
    } else {
        if !is_startup_enabled(false) {
            return Ok(());
        }
        let result = Command::new("reg")
            .args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "Questro",
                "/f",
            ])
            .creation_flags(0x08000000)
            .status()
            .map_err(|_| "Could not update Windows startup")?;
        if !result.success() {
            return Err("Could not update Windows startup".into());
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn set_startup_enabled(_enable: bool, _minimized: bool) -> Result<(), String> {
    Ok(())
}
const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
fn read_json(reader: impl std::io::Read, limit: u64) -> Result<Value, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Response too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
pub fn fetch_json(url: &str) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    read_json(response, MAX_JSON_BYTES)
}
pub struct CatalogDownload {
    pub data: Option<Value>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}
pub fn fetch_catalog(meta: &Value, has_saved_catalog: bool) -> Result<CatalogDownload, String> {
    use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .gzip(true)
        .brotli(true)
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client
        .get("https://discord.com/api/applications/detectable")
        .header("Accept", "application/json");
    if has_saved_catalog {
        if let Some(etag) = meta["etag"].as_str().filter(|value| !value.is_empty()) {
            request = request.header(IF_NONE_MATCH, etag);
        } else if let Some(modified) = meta["lastModified"]
            .as_str()
            .filter(|value| !value.is_empty())
        {
            request = request.header(IF_MODIFIED_SINCE, modified);
        }
    }
    let response = request.send().map_err(|e| e.to_string())?;
    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    let last_modified = response
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        if !has_saved_catalog {
            return Err("Catalog cache unavailable".into());
        }
        return Ok(CatalogDownload {
            data: None,
            etag,
            last_modified,
        });
    }
    if response.status() != reqwest::StatusCode::OK {
        return Err("Catalog source unavailable".into());
    }
    let data = read_json(response, MAX_JSON_BYTES)?;
    Ok(CatalogDownload {
        data: Some(data),
        etag,
        last_modified,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_response_above_the_old_limit_is_accepted_but_still_bounded() {
        let mut body = vec![b' '; 13 * 1024 * 1024];
        body[..2].copy_from_slice(b"[]");
        assert_eq!(
            read_json(body.as_slice(), MAX_JSON_BYTES).unwrap(),
            json!([])
        );
        assert_eq!(
            read_json(std::io::repeat(b' '), 16).unwrap_err(),
            "Response too large"
        );
    }
    #[test]
    fn expired_quest_artwork_is_cleaned_offline_but_a_running_card_is_preserved() {
        use sha2::{Digest, Sha256};
        let mut f = Fixture::new();
        let image = "https://cdn.discordapp.com/quests/123456/expired.png";
        let path = f.0.root.join(format!(
            "data/artwork/{:x}.png",
            Sha256::digest(image.as_bytes())
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"cached image").unwrap();
        f.0.quests = json!({"quests":[{"id":"123456","kind":"play","durationSeconds":900,"expiresAt":(Utc::now()-chrono::Duration::hours(1)).to_rfc3339(),"image":image}]});
        f.0.session = Some(json!({"questId":"123456","questImage":image}));
        f.0.prune_quest_cache(&[]);
        assert!(path.exists());
        f.0.session = None;
        f.0.prune_quest_cache(&[]);
        assert!(!path.exists());
    }
    #[test]
    fn variety_shuffle_supports_custom_counts_without_repeating_the_previous_selection() {
        let mut f = Fixture::new();
        f.0.catalog=(0..600).map(|n|json!({"id":(100000+n).to_string(),"name":format!("Game {n}"),"type":1,"icon_hash":"0123456789abcdef0123456789abcdef"})).collect();
        let first =
            f.0.route(
                "/api/variety/shuffle",
                json!({"count":100,"durationMinutes":10}),
            )
            .unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 100);
        let ids: std::collections::HashSet<String> = first["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| text(i, "id"))
            .collect();
        assert_eq!(ids.len(), 100);
        let second =
            f.0.route(
                "/api/variety/shuffle",
                json!({"count":137,"durationMinutes":7}),
            )
            .unwrap();
        assert_eq!(second["items"].as_array().unwrap().len(), 137);
        assert!(second["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| !ids.contains(&text(i, "id")) && i["durationMinutes"] == 7));
        assert!(f
            .0
            .route(
                "/api/variety/shuffle",
                json!({"count":501,"durationMinutes":10})
            )
            .is_err());
        f.0.variety.prepare().unwrap();
        assert!(f
            .0
            .route(
                "/api/variety/shuffle",
                json!({"count":10,"durationMinutes":10})
            )
            .is_err());
    }
    #[test]
    fn steam_titles_use_unique_steam_ids_instead_of_discord_ids() {
        let mut f = Fixture::new();
        f.0.catalog = vec![
            json!({"id":"123456","name":"First","third_party_skus":[{"distributor":"steam","id":"999999001"}]}),
            json!({"id":"123456","name":"Second","third_party_skus":[{"distributor":"steam","id":"999999002"}]}),
        ];
        let games = f.0.steam_games();
        let games: Vec<_> = games
            .into_iter()
            .filter(|g| ["999999001", "999999002"].contains(&g["steamId"].as_str().unwrap_or("")))
            .collect();
        assert_eq!(games.len(), 2);
        assert_ne!(games[0]["id"], games[1]["id"]);
    }
    #[test]
    fn variety_resize_keeps_games_order_and_individual_durations() {
        let mut f = Fixture::new();
        f.0.catalog=(0..600).map(|n|json!({"id":(100000+n).to_string(),"name":format!("Game {n}"),"type":1,"icon_hash":"0123456789abcdef0123456789abcdef"})).collect();
        let first =
            f.0.route(
                "/api/variety/shuffle",
                json!({"count":100,"durationMinutes":10}),
            )
            .unwrap();
        let id = first["items"][0]["id"].clone();
        f.0.route(
            "/api/variety/items",
            json!({"action":"duration","id":id,"durationMinutes":7}),
        )
        .unwrap();
        let ten =
            f.0.route("/api/variety/resize", json!({"count":10}))
                .unwrap();
        assert_eq!(ten["items"].as_array().unwrap().len(), 10);
        let fifty =
            f.0.route("/api/variety/resize", json!({"count":50}))
                .unwrap();
        for n in 0..50 {
            assert_eq!(fifty["items"][n]["id"], first["items"][n]["id"]);
        }
        assert_eq!(fifty["items"][0]["durationMinutes"], 7);
    }
    #[test]
    fn variety_uses_only_discord_artwork_and_one_shared_duration() {
        let mut f = Fixture::new();
        f.0.catalog = vec![
            json!({"id":"100001","name":"Discord image","icon_hash":"0123456789abcdef0123456789abcdef"}),
            json!({"id":"100002","name":"Cover image","cover_image_hash":"abcdef0123456789abcdef0123456789"}),
            json!({"id":"100003","name":"Steam only","third_party_skus":[{"distributor":"steam","id":"730"}]}),
        ];
        let chosen =
            f.0.route(
                "/api/variety/shuffle",
                json!({"count":10,"durationMinutes":12}),
            )
            .unwrap();
        assert_eq!(chosen["items"].as_array().unwrap().len(), 2);
        assert!(chosen["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(has_discord_artwork));
        f.0.route("/api/variety/duration", json!({"durationMinutes":20}))
            .unwrap();
        assert!(f.0.variety.data["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["durationMinutes"] == 20));
        assert!(f.0.variety.data["pool"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["durationMinutes"] == 20));
    }
    #[test]
    fn game_queue_pause_restores_progress_without_double_counting() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 60.0, false), false).unwrap();
        f.0.append(item("234567", 15.0, false), false).unwrap();
        f.0.session.as_mut().unwrap()["elapsedBeforeSegment"] = json!(1200);
        f.0.route("/api/queue/pause", json!({})).unwrap();
        assert!(f.0.session.is_none());
        assert_eq!(f.0.queue.phase, "paused");
        let mut restored = Backend::new(f.0.root.clone()).unwrap();
        restored.allow_rpc = false;
        assert_eq!(restored.queue.items.len(), 2);
        assert_eq!(restored.queue.phase, "paused");
        let total = restored.total_playtime_seconds();
        restored.route("/api/queue/start", json!({})).unwrap();
        assert_eq!(
            restored.session.as_ref().unwrap()["elapsedBeforeSegment"],
            1200
        );
        assert!(restored.total_playtime_seconds() <= total + 1);
        restored.shutdown();
    }
    #[test]
    fn shutdown_preserves_queue_and_adding_a_game_does_not_replace_it() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 30.0, false), false).unwrap();
        f.0.session.as_mut().unwrap()["elapsedBeforeSegment"] = json!(75);
        f.0.shutdown();
        let mut restored = Backend::new(f.0.root.clone()).unwrap();
        restored.allow_rpc = false;
        assert_eq!(restored.queue.items[0]["resumeElapsedSeconds"], 75);
        restored.append(item("234567", 15.0, false), false).unwrap();
        assert_eq!(restored.queue.items.len(), 2);
        assert!(restored.session.is_none());
        restored
            .route("/api/queue/items", json!({"action":"clear"}))
            .unwrap();
        assert!(restored.queue.items.is_empty());
    }
    #[test]
    fn sync_failure_preserves_game_queue_and_its_progress() {
        let mut f = Fixture::new();
        f.0.append(item("123456", 15.0, false), false).unwrap();
        f.0.session.as_mut().unwrap()["pairedSteamId"] = json!("1000");
        f.0.session.as_mut().unwrap()["elapsedBeforeSegment"] = json!(45);
        f.0.steam_idle.items =
            vec![json!({"steamId":"1000","state":"error","error":"steam_not_owned"})];
        f.0.tick();
        assert!(f.0.session.is_none());
        assert_eq!(f.0.queue.phase, "error");
        assert_eq!(f.0.queue.items[0]["resumeElapsedSeconds"], 45);
        assert_eq!(f.0.queue.error.as_deref(), Some("steam_not_owned"));
    }
    #[test]
    fn sync_rejects_an_unmatched_title_before_launching() {
        let mut f = Fixture::new();
        let mut game = item("123456", 15.0, false);
        game["syncSteam"] = json!(true);
        assert_eq!(
            f.0.start_process(&game).unwrap_err(),
            "steam_sync_unavailable"
        );
        assert!(f.0.session.is_none());
    }
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
    fn prepare_quest_fixture(backend: &mut Backend) {
        let expires = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        backend.quests = json!({"quests":[{"id":"123456","appId":"123456","gameName":"First","kind":"play","durationSeconds":900,"expiresAt":expires},{"id":"234567","appId":"234567","gameName":"Second","kind":"play","durationSeconds":900,"expiresAt":expires}]});
        backend.save("active_quests.json", &backend.quests).unwrap();
    }
    #[test]
    fn paused_quests_and_pending_order_survive_restart_without_offline_time() {
        let mut f = Fixture::new();
        prepare_quest_fixture(&mut f.0);
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/start", json!({"questId":"234567"}))
            .unwrap();
        f.0.started_at = Some(Instant::now() - Duration::from_secs(37));
        f.0.route("/api/quests/pause", json!({"questId":"123456"}))
            .unwrap();
        assert!(f.0.session.is_none());
        assert!(!f.0.quest_queue.active());
        assert!(!f.0.quest_finished(&f.0.available_quest("123456").unwrap()));
        f.0.shutdown();
        let mut restored = Backend::new(f.0.root.clone()).unwrap();
        restored.allow_rpc = false;
        assert!(restored.session.is_none());
        assert_eq!(restored.quest_queue.phase, "paused");
        assert_eq!(restored.quest_queue.items.len(), 2);
        assert_eq!(restored.quest_queue.items[0]["questId"], "123456");
        assert_eq!(restored.quest_queue.items[1]["questId"], "234567");
        restored
            .route("/api/quests/resume-queue", json!({}))
            .unwrap();
        assert_eq!(
            restored.status()["simulation"]["session"]["elapsedSeconds"],
            37
        );
        assert_eq!(
            restored.status()["simulation"]["session"]["remainingSeconds"],
            863
        );
        restored.shutdown();
    }
    #[test]
    fn lost_discord_connection_stops_quest_clock_without_closing_quest() {
        let mut f = Fixture::new();
        prepare_quest_fixture(&mut f.0);
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.started_at = Some(Instant::now() - Duration::from_secs(12));
        f.0.allow_rpc = true;
        f.0.discord_mock = Some(false);
        f.0.tick();
        assert!(f.0.started_at.is_none());
        assert_eq!(f.0.status()["simulation"]["session"]["elapsedSeconds"], 12);
        assert_eq!(f.0.status()["simulation"]["session"]["timerWaiting"], true);
        f.0.tick();
        assert_eq!(f.0.status()["simulation"]["session"]["elapsedSeconds"], 12);
    }
    #[test]
    fn cancelled_saved_quest_is_not_restored() {
        let mut f = Fixture::new();
        prepare_quest_fixture(&mut f.0);
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/start", json!({"questId":"234567"}))
            .unwrap();
        f.0.route("/api/quests/pause", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/cancel", json!({"questId":"123456"}))
            .unwrap();
        let restored = Backend::new(f.0.root.clone()).unwrap();
        assert_eq!(restored.quest_queue.items.len(), 1);
        assert_eq!(restored.quest_queue.items[0]["questId"], "234567");
    }
    #[test]
    fn unchanged_catalog_refresh_does_not_rewrite_saved_game_list() {
        let mut f = Fixture::new();
        f.0.install_catalog(json!([{"id":"123456","name":"Game","executables":[]}]))
            .unwrap();
        let file = f.0.root.join("data/detectable_cache.json");
        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        f.0.apply_catalog_refresh(CatalogDownload {
            data: None,
            etag: Some("etag-1".into()),
            last_modified: None,
        })
        .unwrap();
        assert_eq!(modified, fs::metadata(file).unwrap().modified().unwrap());
        assert_eq!(f.0.meta["changed"], false);
    }
    #[test]
    fn reopening_a_paused_then_cancelled_quest_starts_a_fresh_timer() {
        let mut f = Fixture::new();
        prepare_quest_fixture(&mut f.0);
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.started_at = Some(Instant::now() - Duration::from_secs(40));
        f.0.route("/api/quests/pause", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/cancel", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/reopen", json!({"questId":"123456"}))
            .unwrap();
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        assert_eq!(f.0.status()["simulation"]["session"]["elapsedSeconds"], 0);
    }
    #[test]
    fn saved_queue_recovers_elapsed_time_if_progress_file_is_missing() {
        let mut f = Fixture::new();
        prepare_quest_fixture(&mut f.0);
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.started_at = Some(Instant::now() - Duration::from_secs(40));
        f.0.route("/api/quests/pause", json!({"questId":"123456"}))
            .unwrap();
        fs::remove_file(f.0.root.join("data/quest_progress.json")).unwrap();
        let mut restored = Backend::new(f.0.root.clone()).unwrap();
        restored.allow_rpc = false;
        restored
            .route("/api/quests/resume-queue", json!({}))
            .unwrap();
        assert_eq!(
            restored.status()["simulation"]["session"]["elapsedSeconds"],
            40
        );
        restored.shutdown();
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
    fn restarting_a_queue_never_replays_hidden_finished_games() {
        let mut f = Fixture::new();
        let mut finished = validated_item(item("123456", 15., false)).unwrap();
        finished["state"] = json!("completed");
        let mut cancelled = validated_item(item("234567", 15., false)).unwrap();
        cancelled["state"] = json!("cancelled");
        let upcoming = validated_item(item("345678", 30., false)).unwrap();
        f.0.queue.items = vec![finished, cancelled, upcoming];
        f.0.queue.phase = "error".into();
        f.0.route("/api/queue/start", json!({})).unwrap();
        assert_eq!(f.0.queue.items.len(), 1);
        assert_eq!(f.0.session.as_ref().unwrap()["appId"], "345678");
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
        assert_eq!(f.0.session.as_ref().unwrap()["targetDurationSeconds"], 900);
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
    fn quest_warmup_delay_before_starting_timer() {
        let mut f = Fixture::new();
        prepare_quest_fixture(&mut f.0);
        f.0.discord_mock = Some(true);
        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        f.0.started_at = None;

        // First tick triggers warmup buffer
        f.0.tick_inner();
        assert!(f.0.warmup_deadline.is_some());
        assert!(f.0.started_at.is_none());
        let status = f.0.status();
        assert_eq!(status["simulation"]["session"]["timerWarmup"], true);
        assert!(
            status["simulation"]["session"]["warmupSeconds"]
                .as_i64()
                .unwrap()
                >= 1
        );

        // When warmup expires, next tick starts the countdown timer
        f.0.warmup_deadline = Some(Instant::now() - Duration::from_millis(50));
        f.0.tick_inner();
        assert!(f.0.warmup_deadline.is_none());
        assert!(f.0.started_at.is_some());
        let status = f.0.status();
        assert_ne!(status["simulation"]["session"]["timerWarmup"], true);
    }
    #[test]
    fn custom_quest_duration_and_playtime_accumulation() {
        let mut f = Fixture::new();
        f.0.catalog = vec![
            json!({"id":"123456","name":"Test","executables":[{"os":"win32","name":"audit-first.exe"}]}),
        ];
        let expiry = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        f.0.quests = json!({"quests":[{"id":"123456","appId":"123456","gameName":"Test","kind":"play","durationSeconds":900,"expiresAt":expiry}]});
        f.0.save("active_quests.json", &f.0.quests).unwrap();

        let res =
            f.0.route(
                "/api/settings",
                json!({"quest_duration_seconds": 910, "sound_alerts": true}),
            )
            .unwrap();
        assert_eq!(res["quest_duration_seconds"], 910);
        assert_eq!(res["sound_alerts"], true);
        assert_eq!(f.0.quest_target_duration_seconds(), 910);

        f.0.route("/api/quests/start", json!({"questId":"123456"}))
            .unwrap();
        assert_eq!(f.0.session.as_ref().unwrap()["targetDurationSeconds"], 910);

        let initial_playtime = f.0.total_playtime_seconds();
        f.0.session.as_mut().unwrap()["elapsedBeforeSegment"] = json!(120);
        f.0.stop_process("user_requested").unwrap();
        assert!(f.0.total_playtime_seconds() >= initial_playtime + 120);
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
        fs::create_dir(f.0.root.join("data/questro_overrides.json")).unwrap();
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
    #[test]
    fn settings_and_override_features_persist_and_reset() {
        let mut f = Fixture::new();
        let get = f.0.route("/api/settings", Value::Null).unwrap();
        assert_eq!(get["auto_discord_rpc"], true);
        assert_eq!(get["desktop_notifications"], true);
        assert_eq!(get["start_minimized"], false);
        assert_eq!(get["minimize_to_tray"], false);
        assert!(get["active_steam_library"].as_str().is_some());

        f.0.route(
            "/api/settings",
            json!({
                "auto_discord_rpc": false,
                "desktop_notifications": false,
                "start_minimized": true,
                "minimize_to_tray": true
            }),
        )
        .unwrap();

        let updated = f.0.route("/api/settings", Value::Null).unwrap();
        assert_eq!(updated["auto_discord_rpc"], false);
        assert_eq!(updated["desktop_notifications"], false);
        assert_eq!(updated["start_minimized"], true);
        assert_eq!(updated["minimize_to_tray"], true);

        f.0.route(
            "/api/override",
            json!({"appId": "123456", "executableName": "custom.exe"}),
        )
        .unwrap();
        assert_eq!(f.0.overrides["custom_executables"]["123456"], "custom.exe");

        f.0.route("/api/override", json!({"resetAll": true}))
            .unwrap();
        assert!(f.0.overrides["custom_executables"]
            .as_object()
            .unwrap()
            .is_empty());
    }
}
