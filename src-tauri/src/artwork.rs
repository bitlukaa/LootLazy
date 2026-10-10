use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::Path,
    sync::{mpsc, Arc, Mutex, OnceLock, Weak},
    time::Duration,
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
        Some("cdn.discordapp.com") => {
            parts.len() >= 4
                && ["app-icons", "app-assets", "quests"].contains(&parts[1])
                && crate::storage::id(parts[2]).is_ok()
        }
        Some("shared.akamai.steamstatic.com") => {
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
    let key = format!("{:x}", Sha256::digest(url.as_str().as_bytes()));
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
    let mut response = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    (&mut response)
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Artwork too large".into());
    }
    let kind = image_type(&bytes)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let temporary = dir.join(format!("{key}.{kind}.tmp"));
    fs::write(&temporary, &bytes).map_err(|e| e.to_string())?;
    fs::rename(temporary, dir.join(format!("{key}.{kind}"))).map_err(|e| e.to_string())?;
    let _ = trim(&dir);
    Ok((bytes, kind))
}
fn trim(dir: &Path) -> Result<(), String> {
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
        if total <= 64 * 1024 * 1024 {
            break;
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
