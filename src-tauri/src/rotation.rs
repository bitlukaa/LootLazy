use crate::storage;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

pub const MAX_ITEMS: usize = 500;
pub struct Rotation {
    pub data: Value,
    path: PathBuf,
    segment: Option<Instant>,
    saved_at: Instant,
    tick_at: Instant,
}
impl Rotation {
    pub fn new(path: PathBuf) -> Self {
        let mut data = storage::read(
            &path,
            json!({"items":[],"index":0,"phase":"idle","error":null}),
        );
        if !data["items"].is_array() {
            data = json!({"items":[],"index":0,"phase":"idle","error":null});
        }
        let items = data["items"].as_array_mut().unwrap();
        items.retain(|item| {
            item.is_object()
                && item["id"].is_string()
                && item["durationMinutes"]
                    .as_u64()
                    .is_some_and(|n| (1..=1440).contains(&n))
        });
        items.truncate(MAX_ITEMS);
        for item in items.iter_mut() {
            if ["running", "starting"].contains(&item["state"].as_str().unwrap_or("")) {
                item["state"] = json!("paused");
            }
            let elapsed = item["elapsedSeconds"].as_u64().unwrap_or(0).min(86400);
            item["elapsedSeconds"] = json!(elapsed);
        }
        if !items.is_empty() && data["phase"] != "completed" && data["phase"] != "idle" {
            data["phase"] = json!("paused");
        }
        Self {
            data,
            path,
            segment: None,
            saved_at: Instant::now(),
            tick_at: Instant::now(),
        }
    }
    pub fn new_variety(path: PathBuf) -> Self {
        let mut rotation = Self::new(path);
        let has_progress = rotation.data["items"].as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["elapsedSeconds"].as_u64().unwrap_or(0) > 0)
        });
        if matches!(rotation.phase(), "idle" | "completed") || !has_progress {
            let minutes = rotation.data["defaultMinutes"].as_u64().unwrap_or(5);
            rotation.data =
                json!({"items":[],"index":0,"phase":"idle","error":null,"defaultMinutes":minutes});
        }
        rotation
    }
    pub fn phase(&self) -> &str {
        self.data["phase"].as_str().unwrap_or("idle")
    }
    pub fn active(&self) -> bool {
        ["running", "starting", "waiting"].contains(&self.phase())
    }
    pub fn index(&self) -> usize {
        self.data["index"].as_u64().unwrap_or(0) as usize
    }
    pub fn current(&self) -> Option<Value> {
        self.data["items"].get(self.index()).cloned()
    }
    pub fn elapsed(&self) -> u64 {
        self.current()
            .map(|i| i["elapsedSeconds"].as_u64().unwrap_or(0))
            .unwrap_or(0)
            + self.segment.map(|s| s.elapsed().as_secs()).unwrap_or(0)
    }
    pub fn remaining(&self) -> u64 {
        self.current()
            .map(|i| i["durationMinutes"].as_u64().unwrap_or(10) * 60)
            .unwrap_or(0)
            .saturating_sub(self.elapsed())
    }
    pub fn replace(&mut self, items: Vec<Value>) -> Result<(), String> {
        if self.active() {
            return Err("A session is already running".into());
        }
        if items.len() > MAX_ITEMS {
            return Err("Maximum 500 games".into());
        }
        let previous = self.data.clone();
        self.data = json!({"items":items,"index":0,"phase":"idle","error":null});
        if let Err(e) = self.save() {
            self.data = previous;
            return Err(e);
        }
        Ok(())
    }
    pub fn prepare(&mut self) -> Result<Value, String> {
        if self.active() {
            return Err("A session is already running".into());
        }
        let previous = self.data.clone();
        let items = self.data["items"].as_array_mut().unwrap();
        if items.is_empty() {
            return Err("Queue is empty".into());
        }
        if items.iter().all(|i| i["state"] == "completed") {
            for item in items.iter_mut() {
                item["state"] = json!("queued");
                item["elapsedSeconds"] = json!(0);
            }
        }
        let index = items
            .iter()
            .position(|i| i["state"] != "completed")
            .unwrap_or(0);
        self.data["index"] = json!(index);
        self.data["phase"] = json!("starting");
        self.data["error"] = Value::Null;
        self.data["items"][index]["state"] = json!("starting");
        if let Err(error) = self.save() {
            self.data = previous;
            return Err(error);
        }
        Ok(self.current().unwrap())
    }
    pub fn running(&mut self) {
        self.data["phase"] = json!("running");
        let i = self.index();
        self.data["items"][i]["state"] = json!("running");
        self.segment = Some(Instant::now());
        self.tick_at = Instant::now();
    }
    pub fn pause(&mut self) -> Result<(), String> {
        let elapsed = self.elapsed();
        self.segment = None;
        let i = self.index();
        if self.current().is_some() && self.data["items"][i]["state"] != "completed" {
            self.data["items"][i]["elapsedSeconds"] = json!(elapsed);
            self.data["items"][i]["state"] = json!("paused");
        }
        if !self.data["items"].as_array().unwrap().is_empty() && self.phase() != "completed" {
            self.data["phase"] = json!("paused");
        }
        self.save()
    }
    pub fn fail(&mut self, error: &str) {
        let _ = self.pause();
        self.data["phase"] = json!("error");
        self.data["error"] = json!(error);
        let _ = self.save();
    }
    pub fn complete(&mut self) -> Result<bool, String> {
        self.segment = None;
        let i = self.index();
        self.data["items"][i]["state"] = json!("completed");
        self.data["items"][i]["elapsedSeconds"] = json!(
            self.data["items"][i]["durationMinutes"]
                .as_u64()
                .unwrap_or(10)
                * 60
        );
        let next = self.data["items"]
            .as_array()
            .unwrap()
            .iter()
            .position(|i| i["state"] != "completed");
        if let Some(index) = next {
            self.data["index"] = json!(index);
            self.data["phase"] = json!("paused");
        } else {
            self.data["phase"] = json!("completed");
        }
        self.save()?;
        Ok(next.is_some())
    }
    pub fn save(&mut self) -> Result<(), String> {
        let mut data = self.data.clone();
        let i = self.index();
        if self.current().is_some() {
            data["items"][i]["elapsedSeconds"] = json!(self.elapsed());
        }
        storage::save(&self.path, &data).map_err(|e| format!("save_failed: {e}"))?;
        self.saved_at = Instant::now();
        Ok(())
    }
    pub fn suspended(&mut self) -> bool {
        let gap = self.tick_at.elapsed();
        self.tick_at = Instant::now();
        if self.active() && self.segment.is_some() && gap > Duration::from_secs(15) {
            let elapsed = self.elapsed().saturating_sub(gap.as_secs());
            self.segment = None;
            let i = self.index();
            self.data["items"][i]["elapsedSeconds"] = json!(elapsed);
            self.data["items"][i]["state"] = json!("paused");
            self.data["phase"] = json!("paused");
            let _ = self.save();
            return true;
        }
        false
    }
    pub fn checkpoint(&mut self) {
        if self.active() && self.saved_at.elapsed() >= Duration::from_secs(5) {
            if let Err(e) = self.save() {
                self.fail(&e);
            }
        }
    }
    pub fn status(&self) -> Value {
        let mut v = self.data.clone();
        v.as_object_mut().unwrap().remove("pool");
        v["active"] = json!(self.active());
        v["elapsedSeconds"] = json!(self.elapsed());
        v["remainingSeconds"] = json!(self.remaining());
        v
    }
    pub fn edit(&mut self, body: &Value) -> Result<(), String> {
        let id = body["id"].as_str().ok_or("Invalid game")?;
        let index = self.data["items"]
            .as_array()
            .unwrap()
            .iter()
            .position(|i| i["id"] == id)
            .ok_or("Game not found")?;
        if (self.active() && index <= self.index())
            || self.data["items"][index]["state"] == "completed"
        {
            return Err("Only upcoming games can be edited".into());
        }
        let previous = self.data.clone();
        match body["action"].as_str().unwrap_or("") {
            "duration" => {
                let duration = minutes(body["durationMinutes"].as_u64().unwrap_or(0))?;
                self.data["items"][index]["durationMinutes"] = json!(duration);
                self.data["items"][index]["elapsedSeconds"] = json!(0);
            }
            "remove" => {
                self.data["items"].as_array_mut().unwrap().remove(index);
                self.data["index"] = json!(self.index().min(
                    self.data["items"]
                        .as_array()
                        .unwrap()
                        .len()
                        .saturating_sub(1)
                ));
            }
            "up" | "down" => {
                let next = if body["action"] == "up" {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(self.data["items"].as_array().unwrap().len() - 1)
                };
                if self.active() && next <= self.index() {
                    return Err("Only upcoming games can be edited".into());
                }
                self.data["items"].as_array_mut().unwrap().swap(index, next);
            }
            _ => return Err("Invalid queue action".into()),
        }
        if let Err(e) = self.save() {
            self.data = previous;
            return Err(e);
        }
        Ok(())
    }
}
pub fn minutes(n: u64) -> Result<u64, String> {
    if (1..=1440).contains(&n) {
        Ok(n)
    } else {
        Err("Choose a duration between one minute and 24 hours".into())
    }
}
pub fn item(game: &Value, duration: u64) -> Value {
    json!({"id":game["id"],"appId":game["id"],"gameName":game["name"],"icon_hash":game["icon_hash"],"cover_image_hash":game["cover_image_hash"],"artwork":game["artwork"],"image":game["image"],"third_party_skus":game["third_party_skus"],"durationMinutes":duration,"state":"queued","elapsedSeconds":0})
}

