//! Desktop Steam achievement management. The worker keeps each AppID in its own process.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
static REQUEST: Mutex<()> = Mutex::new(());
pub fn active() -> bool {
    REQUEST.try_lock().is_err()
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub name: String,
    pub expected: bool,
    pub achieved: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    app_id: u32,
    #[serde(default)]
    steam_id: String,
    #[serde(default)]
    changes: Vec<Change>,
    #[serde(default)]
    catalog: Option<Vec<crate::steam_library::Game>>,
}
fn validate(value: Value, apply: bool) -> Result<Input, String> {
    let input: Input = serde_json::from_value(value).map_err(|_| "achievements_invalid")?;
    if input.catalog.as_ref().is_some_and(|items| {
        items.len() > 100000 || items.iter().any(|g| g.app_id == 0 || g.name.len() > 1024)
    }) {
        return Err("achievements_invalid".into());
    }
    if input.app_id == 0
        || input.changes.len() > 200
        || (!apply && !input.changes.is_empty())
        || (apply && (input.changes.is_empty() || input.steam_id.parse::<u64>().unwrap_or(0) == 0))
    {
        return Err("achievements_invalid".into());
    }
    let mut names = HashSet::new();
    for c in &input.changes {
        if c.name.is_empty()
            || c.name.len() > 256
            || c.name.chars().any(char::is_control)
            || c.expected == c.achieved
            || !names.insert(c.name.clone())
        {
            return Err("achievements_invalid".into());
        }
    }
    Ok(input)
}
fn guarded_dir(path: &Path) -> Result<(), String> {
    for parent in path.ancestors() {
        if let Ok(meta) = fs::symlink_metadata(parent) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("achievements_backup_failed".into());
                }
            }
            if meta.file_type().is_symlink() {
                return Err("achievements_backup_failed".into());
            }
        }
    }
    fs::create_dir_all(path).map_err(|_| "achievements_backup_failed".into())
}
fn exclusive(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| "achievements_backup_failed")?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| "achievements_backup_failed".into())
}
pub fn request(root: &Path, dll: &Path, command: &str, body: Value) -> Result<Value, String> {
    if !["list", "apply"].contains(&command) {
        return Err("achievements_invalid".into());
    }
    let input = validate(body, command == "apply")?;
    if input.catalog.is_some() {
        return Err("achievements_invalid".into());
    }
    let _guard = REQUEST.try_lock().map_err(|_| "achievements_busy")?;
    let (_runtime, hash) = crate::steam_runtime::lock_verified(dll, None)?;
    if command == "apply" {
        let before = run(
            root,
            dll,
            &hash,
            &Input {
                app_id: input.app_id,
                steam_id: String::new(),
                catalog: None,
                changes: vec![],
            },
        )?;
        preflight(&before, &input)?;
        let directory = root.join("data/achievement-backups");
        guarded_dir(&directory)?;
        let original: Vec<Value> = input
            .changes
            .iter()
            .map(|c| json!({"name":c.name,"achieved":c.expected}))
            .collect();
        exclusive(&directory.join(format!("{}-{}.json",input.app_id,rand::random::<u64>())),&serde_json::to_vec(&json!({"appId":input.app_id,"steamId":input.steam_id,"before":original,"changes":input.changes,"savedAt":chrono::Utc::now().to_rfc3339()})).map_err(|_|"achievements_backup_failed")?)?;
        prune_backups(&directory);
    }
    run(root, dll, &hash, &input)
}
pub fn library(root: &Path, dll: &Path, roots: &[PathBuf], seeds: &[u32]) -> Result<Value, String> {
    let catalog = crate::steam_library::read(roots)?;
    let _guard = REQUEST.try_lock().map_err(|_| "achievements_busy")?;
    let (_lock, hash) = crate::steam_runtime::lock_verified(dll, None)?;
    let mut last = "steam_not_ready".to_string();
    for app_id in seeds.iter().take(8) {
        let input = Input {
            app_id: *app_id,
            steam_id: String::new(),
            changes: vec![],
            catalog: Some(catalog.clone()),
        };
        match run(root, dll, &hash, &input) {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !["steam_initialization_failed", "steam_not_owned"].contains(&error.as_str()) {
                    return Err(error);
                }
                last = error;
            }
        }
    }
    Err(last)
}
fn prune_backups(directory: &Path) {
    let mut files: Vec<_> = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).ok()?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || path.extension()?.to_str()? != "json"
            {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            let (app, nonce) = stem.split_once('-')?;
            if app.parse::<u32>().ok()? == 0 || nonce.parse::<u64>().is_err() {
                return None;
            }
            Some((meta.modified().ok()?, path))
        })
        .collect();
    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len().saturating_sub(20);
    for (_, file) in files.into_iter().take(excess) {
        let _ = fs::remove_file(file);
    }
}
fn preflight(snapshot: &Value, input: &Input) -> Result<(), String> {
    if snapshot["steamId"].as_str() != Some(input.steam_id.as_str()) {
        return Err("achievements_account_changed".into());
    }
    for change in &input.changes {
        let actual = snapshot["achievements"]
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item["name"].as_str() == Some(&change.name))
            })
            .and_then(|item| item["achieved"].as_bool());
        if actual != Some(change.expected) {
            return Err("achievements_conflict".into());
        }
    }
    Ok(())
}
fn run(root: &Path, dll: &Path, hash: &str, input: &Input) -> Result<Value, String> {
    let base = root.join("data/achievement-worker");
    guarded_dir(&base)?;
    let dir = base.join(format!("{}-{}", std::process::id(), rand::random::<u64>()));
    fs::create_dir(&dir).map_err(|_| "achievements_failed")?;
    let result = (|| {
        exclusive(
            &dir.join("steam_appid.txt"),
            input.app_id.to_string().as_bytes(),
        )?;
        let output = dir.join("result.json");
        let mut command = Command::new(std::env::current_exe().map_err(|_| "achievements_failed")?);
        command
            .arg("--achievement-worker")
            .arg(dll)
            .arg(hash)
            .arg(std::process::id().to_string())
            .arg(&output)
            .env("SteamAppId", input.app_id.to_string())
            .env("SteamGameId", input.app_id.to_string())
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| "achievements_failed")?;
        let sent = child
            .stdin
            .take()
            .ok_or("achievements_failed")
            .and_then(|mut stream| {
                stream
                    .write_all(&serde_json::to_vec(input).map_err(|_| "achievements_invalid")?)
                    .map_err(|_| "achievements_failed")
            });
        if sent.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("achievements_failed".into());
        }
        let deadline = Instant::now() + Duration::from_secs(35);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("achievements_failed".into());
                }
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("achievements_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        let file = fs::File::open(output).map_err(|_| "achievements_failed")?;
        if file.metadata().map_err(|_| "achievements_failed")?.len() > 64 * 1024 * 1024 {
            return Err("achievements_failed".into());
        }
        let response: Value = serde_json::from_reader(file).map_err(|_| "achievements_failed")?;
        if let Some(error) = response["error"].as_str() {
            return Err(error.to_string());
        }
        Ok(response)
    })();
    let _ = fs::remove_dir_all(&dir);
    result
}
// SteamID is nested at offset 12 despite the callback's trailing 8-byte padding.
fn callback_result(id: i32, bytes: &[u8], app: u32, steam: u64) -> Option<Result<(), String>> {
    let required = if id == 1101 {
        24
    } else if id == 1102 {
        12
    } else {
        return None;
    };
    if bytes.len() < required {
        return None;
    }
    let game = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
    if game as u32 != app {
        return None;
    }
    if id == 1101 && u64::from_le_bytes(bytes[12..20].try_into().ok()?) != steam {
        return None;
    }
    Some(if u32::from_le_bytes(bytes[8..12].try_into().ok()?) == 1 {
        Ok(())
    } else {
        Err(if id == 1101 {
            match u32::from_le_bytes(bytes[8..12].try_into().unwrap()) {
                15 => "achievements_access_denied",
                2 | 9 => "achievements_stats_unavailable",
                _ => "achievements_read_failed",
            }
        } else {
            "achievements_store_failed"
        }
        .into())
    })
}
pub fn worker(args: &[String]) -> i32 {
    if args.len() != 6 {
        return 2;
    }
    let result = (|| {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "achievements_invalid")?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("achievements_invalid".into());
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "achievements_invalid")?;
        let apply = value["changes"].as_array().is_some_and(|c| !c.is_empty());
        let input = validate(value, apply)?;
        #[cfg(windows)]
        {
            unsafe { native::execute(args, &input) }
        }
        #[cfg(not(windows))]
        {
            let _ = input;
            Err("achievements_not_supported".into())
        }
    })();
    let response = match result {
        Ok(value) => value,
        Err(error) => json!({"error":error}),
    };
    if exclusive(
        Path::new(&args[5]),
        &serde_json::to_vec(&response).unwrap_or_default(),
    )
    .is_ok()
    {
        0
    } else {
        2
    }
}
#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        ffi::{c_char, c_void, CString},
        os::windows::ffi::OsStrExt,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE, WAIT_TIMEOUT},
        System::{
            LibraryLoader::{
                GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            },
            Threading::{OpenProcess, WaitForSingleObject},
        },
    };
    struct Runtime {
        library: HMODULE,
        parent: HANDLE,
        shutdown: Option<unsafe extern "C" fn()>,
    }
    impl Drop for Runtime {
        fn drop(&mut self) {
            unsafe {
                if let Some(shutdown) = self.shutdown {
                    shutdown();
                }
                if !self.parent.is_null() {
                    CloseHandle(self.parent);
                }
                FreeLibrary(self.library);
            }
        }
    }
    impl Runtime {
        unsafe fn function<T: Copy>(&self, name: &str) -> Result<T, String> {
            let symbol = CString::new(name).map_err(|_| "achievements_not_supported")?;
            let address = GetProcAddress(self.library, symbol.as_ptr().cast())
                .ok_or_else(|| format!("achievements_not_supported:{name}"))?;
            Ok(std::mem::transmute_copy(&address))
        }
        unsafe fn interface(&self, names: &[&str]) -> Result<*mut c_void, String> {
            for name in names {
                if let Ok(get) = self.function::<unsafe extern "C" fn() -> *mut c_void>(name) {
                    let pointer = get();
                    if !pointer.is_null() {
                        return Ok(pointer);
                    }
                }
            }
            Err("achievements_not_supported".into())
        }
        unsafe fn alive(&self) -> Result<(), String> {
            if WaitForSingleObject(self.parent, 0) == WAIT_TIMEOUT {
                Ok(())
            } else {
                Err("achievements_failed".into())
            }
        }
    }
    #[repr(C)]
    struct Callback {
        user: i32,
        id: i32,
        data: *mut c_void,
        size: i32,
    }
    struct Api<'a> {
        runtime: &'a Runtime,
        stats: *mut c_void,
        user: *mut c_void,
        steam: u64,
        app: u32,
        pipe: i32,
    }
    impl Api<'_> {
        unsafe fn account(&self) -> Result<(), String> {
            self.runtime.alive()?;
            let get: unsafe extern "C" fn(*mut c_void) -> u64 =
                self.runtime.function("SteamAPI_ISteamUser_GetSteamID")?;
            if get(self.user) == self.steam {
                Ok(())
            } else {
                Err("achievements_account_changed".into())
            }
        }
        unsafe fn wait(&self, id: i32, call: u64) -> Result<(), String> {
            let run: unsafe extern "C" fn(i32) =
                self.runtime.function("SteamAPI_ManualDispatch_RunFrame")?;
            let next: unsafe extern "C" fn(i32, *mut Callback) -> bool = self
                .runtime
                .function("SteamAPI_ManualDispatch_GetNextCallback")?;
            let free: unsafe extern "C" fn(i32) = self
                .runtime
                .function("SteamAPI_ManualDispatch_FreeLastCallback")?;
            let result: unsafe extern "C" fn(i32, u64, *mut c_void, i32, i32, *mut bool) -> bool =
                self.runtime
                    .function("SteamAPI_ManualDispatch_GetAPICallResult")?;
            let deadline = Instant::now() + Duration::from_secs(12);
            while Instant::now() < deadline {
                self.account()?;
                run(self.pipe);
                let mut callback = Callback {
                    user: 0,
                    id: 0,
                    data: std::ptr::null_mut(),
                    size: 0,
                };
                while next(self.pipe, &mut callback) {
                    let outcome = if callback.id == id
                        && !callback.data.is_null()
                        && callback.size >= 0
                        && callback.size <= 4096
                    {
                        callback_result(
                            id,
                            std::slice::from_raw_parts(
                                callback.data.cast::<u8>(),
                                callback.size as usize,
                            ),
                            self.app,
                            self.steam,
                        )
                    } else if call != 0
                        && callback.id == 703
                        && !callback.data.is_null()
                        && callback.size >= 16
                    {
                        let complete = std::slice::from_raw_parts(callback.data.cast::<u8>(), 16);
                        if u64::from_le_bytes(complete[..8].try_into().unwrap()) == call
                            && i32::from_le_bytes(complete[8..12].try_into().unwrap()) == id
                        {
                            let mut payload = [0u8; 24];
                            let mut failed = false;
                            if result(
                                self.pipe,
                                call,
                                payload.as_mut_ptr().cast(),
                                24,
                                id,
                                &mut failed,
                            ) && !failed
                            {
                                callback_result(id, &payload, self.app, self.steam)
                            } else {
                                Some(Err("achievements_read_failed".into()))
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    free(self.pipe);
                    if let Some(result) = outcome {
                        return result;
                    }
                }
                std::thread::sleep(Duration::from_millis(30));
            }
            Err("achievements_timeout".into())
        }
        unsafe fn icons(&self, items: &mut [Value]) {
            use base64::Engine;
            let Ok(utils) = self
                .runtime
                .interface(&["SteamAPI_SteamUtils_v010", "SteamAPI_SteamUtils_v009"])
            else {
                return;
            };
            let Ok(get) = self
                .runtime
                .function::<unsafe extern "C" fn(*mut c_void, *const c_char) -> i32>(
                    "SteamAPI_ISteamUserStats_GetAchievementIcon",
                )
            else {
                return;
            };
            let Ok(size) =
                self.runtime
                    .function::<unsafe extern "C" fn(*mut c_void, i32, *mut u32, *mut u32) -> bool>(
                        "SteamAPI_ISteamUtils_GetImageSize",
                    )
            else {
                return;
            };
            let Ok(rgba) = self
                .runtime
                .function::<unsafe extern "C" fn(*mut c_void, i32, *mut u8, i32) -> bool>(
                    "SteamAPI_ISteamUtils_GetImageRGBA",
                )
            else {
                return;
            };
            // Trigger all downloads first; one bounded wait covers the whole collection.
            for item in items.iter() {
                if let Ok(name) = CString::new(item["name"].as_str().unwrap_or("")) {
                    get(self.stats, name.as_ptr());
                }
            }
            let until = Instant::now() + Duration::from_millis(1200);
            loop {
                if self.account().is_err() {
                    return;
                }
                let mut missing = false;
                for item in items.iter_mut().filter(|item| item.get("icon").is_none()) {
                    let Ok(name) = CString::new(item["name"].as_str().unwrap_or("")) else {
                        continue;
                    };
                    let handle = get(self.stats, name.as_ptr());
                    if handle <= 0 {
                        missing = true;
                        continue;
                    }
                    let (mut w, mut h) = (0u32, 0u32);
                    if !size(utils, handle, &mut w, &mut h)
                        || w == 0
                        || h == 0
                        || w > 512
                        || h > 512
                    {
                        continue;
                    }
                    let mut pixels = vec![0u8; (w * h * 4) as usize];
                    if !rgba(utils, handle, pixels.as_mut_ptr(), pixels.len() as i32) {
                        continue;
                    }
                    let mut thumbnail = vec![0u8; 32 * 32 * 4];
                    for y in 0..32usize {
                        for x in 0..32usize {
                            let source =
                                ((y * h as usize / 32) * w as usize + x * w as usize / 32) * 4;
                            thumbnail[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
                                .copy_from_slice(&pixels[source..source + 4]);
                        }
                    }
                    let mut bytes = Vec::new();
                    let mut encoder = png::Encoder::new(&mut bytes, 32, 32);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    if let Ok(mut writer) = encoder.write_header() {
                        if writer.write_image_data(&thumbnail).is_err() {
                            continue;
                        }
                    }
                    if !bytes.is_empty() && bytes.len() < 6144 {
                        item["icon"] = json!(format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        ));
                    }
                }
                if !missing || Instant::now() >= until {
                    break;
                }
                // Pump image-ready callbacks without mixing them with stats acknowledgments.
                let Ok(run) = self
                    .runtime
                    .function::<unsafe extern "C" fn(i32)>("SteamAPI_ManualDispatch_RunFrame")
                else {
                    break;
                };
                run(self.pipe);
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        unsafe fn snapshot(&self) -> Result<Value, String> {
            self.account()?;
            let count: unsafe extern "C" fn(*mut c_void) -> u32 = self
                .runtime
                .function("SteamAPI_ISteamUserStats_GetNumAchievements")?;
            let name: unsafe extern "C" fn(*mut c_void, u32) -> *const c_char = self
                .runtime
                .function("SteamAPI_ISteamUserStats_GetAchievementName")?;
            let attr: unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
            ) -> *const c_char = self
                .runtime
                .function("SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute")?;
            let state: unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *mut bool,
                *mut u32,
            ) -> bool = self
                .runtime
                .function("SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime")?;
            let n = count(self.stats);
            if n > 5000 {
                return Err("achievements_read_failed".into());
            }
            let mut achievements = Vec::new();
            for i in 0..n {
                let key = bounded(name(self.stats, i), 256)?;
                if key.is_empty() {
                    return Err("achievements_read_failed".into());
                }
                let c = CString::new(key.clone()).map_err(|_| "achievements_read_failed")?;
                let mut achieved = false;
                let mut unlocked = 0;
                if !state(self.stats, c.as_ptr(), &mut achieved, &mut unlocked) {
                    return Err("achievements_read_failed".into());
                }
                let title = bounded(attr(self.stats, c.as_ptr(), c"name".as_ptr()), 512)?;
                let description = bounded(attr(self.stats, c.as_ptr(), c"desc".as_ptr()), 2048)?;
                achievements.push(json!({"name":key,"title":if title.is_empty(){key.clone()}else{title},"description":description,"achieved":achieved,"unlockedAt":unlocked}));
            }
            self.icons(&mut achievements);
            self.account()?;
            Ok(
                json!({"appId":self.app,"steamId":self.steam.to_string(),"achievements":achievements}),
            )
        }
    }
    unsafe fn bounded(pointer: *const c_char, max: usize) -> Result<String, String> {
        if pointer.is_null() {
            return Ok(String::new());
        }
        let mut bytes = Vec::new();
        for n in 0..=max {
            let byte = *pointer.add(n) as u8;
            if byte == 0 {
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            if n == max {
                return Err("achievements_read_failed".into());
            }
            bytes.push(byte);
        }
        Err("achievements_read_failed".into())
    }
    pub unsafe fn execute(args: &[String], input: &Input) -> Result<Value, String> {
        let path = fs::canonicalize(&args[2]).map_err(|_| "steam_runtime_missing")?;
        if !path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case("steam_api64.dll")
        }) {
            return Err("steam_runtime_missing".into());
        }
        let (_lock, _) = crate::steam_runtime::lock_verified(&path, Some(&args[3]))?;
        execute_loaded(args, input, &path)
    }
    pub(super) unsafe fn execute_loaded(
        args: &[String],
        input: &Input,
        path: &Path,
    ) -> Result<Value, String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let library = LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        );
        if library.is_null() {
            return Err("steam_runtime_missing".into());
        }
        let mut runtime = Runtime {
            library,
            parent: std::ptr::null_mut(),
            shutdown: None,
        };
        runtime.parent = OpenProcess(
            0x00100000,
            0,
            args[4].parse().map_err(|_| "achievements_invalid")?,
        );
        if runtime.parent.is_null() {
            return Err("achievements_failed".into());
        }
        let shutdown = runtime.function::<unsafe extern "C" fn()>("SteamAPI_Shutdown")?;
        let initialized = if let Ok(init) =
            runtime.function::<unsafe extern "C" fn(*mut i8) -> i32>("SteamAPI_InitFlat")
        {
            let mut message = [0i8; 1024];
            init(message.as_mut_ptr()) == 0
        } else {
            let init: unsafe extern "C" fn() -> bool = runtime
                .function("SteamAPI_Init")
                .or_else(|_| runtime.function("SteamAPI_InitSafe"))?;
            init()
        };
        if !initialized {
            return Err("steam_initialization_failed".into());
        }
        runtime.shutdown = Some(shutdown);
        let dispatch: unsafe extern "C" fn() = runtime.function("SteamAPI_ManualDispatch_Init")?;
        dispatch();
        let pipe: unsafe extern "C" fn() -> i32 = runtime.function("SteamAPI_GetHSteamPipe")?;

        let user = runtime.interface(&[
            "SteamAPI_SteamUser_v023",
            "SteamAPI_SteamUser_v022",
            "SteamAPI_SteamUser_v021",
            "SteamAPI_SteamUser_v020",
        ])?;
        let apps = runtime.interface(&["SteamAPI_SteamApps_v008"])?;
        let owned: unsafe extern "C" fn(*mut c_void, u32) -> bool =
            runtime.function("SteamAPI_ISteamApps_BIsSubscribedApp")?;
        if !owned(apps, input.app_id) {
            return Err("steam_not_owned".into());
        }
        let steam_id: unsafe extern "C" fn(*mut c_void) -> u64 =
            runtime.function("SteamAPI_ISteamUser_GetSteamID")?;
        let steam = steam_id(user);
        if steam == 0 {
            return Err("steam_not_ready".into());
        }
        if let Some(catalog) = &input.catalog {
            let mut games = Vec::new();
            for game in catalog {
                runtime.alive()?;
                if owned(apps, game.app_id) {
                    games.push(game.clone());
                }
            }
            if steam_id(user) != steam {
                return Err("achievements_account_changed".into());
            }
            games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            return Ok(
                json!({"games":games,"steamId":steam.to_string(),"source":"desktop_library_cache"}),
            );
        }
        let stats = runtime.interface(&[
            "SteamAPI_SteamUserStats_v013",
            "SteamAPI_SteamUserStats_v012",
        ])?;
        if !input.changes.is_empty() && input.steam_id != steam.to_string() {
            return Err("achievements_account_changed".into());
        }
        let api = Api {
            runtime: &runtime,
            stats,
            user,
            steam,
            app: input.app_id,
            pipe: pipe(),
        };
        // Older runtimes require the local-user request; it is absent in v013.
        if let Ok(current) = runtime.function::<unsafe extern "C" fn(*mut c_void) -> bool>(
            "SteamAPI_ISteamUserStats_RequestCurrentStats",
        ) {
            if !current(stats) {
                return Err("steam_not_ready".into());
            }
            api.wait(1101, 0)?;
        } else {
            let request: unsafe extern "C" fn(*mut c_void, u64) -> u64 =
                runtime.function("SteamAPI_ISteamUserStats_RequestUserStats")?;
            let call = request(stats, steam);
            if call == 0 {
                return Err("achievements_stats_unavailable".into());
            }
            api.wait(1101, call)?;
        }
        let snapshot = api.snapshot()?;
        if input.changes.is_empty() {
            return Ok(snapshot);
        }
        preflight(&snapshot, input)?;
        let set: unsafe extern "C" fn(*mut c_void, *const c_char) -> bool =
            runtime.function("SteamAPI_ISteamUserStats_SetAchievement")?;
        let clear: unsafe extern "C" fn(*mut c_void, *const c_char) -> bool =
            runtime.function("SteamAPI_ISteamUserStats_ClearAchievement")?;
        let store: unsafe extern "C" fn(*mut c_void) -> bool =
            runtime.function("SteamAPI_ISteamUserStats_StoreStats")?;
        let mut changed: Vec<&Change> = Vec::new();
        for change in &input.changes {
            api.account()?;
            let name = CString::new(change.name.clone()).map_err(|_| "achievements_invalid")?;
            if !(if change.achieved {
                set(stats, name.as_ptr())
            } else {
                clear(stats, name.as_ptr())
            }) {
                for before in changed {
                    let key =
                        CString::new(before.name.clone()).map_err(|_| "achievements_invalid")?;
                    if before.expected {
                        set(stats, key.as_ptr());
                    } else {
                        clear(stats, key.as_ptr());
                    }
                }
                return Err("achievements_store_failed".into());
            }
            changed.push(change);
        }
        api.account()?;
        if !store(stats) {
            return Err("achievements_store_failed".into());
        }
        api.wait(1102, 0)?;
        let final_state = api.snapshot()?;
        if input.changes.iter().any(|c| {
            !final_state["achievements"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["name"] == c.name && item["achieved"] == c.achieved)
            })
        }) {
            return Err("achievements_store_failed".into());
        }
        Ok(final_state)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_before_any_worker_or_writes() {
        for value in [
            json!({"appId":0}),
            json!({"appId":1,"steamId":"0","changes":[{"name":"a","expected":false,"achieved":true}]}),
            json!({"appId":1,"steamId":"1","changes":[{"name":"a","expected":false,"achieved":true},{"name":"a","expected":false,"achieved":true}]}),
            json!({"appId":1,"steamId":"1","changes":[{"name":"a\u{0}","expected":false,"achieved":true}]}),
        ] {
            assert!(validate(value, true).is_err());
        }
    }
    #[test]
    fn rejects_stale_account_and_achievement_state() {
        let input=validate(json!({"appId":1,"steamId":"7","changes":[{"name":"A","expected":false,"achieved":true}]}),true).unwrap();
        assert_eq!(
            preflight(&json!({"steamId":"8"}), &input),
            Err("achievements_account_changed".into())
        );
        assert_eq!(
            preflight(
                &json!({"steamId":"7","achievements":[{"name":"A","achieved":true}]}),
                &input
            ),
            Err("achievements_conflict".into())
        );
        assert!(preflight(
            &json!({"steamId":"7","achievements":[{"name":"A","achieved":false}]}),
            &input
        )
        .is_ok());
    }
    #[test]
    fn store_requires_matching_success_callback() {
        let mut data = vec![0u8; 24];
        data[..8].copy_from_slice(&5u64.to_le_bytes());
        data[8..12].copy_from_slice(&1u32.to_le_bytes());
        data[12..20].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(callback_result(1101, &data, 5, 7), Some(Ok(())));
        assert_eq!(callback_result(1101, &data, 5, 8), None);
        assert_eq!(callback_result(1102, &data, 6, 7), None);
        assert_eq!(callback_result(1102, &data[..8], 5, 7), None);
        data[8] = 2;
        assert_eq!(
            callback_result(1102, &data, 5, 7),
            Some(Err("achievements_store_failed".into()))
        );
    }
    #[cfg(windows)]
    #[test]
    fn flat_api_fixture_requires_confirmed_saves_and_the_same_account() {
        let dir = std::env::temp_dir().join(format!(
            "lootlazy-achievement-fixture-{}",
            rand::random::<u64>()
        ));
        fs::create_dir(&dir).unwrap();
        let dll = dir.join("steam_api64.dll");
        let compiler = std::env::var_os("RUSTC")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap())
                    .join(".cargo/bin/rustc.exe")
            });
        let status = Command::new(compiler)
            .args(["--crate-type=cdylib", "--edition=2021", "-O"])
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/steam-achievements.rs"),
            )
            .arg("-o")
            .arg(&dll)
            .status()
            .unwrap();
        assert!(status.success());
        let args = vec![
            String::new(),
            "--achievement-worker".into(),
            dll.to_string_lossy().into_owned(),
            String::new(),
            std::process::id().to_string(),
            String::new(),
        ];
        let list = Input {
            app_id: 5,
            steam_id: String::new(),
            changes: vec![],
            catalog: None,
        };
        let snapshot = unsafe { native::execute_loaded(&args, &list, &dll) }.unwrap();
        assert_eq!(snapshot["steamId"], "7");
        assert!(snapshot["achievements"][0]["icon"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,iVBOR"));
        assert_eq!(snapshot["achievements"][0]["achieved"], false);
        let mut apply = Input {
            catalog: None,
            app_id: 5,
            steam_id: "7".into(),
            changes: vec![
                Change {
                    name: "A".into(),
                    expected: false,
                    achieved: true,
                },
                Change {
                    name: "B".into(),
                    expected: true,
                    achieved: false,
                },
            ],
        };
        let saved = unsafe { native::execute_loaded(&args, &apply, &dll) }.unwrap();
        assert_eq!(saved["achievements"][0]["achieved"], true);
        assert_eq!(saved["achievements"][1]["achieved"], false);
        let library = Input {
            app_id: 5,
            steam_id: String::new(),
            changes: vec![],
            catalog: Some(vec![
                crate::steam_library::Game {
                    app_id: 42,
                    name: "Owned".into(),
                    risky: true,
                    risk_reason: "multiplayer".into(),
                },
                crate::steam_library::Game {
                    app_id: 9999,
                    name: "Not owned".into(),
                    risky: false,
                    risk_reason: "unknown".into(),
                },
            ]),
        };
        let result = unsafe { native::execute_loaded(&args, &library, &dll) }.unwrap();
        assert_eq!(result["games"].as_array().unwrap().len(), 1);
        assert_eq!(result["games"][0]["appId"], 42);
        apply.app_id = 6;
        assert_eq!(
            unsafe { native::execute_loaded(&args, &apply, &dll) },
            Err("achievements_store_failed".into())
        );
        apply.app_id = 9;
        assert_eq!(
            unsafe { native::execute_loaded(&args, &apply, &dll) },
            Err("achievements_store_failed".into())
        );
        apply.steam_id = "8".into();
        assert_eq!(
            unsafe { native::execute_loaded(&args, &apply, &dll) },
            Err("achievements_account_changed".into())
        );
        apply.steam_id = "7".into();
        apply.changes[0].expected = true;
        assert_eq!(
            unsafe { native::execute_loaded(&args, &apply, &dll) },
            Err("achievements_conflict".into())
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
