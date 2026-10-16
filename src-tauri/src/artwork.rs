use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::Path,
    sync::{mpsc, Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};
type Job = Box<dyn FnOnce() + Send + 'static>;
pub struct DownloadPool {
    sender: mpsc::Sender<Job>,
}
impl DownloadPool {
    pub fn new(workers: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..workers {
            let receiver = receiver.clone();
            std::thread::spawn(move || loop {
                let job = receiver.lock().unwrap().recv();
                match job {
                    Ok(job) => job(),
                    Err(_) => break,
                }
            });
        }
        Self { sender }
    }
    pub fn dispatch(&self, job: impl FnOnce() + Send + 'static) {
        let _ = self.sender.send(Box::new(job));
    }
}
static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
static URL_LOCKS: OnceLock<Mutex<HashMap<String, Weak<Mutex<()>>>>> = OnceLock::new();
static FAILED_URLS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
static TRIM_LOCK: Mutex<()> = Mutex::new(());
pub fn validated(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Invalid artwork URL".into());
    }
    let parts: Vec<_> = url.path().split('/').collect();
    let allowed = match url.host_str() {
        Some("avatars.steamstatic.com") => {
            let name = url.path().strip_prefix('/').unwrap_or("");
            name.strip_suffix("_medium.jpg").is_some_and(|hash| {
                hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        }
        Some("cdn.discordapp.com") => {
            parts.len() >= 4
                && ["app-icons", "app-assets", "quests"].contains(&parts[1])
                && crate::storage::id(parts[2]).is_ok()
        }
        Some(
            "shared.akamai.steamstatic.com"
            | "shared.fastly.steamstatic.com"
            | "cdn.akamai.steamstatic.com"
            | "cdn.cloudflare.steamstatic.com",
        ) => {
            parts.len() >= 6
                && parts[1..4] == ["store_item_assets", "steam", "apps"]
                && parts[4].bytes().all(|b| b.is_ascii_digit())
        }
        _ => false,
    };
    if allowed {
        Ok(url)
    } else {
        Err("Unsupported artwork source".into())
    }
}
fn image_type(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        Ok("png")
    } else if bytes.starts_with(&[255, 216]) {
        Ok("jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok("webp")
    } else {
        Err("Unsupported image format".into())
    }
}
pub fn get(root: &Path, value: &str) -> Result<(Vec<u8>, &'static str), String> {
    let url = validated(value)?;
    let parts: Vec<_> = url.path().split('/').collect();
    let id = if parts.len() >= 6 && parts[1..4] == ["store_item_assets", "steam", "apps"] {
        parts[4].parse::<u32>().ok().filter(|id| *id > 0)
    } else {
        None
    };
    if let Some(id) = id {
        for steam in crate::executables::detected_roots() {
            let cache = steam.join(format!("appcache/librarycache/{id}"));
            let mut files = vec![cache.join("header.jpg")];
            if let Ok(entries) = fs::read_dir(&cache) {
                for entry in entries.flatten().take(64) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if entry
                        .file_type()
                        .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
                        && name.len() == 40
                        && name.bytes().all(|b| b.is_ascii_hexdigit())
                    {
                        files.push(entry.path().join("header.jpg"));
                    }
                }
            }
            files.extend([cache.join("library_600x900.jpg"), cache.join("logo.png")]);
            for file in files {
                if let Ok(meta) = fs::metadata(&file) {
                    if meta.len() <= 4 * 1024 * 1024 {
                        if let Ok(bytes) = fs::read(&file) {
                            if let Ok(kind) = image_type(&bytes) {
                                return Ok((bytes, kind));
                            }
                        }
                    }
                }
            }
        }
    }
    match get_direct(root, value) {
        Ok(image) => Ok(image),
        Err(original) => {
            if let Some(id) = id {
                if let Ok(games) = crate::steam_store::search(&id.to_string()) {
                    if let Some(image) = games
                        .first()
                        .and_then(|game| game["image"].as_str())
                        .filter(|image| *image != value)
                    {
                        if let Ok(image) = get_direct(root, image) {
                            return Ok(image);
                        }
                    }
                }
            }
            Err(original)
        }
    }
}
fn get_direct(root: &Path, value: &str) -> Result<(Vec<u8>, &'static str), String> {
    let url = validated(value)?;
    let key = format!("{:x}", Sha256::digest(url.as_str().as_bytes()));
    {
        let mut failed = FAILED_URLS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| "Artwork unavailable")?;
        failed.retain(|_, when| when.elapsed() < Duration::from_secs(900));
        if failed.contains_key(&key) {
            return Err("Artwork not found".into());
        }
    }
    let lock = {
        let mut locks = URL_LOCKS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| "Artwork unavailable")?;
        locks.retain(|_, value| value.strong_count() > 0);
        if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
            lock
        } else {
            let lock = Arc::new(Mutex::new(()));
            locks.insert(key.clone(), Arc::downgrade(&lock));
            lock
        }
    };
    let _same_url = lock.lock().map_err(|_| "Artwork unavailable")?;
    let dir = root.join("data/artwork");
    for ext in ["png", "jpg", "webp"] {
        if let Ok(bytes) = fs::read(dir.join(format!("{key}.{ext}"))) {
            if let Ok(kind) = image_type(&bytes) {
                return Ok((bytes, kind));
            }
            let _ = fs::remove_file(dir.join(format!("{key}.{ext}")));
        }
    }
    let client = CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let response = client.get(url.clone()).send().map_err(|e| e.to_string())?;
    if [reqwest::StatusCode::NOT_FOUND, reqwest::StatusCode::GONE].contains(&response.status()) {
        let mut failed = FAILED_URLS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| "Artwork unavailable")?;
        if failed.len() >= 1024 {
            failed.clear();
        }
        failed.insert(key, Instant::now());
        return Err("Artwork not found".into());
    }
    let mut response = response.error_for_status().map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    (&mut response)
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Artwork too large".into());
    }
    let kind = image_type(&bytes)?;
    {
        let _cache = TRIM_LOCK.lock().map_err(|_| "Artwork cache unavailable")?;
        if url.path().starts_with("/quests/") && !current_quest_images(root).contains(url.as_str())
        {
            return Ok((bytes, kind));
        }
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let temporary = dir.join(format!("{key}.{kind}.tmp"));
        fs::write(&temporary, &bytes).map_err(|e| e.to_string())?;
        fs::rename(temporary, dir.join(format!("{key}.{kind}"))).map_err(|e| e.to_string())?;
    }
    let _ = trim(
        &dir,
        &image_keys(current_quest_images(root).iter().map(String::as_str)),
    );
    Ok((bytes, kind))
}
fn current_quest_images(root: &Path) -> HashSet<String> {
    let feed = crate::storage::read(&root.join("data/active_quests.json"), serde_json::json!({}));
    feed["quests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|q| crate::feed::current(q))
        .filter_map(|q| q["image"].as_str().map(str::to_string))
        .collect()
}
fn image_keys<'a>(urls: impl Iterator<Item = &'a str>) -> HashSet<String> {
    urls.filter_map(|value| validated(value).ok())
        .filter(|url| url.path().starts_with("/quests/"))
        .map(|url| format!("{:x}", Sha256::digest(url.as_str().as_bytes())))
        .collect()
}
pub fn prune_quests(root: &Path, previous: &[String], keep: &[String]) -> Result<(), String> {
    let dir = root.join("data/artwork");
    if !dir.is_dir() {
        return Ok(());
    }
    let preserved = image_keys(keep.iter().map(String::as_str));
    {
        let _cache = TRIM_LOCK.lock().map_err(|_| "Artwork cache unavailable")?;
        for key in image_keys(previous.iter().map(String::as_str)).difference(&preserved) {
            for ext in ["png", "jpg", "webp"] {
                let file = dir.join(format!("{key}.{ext}"));
                match fs::remove_file(file) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
    }
    trim(&dir, &preserved)
}
fn trim(dir: &Path, preserved: &HashSet<String>) -> Result<(), String> {
    let _trim = TRIM_LOCK.lock().map_err(|_| "Artwork cache unavailable")?;
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let (key, ext) = name.split_once('.')?;
            if key.len() != 64
                || !key.bytes().all(|b| b.is_ascii_hexdigit())
                || !["png", "jpg", "webp"].contains(&ext)
            {
                return None;
            }
            let m = e.metadata().ok()?;
            Some((e.path(), m.len(), m.modified().ok()?))
        })
        .collect();
    entries.sort_by_key(|e| e.2);
    let mut total: u64 = entries.iter().map(|e| e.1).sum();
    for (path, size, _) in entries {
        let key = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let expired = fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(7 * 86400))
            && !preserved.contains(key);
        if total <= 64 * 1024 * 1024 && !expired {
            continue;
        }
        fs::remove_file(path).map_err(|e| e.to_string())?;
        total -= size;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_quest_images_are_deleted_without_touching_shared_images_or_user_files() {
        let root = std::env::temp_dir().join(format!("lootlazy-art-{}", rand::random::<u64>()));
        let dir = root.join("data/artwork");
        fs::create_dir_all(&dir).unwrap();
        let removed = "https://cdn.discordapp.com/quests/123456/old.png".to_string();
        let shared = "https://cdn.discordapp.com/quests/234567/shared.png".to_string();
        let file = |url: &str| dir.join(format!("{:x}.png", Sha256::digest(url.as_bytes())));
        fs::write(file(&removed), b"old").unwrap();
        fs::write(file(&shared), b"shared").unwrap();
        fs::write(dir.join("user-notes.txt"), b"keep").unwrap();
        let orphan = dir.join(format!("{}.jpg", "a".repeat(64)));
        fs::write(&orphan, b"orphan").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&orphan)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::SystemTime::now() - Duration::from_secs(8 * 86400)),
            )
            .unwrap();
        prune_quests(&root, &[removed.clone(), shared.clone()], &[shared.clone()]).unwrap();
        assert!(!file(&removed).exists());
        assert!(file(&shared).exists());
        assert!(dir.join("user-notes.txt").exists());
        assert!(!orphan.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn downloads_have_bounded_concurrency() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let pool = DownloadPool::new(4);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        for _ in 0..12 {
            let active = active.clone();
            let peak = peak.clone();
            let tx = tx.clone();
            pool.dispatch(move || {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
                active.fetch_sub(1, Ordering::SeqCst);
                tx.send(()).unwrap();
            });
        }
        for _ in 0..12 {
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn only_trusted_sources() {
        assert!(validated("https://cdn.discordapp.com/app-icons/123456/game.png").is_ok());
        assert!(validated(
            "https://avatars.steamstatic.com/0123456789abcdef0123456789abcdef01234567_medium.jpg"
        )
        .is_ok());
        for value in ["https://avatars.steamstatic.com/not-a-hash_medium.jpg", "https://avatars.steamstatic.com/0123456789abcdef0123456789abcdef01234567_medium.jpg?token=1", "https://avatars.steamstatic.com.evil.test/0123456789abcdef0123456789abcdef01234567_medium.jpg"] { assert!(validated(value).is_err()); }

        for url in [
            "http://cdn.discordapp.com/app-icons/123456/a.png",
            "https://evil.com/a.png",
            "https://cdn.discordapp.com/app-icons/123456/a.png?token=x",
            "https://x@cdn.discordapp.com/app-icons/123456/a.png",
        ] {
            assert!(validated(url).is_err());
        }
    }
}
