use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};
static LOG_LOCK: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
pub fn code(error: &str) -> &'static str {
    let text = error.to_lowercase();
    if text.contains("discord_launch_failed") {
        "discord_launch_failed"
    } else if text.contains("save_failed") {
        "save_failed"
    } else if text.contains("access is denied")
        || text.contains("permission denied")
        || text.contains("os error 5")
    {
        "file_access"
    } else if text.contains("missing_executable") {
        "executable_missing"
    } else if text.contains("invalid")
        && (text.contains("executable") || text.contains("filename") || text.contains("windows"))
    {
        "executable_invalid"
    } else if text.contains("no longer available") {
        "quest_unavailable"
    } else if text.contains("locally closed") {
        "quest_closed"
    } else if text.contains("maximum 30") {
        "queue_full"
    } else if text.contains("already running")
        || text.contains("current session")
        || text.contains("another queue")
        || text.contains("stop the quest")
    {
        "game_busy"
    } else if text.contains("process_ended_early") {
        "process_ended_early"
    } else if text.contains("process_launch_failed") || text.contains("launch_failed") {
        "launch_failed"
    } else if text.contains("timed out")
        || text.contains("error sending request")
        || text.contains("dns")
    {
        "network_unavailable"
    } else {
        "operation_failed"
    }
}
pub fn record(root: &Path, route: &str, error: &str) {
    let Ok(mut lock) = LOG_LOCK.lock() else {
        return;
    };
    let last = lock.get_or_insert_with(HashMap::new);
    let route = route.split('?').next().unwrap_or("operation");
    let detail: String = error
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(1024)
        .collect();
    let key = format!("{route}:{detail}");
    if last
        .get(&key)
        .is_some_and(|when| when.elapsed() < Duration::from_secs(60))
    {
        return;
    }
    if last.len() >= 256 {
        last.clear();
    }
    last.insert(key, Instant::now());
    let folder = root.join("data/logs");
    if fs::create_dir_all(&folder).is_err() {
        return;
    }
    let file = folder.join("questro.log");
    if fs::metadata(&file).is_ok_and(|m| m.len() >= 256 * 1024) {
        let previous = folder.join("questro.previous.log");
        let _ = fs::remove_file(&previous);
        if fs::rename(&file, previous).is_err() {
            return;
        }
    }
    if let Ok(mut output) = fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(
            output,
            "{} {} {} {}",
            chrono::Utc::now().to_rfc3339(),
            route,
            code(error),
            detail
        );
    }
}
