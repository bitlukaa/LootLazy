use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
    time::Duration,
};
static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static STARTING: AtomicBool = AtomicBool::new(false);
struct Starting;
impl Drop for Starting {
    fn drop(&mut self) {
        STARTING.store(false, Ordering::SeqCst);
    }
}
const MAX_FRAME: usize = 65536;
#[cfg(steam_engine)]
const PAYLOAD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../data/steam-engine/LootLazy.Steam.exe"
));
#[cfg(not(steam_engine))]
const PAYLOAD: &[u8] = &[];

pub fn active() -> bool {
    ACTIVE.load(Ordering::SeqCst) || STARTING.load(Ordering::SeqCst)
}
pub fn shutdown() {
    // Avoid blocking application exit behind an in-flight login request.
    if let Ok(mut engine) = ENGINE.try_lock() {
        *engine = None;
    }
    ACTIVE.store(false, Ordering::SeqCst);
}
fn checked_directory(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        if let Ok(info) = fs::symlink_metadata(ancestor) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if info.file_attributes() & 0x400 != 0 {
                    return Err("accounts_storage".into());
                }
            }
            if info.file_type().is_symlink() {
                return Err("accounts_storage".into());
            }
        }
    }
    fs::create_dir_all(path).map_err(|_| "accounts_storage".to_string())
}
fn digest_file(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = fs::File::open(path).map_err(|_| "accounts_storage")?;
    let mut digest = Sha256::new();
    let mut bytes = [0u8; 32768];
    loop {
        let count = file.read(&mut bytes).map_err(|_| "accounts_storage")?;
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    Ok(digest.finalize().to_vec())
}
fn clear_runtime(path: &Path) -> Result<(), String> {
    let Ok(info) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if info.file_attributes() & 0x400 != 0 {
            return Err("accounts_storage".into());
        }
    }
    if info.file_type().is_symlink() {
        return Err("accounts_storage".into());
    }
    if info.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| "accounts_storage")? {
            clear_runtime(&entry.map_err(|_| "accounts_storage")?.path())?;
        }
        fs::remove_dir(path).map_err(|_| "accounts_storage".to_string())
    } else {
        fs::remove_file(path).map_err(|_| "accounts_storage".to_string())
    }
}
fn materialize(root: &Path) -> Result<PathBuf, String> {
    if PAYLOAD.is_empty() {
        return Err("accounts_engine_missing".into());
    }
    let folder = root.join("data/steam-engine");
    checked_directory(&folder)?;
    let executable = folder.join("LootLazy.Steam.exe");
    if let Ok(info) = fs::symlink_metadata(&executable) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if info.file_attributes() & 0x400 != 0 {
                return Err("accounts_storage".into());
            }
        }
        if !info.is_file() || info.file_type().is_symlink() {
            return Err("accounts_storage".into());
        }
        if info.len() == PAYLOAD.len() as u64
            && digest_file(&executable)? == Sha256::digest(PAYLOAD).as_slice()
        {
            return Ok(executable);
        }
    }
    // The engine has not been started yet; remove only its previous private runtime extraction.
    clear_runtime(&folder.join("runtime"))?;
    let temporary = folder.join(format!("engine-{:016x}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "accounts_storage")?;
        file.write_all(PAYLOAD).map_err(|_| "accounts_storage")?;
        file.sync_all().map_err(|_| "accounts_storage")?;
        drop(file);
        if executable.exists() {
            fs::remove_file(&executable).map_err(|_| "accounts_storage")?;
        }
        fs::rename(&temporary, &executable).map_err(|_| "accounts_storage")?;
        Ok(executable)
    })();
    if temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    result
}
struct Engine {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<Value>,
    sequence: u64,
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        ACTIVE.store(false, Ordering::SeqCst);
    }
}
impl Engine {
    fn spawn(root: &Path) -> Result<Self, String> {
        let executable = materialize(root)?;
        let state = root.join("data/steam-accounts");
        checked_directory(&state)?;
        let runtime = root.join("data/steam-engine/runtime");
        checked_directory(&runtime)?;
        let mut command = Command::new(executable);
        command
            .args([
                "--bridge",
                state.to_str().ok_or("accounts_storage")?,
                &std::process::id().to_string(),
            ])
            .current_dir(&state)
            .env("DOTNET_BUNDLE_EXTRACT_BASE_DIR", runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| "accounts_engine_failed")?;
        let stdin = child.stdin.take().ok_or("accounts_engine_failed")?;
        let stdout = child.stdout.take().ok_or("accounts_engine_failed")?;
        let (sender, responses) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut frame = Vec::new();
                let result = (&mut reader)
                    .take((MAX_FRAME + 1) as u64)
                    .read_until(b'\n', &mut frame);
                if !matches!(result, Ok(count) if count > 0 && count <= MAX_FRAME) {
                    break;
                }
                let Ok(value) = serde_json::from_slice(&frame) else {
                    break;
                };
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        let engine = Self {
            child,
            stdin,
            responses,
            sequence: 0,
        };
        let ready = engine
            .responses
            .recv_timeout(Duration::from_secs(90))
            .map_err(|_| "accounts_engine_failed")?;
        if ready["ready"] != true || ready["protocol"] != 1 {
            return Err("accounts_protocol".into());
        }
        Ok(engine)
    }
    fn request(&mut self, command: &str, body: Value) -> Result<Value, String> {
        self.sequence += 1;
        let mut bytes = serde_json::to_vec(
            &json!({"protocol":1,"id":self.sequence,"command":command,"body":body}),
        )
        .map_err(|_| "accounts_invalid")?;
        if bytes.len() > 16000 {
            return Err("accounts_invalid".into());
        }
        bytes.push(b'\n');
        let write = self
            .stdin
            .write_all(&bytes)
            .and_then(|_| self.stdin.flush());
        bytes.fill(0);
        write.map_err(|_| "accounts_engine_failed")?;
        let response = self
            .responses
            .recv_timeout(Duration::from_secs(20))
            .map_err(|_| "accounts_engine_failed")?;
        if response["id"].as_u64() != Some(self.sequence) {
            return Err("accounts_protocol".into());
        }
        if response["ok"] != true {
            let error = response["error"].as_str().unwrap_or("accounts_failed");
            return Err(if error.starts_with("accounts_") && error.len() < 80 {
                error.to_string()
            } else {
                "accounts_failed".into()
            });
        }
        let result = response["result"].clone();
        ACTIVE.store(result["active"].as_bool().unwrap_or(true), Ordering::SeqCst);
        Ok(result)
    }
}
pub fn request(root: &Path, command: &str, body: Value) -> Result<Value, String> {
    if ![
        "status",
        "add",
        "remove",
        "games",
        "signin",
        "input",
        "start",
        "stop",
        "library",
        "qr",
        "commit",
        "disconnect",
        "presence",
        "resume",
    ]
    .contains(&command)
    {
        return Err("accounts_invalid".into());
    }
    let mut guard = ENGINE.lock().map_err(|_| "accounts_engine_failed")?;
    if let Some(engine) = guard.as_mut() {
        if engine
            .child
            .try_wait()
            .map_err(|_| "accounts_engine_failed")?
            .is_some()
        {
            *guard = None;
        }
    }
    if guard.is_none() {
        *guard = Some(Engine::spawn(root)?);
    }
    let _starting = if ["signin", "start"].contains(&command) {
        STARTING.store(true, Ordering::SeqCst);
        Some(Starting)
    } else {
        None
    };
    let result = guard.as_mut().unwrap().request(command, body);
    if matches!(
        result.as_ref().err().map(String::as_str),
        Some("accounts_engine_failed" | "accounts_protocol")
    ) {
        *guard = None;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_runtime_cache_is_removed_without_touching_siblings() {
        let root = std::env::temp_dir().join(format!(
            "lootlazy-engine-cache-{:016x}",
            rand::random::<u64>()
        ));
        let runtime = root.join("runtime/old-version");
        fs::create_dir_all(&runtime).unwrap();
        fs::write(runtime.join("fixture.dll"), b"old-runtime").unwrap();
        fs::write(root.join("keep.txt"), b"unrelated").unwrap();
        clear_runtime(&root.join("runtime")).unwrap();
        assert!(!root.join("runtime").exists());
        assert_eq!(fs::read(root.join("keep.txt")).unwrap(), b"unrelated");
        fs::remove_file(root.join("keep.txt")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[cfg(steam_engine)]
    #[test]
    fn embedded_engine_round_trip_uses_isolated_account_state() {
        let root = std::env::temp_dir().join(format!(
            "lootlazy-accounts-native-{:016x}",
            rand::random::<u64>()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut engine = Engine::spawn(&root).unwrap();
        let status = engine.request("status", json!({})).unwrap();
        assert_eq!(status["protocol"], 1);
        assert_eq!(status["accounts"], json!([]));
        assert!(!active());
        drop(engine);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unknown_commands_do_not_start_engine() {
        assert_eq!(
            request(Path::new("unused"), "command", json!({})).unwrap_err(),
            "accounts_invalid"
        );
        assert!(!active());
    }
}
