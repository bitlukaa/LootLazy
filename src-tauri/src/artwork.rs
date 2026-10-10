use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path, time::Duration};
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
    let dir = root.join("data/artwork");
    for ext in ["png", "jpg", "webp"] {
        if let Ok(bytes) = fs::read(dir.join(format!("{key}.{ext}"))) {
            let kind = image_type(&bytes)?;
            return Ok((bytes, kind));
        }
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
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
    fs::write(dir.join(format!("{key}.{kind}")), &bytes).map_err(|e| e.to_string())?;
    let _ = trim(&dir);
    Ok((bytes, kind))
}
fn trim(dir: &Path) -> Result<(), String> {
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