struct SteamProcess {
    child: Option<Child>,
    ready_file: PathBuf,
    launched: Instant,
    started: Option<Instant>,
}
pub struct SteamIdle {
    total_seconds: u64,
    pub items: Vec<Value>,
    workers: std::collections::HashMap<String, SteamProcess>,
    path: PathBuf,
    saved_at: Instant,
    tick_at: Instant,
}
impl SteamIdle {
    pub fn new(root: &Path) -> Self {
        let path = root.join("data/steam_sessions.json");
        let old = if path.exists() {
            storage::read(&path, json!({"items":[]}))
        } else {
            storage::read(&root.join("data/steam_queue.json"), json!({"items":[]}))
        };
        let mut seen = std::collections::HashSet::new();
        let mut items: Vec<Value> = old["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| {
                item.is_object()
                    && item["steamId"]
                        .as_str()
                        .is_some_and(|id| id.parse::<u32>().is_ok_and(|id| id > 0))
                    && item["durationMinutes"].as_u64().is_some_and(|m| m <= 1440)
            })
            .filter(|item| seen.insert(item["steamId"].as_str().unwrap().to_string()))
            .take(64)
            .cloned()
            .collect();
        for item in &mut items {
            if ["running", "starting", "queued", "waiting"]
                .contains(&item["state"].as_str().unwrap_or(""))
            {
                item["state"] = json!("paused");
            }
            item["elapsedSeconds"] = json!(item["elapsedSeconds"].as_u64().unwrap_or(0));
        }
        let total_seconds = old["totalSeconds"].as_u64().unwrap_or_else(|| {
            items
                .iter()
                .map(|item| item["elapsedSeconds"].as_u64().unwrap_or(0))
                .sum()
        });
        Self {
            total_seconds,
            items,
            workers: std::collections::HashMap::new(),
            path,
            saved_at: Instant::now(),
            tick_at: Instant::now(),
        }
    }
    pub fn total_seconds(&self) -> u64 {
        self.total_seconds
            + self
                .workers
                .values()
                .filter_map(|worker| worker.started)
                .map(|started| started.elapsed().as_secs())
                .sum::<u64>()
    }
    pub fn active(&self) -> bool {
        !self.workers.is_empty()
    }
    fn index(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|item| item["steamId"] == id)
    }
    fn elapsed(&self, id: &str) -> u64 {
        self.index(id)
            .map(|i| self.items[i]["elapsedSeconds"].as_u64().unwrap_or(0))
            .unwrap_or(0)
            + self
                .workers
                .get(id)
                .and_then(|w| w.started)
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0)
    }
    pub fn status(&self) -> Value {
        let items: Vec<Value> = self
            .items
            .iter()
            .map(|item| {
                let mut item = item.clone();
                let id = item["steamId"].as_str().unwrap_or("");
                let elapsed = self.elapsed(id);
                let duration = item["durationMinutes"].as_u64().unwrap_or(0) * 60;
                item["elapsedSeconds"] = json!(elapsed);
                item["remainingSeconds"] = if duration == 0 {
                    Value::Null
                } else {
                    json!(duration.saturating_sub(elapsed))
                };
                item
            })
            .collect();
        json!({"items":items,"totalSeconds":self.total_seconds(),"active":self.active(),"phase":if self.active(){"running"}else{"idle"}})
    }
    pub fn save(&mut self) -> Result<(), String> {
        storage::save(
            &self.path,
            &json!({"items":self.status()["items"],"totalSeconds":self.total_seconds()}),
        )
        .map_err(|e| format!("save_failed: {e}"))?;
        self.saved_at = Instant::now();
        Ok(())
    }
    pub fn start(
        &mut self,
        root: &Path,
        dll: &Path,
        game: &Value,
        duration: u64,
    ) -> Result<(), String> {
        if duration > 1440 {
            return Err("Invalid duration".into());
        }
        let id = game["steamId"]
            .as_str()
            .ok_or("Invalid Steam application")?
            .to_string();
        if !id.parse::<u32>().is_ok_and(|id| id > 0) {
            return Err("Invalid Steam application".into());
        }
        if let Some(i) = self.index(&id) {
            if self.workers.contains_key(&id) {
                if self.items[i]["durationMinutes"] != duration {
                    self.items[i]["durationMinutes"] = json!(duration);
                    self.items[i]["elapsedSeconds"] = json!(0);
                    if let Some(worker) = self.workers.get_mut(&id) {
                        if let Some(started) = worker.started {
                            self.total_seconds += started.elapsed().as_secs();
                            worker.started = Some(Instant::now());
                        }
                    }
                }
                return self.save();
            }
        }
        if self.workers.len() >= 32 {
            return Err("steam_session_limit".into());
        }
        let previous = self.items.clone();
        let index = if let Some(i) = self.index(&id) {
            if self.items[i]["state"] == "completed" || self.items[i]["durationMinutes"] != duration
            {
                self.items[i]["elapsedSeconds"] = json!(0);
            }
            i
        } else {
            if self.items.len() >= 64 {
                return Err("steam_session_limit".into());
            }
            let mut item = item(game, duration);
            item["steamId"] = json!(id);
            self.items.push(item);
            self.items.len() - 1
        };
        self.items[index]["durationMinutes"] = json!(duration);
        self.items[index]["state"] = json!("starting");
        self.items[index]["gameTimeManaged"] = json!(false);
        self.items[index]["error"] = Value::Null;
        if let Err(e) = self.save() {
            self.items = previous;
            return Err(e);
        }
        let result: Result<(), String> = (|| {
            let dir = root.join("data/steam-worker").join(&id);
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let ready = dir.join("status.txt");
            let _ = fs::remove_file(&ready);
            fs::write(dir.join("steam_appid.txt"), &id).map_err(|e| e.to_string())?;
            let (_, runtime_hash) = crate::steam_runtime::lock_verified(dll, None)?;
            let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
            command
                .args(["--steam-session-worker", &id])
                .arg(dll)
                .arg(&ready)
                .arg(std::process::id().to_string())
                .arg(&runtime_hash)
                .current_dir(&dir)
                .env("SteamAppId", &id)
                .env("SteamGameId", &id);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let child = command.spawn().map_err(|e| format!("launch_failed: {e}"))?;
            self.workers.insert(
                id.clone(),
                SteamProcess {
                    child: Some(child),
                    ready_file: ready,
                    launched: Instant::now(),
                    started: None,
                },
            );
            self.tick_at = Instant::now();
            Ok(())
        })();
        if let Err(ref e) = result {
            self.items[index]["state"] = json!("error");
            self.items[index]["error"] = json!(e);
            let _ = self.save();
        }
        result
    }
    fn end(&mut self, id: &str, state: &str, error: Option<&str>) {
        let elapsed = self.elapsed(id);
        if let Some(mut worker) = self.workers.remove(id) {
            if let Some(started) = worker.started {
                self.total_seconds += started.elapsed().as_secs();
            }
            if let Some(mut child) = worker.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Some(i) = self.index(id) {
            self.items[i]["elapsedSeconds"] = json!(elapsed);
            self.items[i]["state"] = json!(state);
            self.items[i]["error"] = error.map(json_value).unwrap_or(Value::Null);
        }
    }
    pub fn pause(&mut self, id: Option<&str>) -> Result<(), String> {
        let ids: Vec<String> = match id {
            Some(id) => vec![id.into()],
            None => self.workers.keys().cloned().collect(),
        };
        for id in ids {
            self.end(&id, "paused", None);
        }
        self.save()
    }
    pub fn stop(&mut self, id: Option<&str>) -> Result<(), String> {
        self.pause(id)?;
        self.items
            .retain(|item| id.is_some_and(|id| item["steamId"] != id));
        self.save()
    }
    pub fn tick(&mut self) {
        let gap = self.tick_at.elapsed();
        self.tick_at = Instant::now();
        if !self.active() {
            return;
        }
        if gap > Duration::from_secs(15) {
            let ids: Vec<_> = self.workers.keys().cloned().collect();
            for id in ids {
                let elapsed = self.elapsed(&id).saturating_sub(gap.as_secs());
                let uncounted_gap = self
                    .workers
                    .get(&id)
                    .and_then(|worker| worker.started)
                    .map(|started| started.elapsed().as_secs().min(gap.as_secs()))
                    .unwrap_or(0);
                self.end(&id, "paused", None);
                self.total_seconds = self.total_seconds.saturating_sub(uncounted_gap);
                if let Some(i) = self.index(&id) {
                    self.items[i]["elapsedSeconds"] = json!(elapsed);
                }
            }
            let _ = self.save();
            return;
        }
        let ids: Vec<_> = self.workers.keys().cloned().collect();
        let mut changed = false;
        for id in ids {
            let mut failure = None;
            let mut ready = false;
            if let Some(worker) = self.workers.get_mut(&id) {
                let text = fs::read_to_string(&worker.ready_file).unwrap_or_default();
                if text.starts_with("error:") {
                    failure = Some(text.trim_start_matches("error:").trim().to_string());
                } else if worker
                    .child
                    .as_mut()
                    .is_some_and(|c| c.try_wait().ok().flatten().is_some())
                {
                    failure = Some("steam_connection_lost".into());
                } else if worker.started.is_none() {
                    if text.trim() == "ready" {
                        worker.started = Some(Instant::now());
                        ready = true;
                    } else if worker.launched.elapsed() > Duration::from_secs(125) {
                        failure = Some("steam_initialization_failed".into());
                    }
                }
            }
            if let Some(error) = failure {
                self.end(&id, "error", Some(&error));
                changed = true;
                continue;
            }
            if ready {
                if let Some(i) = self.index(&id) {
                    self.items[i]["state"] = json!("running");
                }
                changed = true;
            }
            if let Some(i) = self.index(&id) {
                let duration = self.items[i]["durationMinutes"].as_u64().unwrap_or(0) * 60;
                if duration > 0
                    && self.elapsed(&id) >= duration
                    && self.items[i]["gameTimeManaged"] != true
                {
                    self.end(&id, "completed", None);
                    changed = true;
                }
            }
        }
        if changed || self.saved_at.elapsed() >= Duration::from_secs(5) {
            if let Err(e) = self.save() {
                let ids: Vec<_> = self.workers.keys().cloned().collect();
                for id in ids {
                    self.end(&id, "error", Some(&e));
                }
            }
        }
    }
}
fn json_value(value: &str) -> Value {
    json!(value)
}

pub fn libraries(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = crate::executables::detected_roots();
    roots.extend_from_slice(extra);
    let initial = roots.clone();
    for root in initial {
        if let Ok(vdf) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in vdf.lines() {
                let p: Vec<_> = line.split('"').collect();
                if p.get(1) == Some(&"path") {
                    if let Some(path) = p.get(3) {
                        roots.push(path.replace("\\\\", "\\").into());
                    }
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    roots.retain(|p| {
        p.join("steamapps").is_dir() && seen.insert(p.to_string_lossy().to_lowercase())
    });
    roots
}
pub fn discover_runtime(roots: &[PathBuf]) -> Option<PathBuf> {
    for root in roots {
        let mut stack = vec![(root.join("steamapps/common"), 0)];
        let mut count = 0;
        while let Some((dir, depth)) = stack.pop() {
            count += 1;
            if count > 1500 {
                break;
            }
            if let Ok(entries) = fs::read_dir(dir) {
                for e in entries.flatten() {
                    let Ok(kind) = e.file_type() else { continue };
                    if kind.is_symlink() {
                        continue;
                    }
                    let path = e.path();
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        if e.metadata().is_ok_and(|m| m.file_attributes() & 0x400 != 0) {
                            continue;
                        }
                    }
                    if kind.is_file()
                        && e.file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case("steam_api64.dll")
                    {
                        return fs::canonicalize(path).ok();
                    }
                    if kind.is_dir() && depth < 6 {
                        stack.push((path, depth + 1));
                    }
                }
            }
        }
    }
    None
}

pub fn worker(args: &[String]) -> i32 {
    #[cfg(windows)]
    {
        unsafe {
            use std::{ffi::c_void, os::windows::ffi::OsStrExt};
            use windows_sys::Win32::{
                Foundation::{CloseHandle, FreeLibrary, WAIT_TIMEOUT},
                System::{
                    LibraryLoader::{
                        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
                    },
                    Threading::{OpenProcess, WaitForSingleObject},
                },
            };
            if args.len() != 7 {
                return 2;
            }
            let id = match args[2].parse::<u32>() {
                Ok(id) if id > 0 => id,
                _ => return 2,
            };
            let file = Path::new(&args[4]);
            let error = |message: &str| {
                let _ = fs::write(file, format!("error:{message}"));
            };
            let path = match fs::canonicalize(&args[3]) {
                Ok(p)
                    if p.file_name().is_some_and(|n| {
                        n.to_string_lossy().eq_ignore_ascii_case("steam_api64.dll")
                    }) =>
                {
                    p
                }
                _ => {
                    error("steam_runtime_missing");
                    return 2;
                }
            };
            let _runtime_lock = match crate::steam_runtime::lock_verified(&path, Some(&args[6])) {
                Ok((file, _)) => file,
                Err(_) => {
                    error("steam_runtime_untrusted");
                    return 2;
                }
            };
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let library = LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            );
            if library.is_null() {
                error("steam_runtime_missing");
                return 2;
            }
            let init = GetProcAddress(library, c"SteamAPI_Init".as_ptr().cast())
                .or_else(|| GetProcAddress(library, c"SteamAPI_InitSafe".as_ptr().cast()));
            let init_flat = GetProcAddress(library, c"SteamAPI_InitFlat".as_ptr().cast());
            let shutdown = GetProcAddress(library, c"SteamAPI_Shutdown".as_ptr().cast());
            let callbacks = GetProcAddress(library, c"SteamAPI_RunCallbacks".as_ptr().cast());
            let steam = GetProcAddress(library, c"SteamAPI_IsSteamRunning".as_ptr().cast());
            let apps = GetProcAddress(library, c"SteamAPI_SteamApps_v008".as_ptr().cast());
            let subscribed = GetProcAddress(
                library,
                c"SteamAPI_ISteamApps_BIsSubscribedApp".as_ptr().cast(),
            );
            if (init.is_none() && init_flat.is_none())
                || shutdown.is_none()
                || callbacks.is_none()
                || steam.is_none()
                || apps.is_none()
                || subscribed.is_none()
            {
                error("steam_runtime_incompatible");
                FreeLibrary(library);
                return 2;
            }
            let shutdown: unsafe extern "C" fn() = std::mem::transmute(shutdown.unwrap());
            let callbacks: unsafe extern "C" fn() = std::mem::transmute(callbacks.unwrap());
            let steam: unsafe extern "C" fn() -> bool = std::mem::transmute(steam.unwrap());
            let apps: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(apps.unwrap());
            let subscribed: unsafe extern "C" fn(*mut c_void, u32) -> bool =
                std::mem::transmute(subscribed.unwrap());
            let parent_id = args[5].parse::<u32>().unwrap_or(0);
            let parent = OpenProcess(0x00100000, 0, parent_id);
            if parent.is_null() {
                FreeLibrary(library);
                return 2;
            }
            let deadline = Instant::now() + Duration::from_secs(120);
            let mut last_result = 2;
            let _ = fs::write(file, "waiting");
            loop {
                if WaitForSingleObject(parent, 0) != WAIT_TIMEOUT {
                    CloseHandle(parent);
                    FreeLibrary(library);
                    return 0;
                }
                if steam() {
                    let result = if let Some(flat) = init_flat {
                        let initialize: unsafe extern "C" fn(*mut i8) -> i32 =
                            std::mem::transmute(flat);
                        let mut message = [0i8; 1024];
                        initialize(message.as_mut_ptr())
                    } else if let Some(legacy) = init {
                        let initialize: unsafe extern "C" fn() -> bool =
                            std::mem::transmute(legacy);
                        if initialize() {
                            0
                        } else {
                            1
                        }
                    } else {
                        1
                    };
                    if result == 0 {
                        break;
                    }
                    last_result = result;
                    if result == 3 {
                        error("steam_runtime_incompatible");
                        CloseHandle(parent);
                        FreeLibrary(library);
                        return 3;
                    }
                }
                if Instant::now() >= deadline {
                    error(if last_result == 1 {
                        "steam_initialization_failed"
                    } else {
                        "steam_not_ready"
                    });
                    CloseHandle(parent);
                    FreeLibrary(library);
                    return 3;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            let app_interface = apps();
            if app_interface.is_null() || !subscribed(app_interface, id) {
                error("steam_not_owned");
                CloseHandle(parent);
                shutdown();
                FreeLibrary(library);
                return 4;
            }

            let _ = fs::write(file, "ready");
            while WaitForSingleObject(parent, 0) == WAIT_TIMEOUT && steam() {
                callbacks();
                std::thread::sleep(Duration::from_millis(1000));
            }
            let _ = fs::write(file, "error:steam_connection_lost");
            CloseHandle(parent);
            shutdown();
            FreeLibrary(library);
            0
        }
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "questro-rotation-{}-{}.json",
            std::process::id(),
            rand::random::<u64>()
        ))
    }
    #[test]
    fn variety_restart_clears_unstarted_lineup_and_keeps_paused_progress() {
        let file = path();
        let mut rotation = Rotation::new(file.clone());
        rotation
            .replace(vec![item(&json!({"id":"123456","name":"Test"}), 5)])
            .unwrap();
        rotation.data["pool"] = rotation.data["items"].clone();
        rotation.data["defaultMinutes"] = json!(15);
        rotation.save().unwrap();
        let fresh = Rotation::new_variety(file.clone());
        assert_eq!(fresh.data["items"], json!([]));
        assert!(fresh.data["pool"].is_null());
        assert_eq!(fresh.data["defaultMinutes"], 15);
        rotation.prepare().unwrap();
        rotation.data["phase"] = json!("paused");
        rotation.save().unwrap();
        assert_eq!(Rotation::new_variety(file.clone()).data["items"], json!([]));
        rotation.running();
        rotation.segment = Some(Instant::now() - Duration::from_secs(20));
        rotation.save().unwrap();
        let restored = Rotation::new_variety(file.clone());
        assert_eq!(restored.phase(), "paused");
        assert_eq!(restored.data["items"].as_array().unwrap().len(), 1);
        assert!(restored.elapsed() >= 20);
        let _ = fs::remove_file(file);
    }
    #[test]
    fn restore_does_not_run_or_count_offline_time() {
        let file = path();
        let mut r = Rotation::new(file.clone());
        r.replace(vec![item(&json!({"id":"123456","name":"Test"}), 10)])
            .unwrap();
        r.prepare().unwrap();
        r.running();
        r.segment = Some(Instant::now() - Duration::from_secs(20));
        r.save().unwrap();
        let restored = Rotation::new(file.clone());
        assert_eq!(restored.phase(), "paused");
        assert!(!restored.active());
        assert!(restored.elapsed() >= 20 && restored.elapsed() < 22);
        let _ = fs::remove_file(file);
    }
    #[test]
    fn suspension_does_not_count_the_sleep_interval() {
        let file = path();
        let mut r = Rotation::new(file.clone());
        r.replace(vec![item(&json!({"id":"123456"}), 10)]).unwrap();
        r.prepare().unwrap();
        r.running();
        r.segment = Some(Instant::now() - Duration::from_secs(40));
        r.tick_at = Instant::now() - Duration::from_secs(20);
        assert!(r.suspended());
        assert_eq!(r.phase(), "paused");
        assert!(r.elapsed() >= 20 && r.elapsed() < 22);
        let _ = fs::remove_file(file);
    }
    #[test]
    fn parallel_steam_sessions_have_independent_timers() {
        let root =
            std::env::temp_dir().join(format!("questro-steam-test-{}", rand::random::<u64>()));
        fs::create_dir_all(root.join("data")).unwrap();
        let mut steam = SteamIdle::new(&root);
        steam.items = vec![
            json!({"id":"1000","steamId":"1000","durationMinutes":1,"elapsedSeconds":30,"state":"running"}),
            json!({"id":"2000","steamId":"2000","durationMinutes":0,"elapsedSeconds":5,"state":"running"}),
        ];
        for (id, age) in [("1000", 40), ("2000", 10)] {
            steam.workers.insert(
                id.into(),
                SteamProcess {
                    child: None,
                    ready_file: root.join("no-status"),
                    launched: Instant::now(),
                    started: Some(Instant::now() - Duration::from_secs(age)),
                },
            );
        }
        steam.tick();
        assert_eq!(steam.items[0]["state"], "completed");
        assert_eq!(steam.items[1]["state"], "running");
        assert_eq!(steam.workers.len(), 1);
        assert!(steam.status()["items"][1]["remainingSeconds"].is_null());
        steam.stop(Some("1000")).unwrap();
        assert_eq!(steam.workers.len(), 1);
        assert_eq!(steam.items.len(), 1);
        steam.pause(None).unwrap();
        let restored = SteamIdle::new(&root);
        assert_eq!(restored.items[0]["state"], "paused");
        assert!(!restored.active());
        let _ = fs::remove_file(root.join("data/steam_sessions.json"));
        let _ = fs::remove_dir(root.join("data"));
        let _ = fs::remove_dir(root);
    }
    #[test]
    fn steam_clock_waits_for_worker_readiness() {
        let root =
            std::env::temp_dir().join(format!("questro-steam-wait-{}", rand::random::<u64>()));
        fs::create_dir_all(root.join("data")).unwrap();
        let ready = root.join("status.txt");
        fs::write(&ready, "waiting").unwrap();
        let mut steam = SteamIdle::new(&root);
        steam.items = vec![
            json!({"id":"1000","steamId":"1000","durationMinutes":10,"elapsedSeconds":0,"state":"starting"}),
        ];
        steam.workers.insert(
            "1000".into(),
            SteamProcess {
                child: None,
                ready_file: ready.clone(),
                launched: Instant::now() - Duration::from_secs(30),
                started: None,
            },
        );
        steam.tick();
        assert_eq!(steam.elapsed("1000"), 0);
        assert_eq!(steam.items[0]["state"], "starting");
        fs::write(&ready, "ready").unwrap();
        steam.tick();
        assert_eq!(steam.items[0]["state"], "running");
        assert!(steam.workers["1000"].started.is_some());
        steam.pause(None).unwrap();
        let _ = fs::remove_file(ready);
        let _ = fs::remove_file(root.join("data/steam_sessions.json"));
        let _ = fs::remove_dir(root.join("data"));
        let _ = fs::remove_dir(root);
    }
    #[test]
    fn paired_steam_timer_waits_for_its_game_time_owner() {
        let root =
            std::env::temp_dir().join(format!("questro-steam-pair-{}", rand::random::<u64>()));
        fs::create_dir_all(root.join("data")).unwrap();
        let mut steam = SteamIdle::new(&root);
        steam.items = vec![
            json!({"id":"1000","steamId":"1000","durationMinutes":1,"elapsedSeconds":0,"state":"running","gameTimeManaged":true}),
        ];
        steam.workers.insert(
            "1000".into(),
            SteamProcess {
                child: None,
                ready_file: root.join("no-status"),
                launched: Instant::now(),
                started: Some(Instant::now() - Duration::from_secs(75)),
            },
        );
        steam.tick();
        assert_eq!(steam.items[0]["state"], "running");
        steam.pause(None).unwrap();
        assert_eq!(steam.items[0]["state"], "paused");
        let _ = fs::remove_file(root.join("data/steam_sessions.json"));
        let _ = fs::remove_dir(root.join("data"));
        let _ = fs::remove_dir(root);
    }
    #[test]
    fn steam_total_survives_removal_and_restart() {
        let root =
            std::env::temp_dir().join(format!("questro-steam-total-{}", rand::random::<u64>()));
        fs::create_dir_all(root.join("data")).unwrap();
        let mut steam = SteamIdle::new(&root);
        steam.items = vec![
            json!({"id":"1000","steamId":"1000","durationMinutes":0,"elapsedSeconds":0,"state":"running"}),
        ];
        steam.workers.insert(
            "1000".into(),
            SteamProcess {
                child: None,
                ready_file: root.join("no-status"),
                launched: Instant::now(),
                started: Some(Instant::now() - Duration::from_secs(20)),
            },
        );
        steam.stop(Some("1000")).unwrap();
        assert!(steam.items.is_empty());
        let restored = SteamIdle::new(&root);
        assert!((20..=22).contains(&restored.total_seconds()));
        let _ = fs::remove_file(root.join("data/steam_sessions.json"));
        let _ = fs::remove_dir(root.join("data"));
        let _ = fs::remove_dir(root);
    }
    #[test]
    fn replace_while_running_is_rejected() {
        let file = path();
        let mut r = Rotation::new(file.clone());
        r.replace(vec![item(&json!({"id":"123456"}), 10)]).unwrap();
        r.prepare().unwrap();
        assert!(r.replace(vec![]).is_err());
        let _ = fs::remove_file(file);
    }
    #[test]
    fn completing_advances_and_pause_preserves_time() {
        let file = path();
        let mut r = Rotation::new(file.clone());
        r.replace(vec![
            item(&json!({"id":"123456"}), 10),
            item(&json!({"id":"654321"}), 5),
        ])
        .unwrap();
        r.prepare().unwrap();
        r.running();
        r.segment = Some(Instant::now() - Duration::from_secs(25));
        r.pause().unwrap();
        assert!(r.elapsed() >= 25);
        assert!(r.complete().unwrap());
        assert_eq!(r.index(), 1);
        r.prepare().unwrap();
        assert!(!r.complete().unwrap());
        assert_eq!(r.phase(), "completed");
        let _ = fs::remove_file(file);
    }
}

pub fn installed_games(roots: &[PathBuf]) -> Vec<Value> {
    fn field(source: &str, key: &str) -> Option<String> {
        source.lines().find_map(|line| {
            let parts: Vec<_> = line.split('"').collect();
            if parts.get(1) == Some(&key) {
                parts.get(3).map(|v| v.to_string())
            } else {
                None
            }
        })
    }
    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        if let Ok(entries) = fs::read_dir(root.join("steamapps")) {
            for entry in entries.flatten().take(1500) {
                if !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("appmanifest_")
                {
                    continue;
                }
                if entry.metadata().is_ok_and(|m| m.len() > 262144) {
                    continue;
                }
                if let Ok(manifest) = fs::read_to_string(entry.path()) {
                    if let (Some(id), Some(name)) =
                        (field(&manifest, "appid"), field(&manifest, "name"))
                    {
                        if id != "228980"
                            && id.bytes().all(|b| b.is_ascii_digit())
                            && seen.insert(id.clone())
                        {
                            games.push(json!({"id":id,"steamId":id,"name":name,"installed":true,"third_party_skus":[{"distributor":"steam","id":id}]}));
                        }
                    }
                }
            }
        }
    }
    games
}
