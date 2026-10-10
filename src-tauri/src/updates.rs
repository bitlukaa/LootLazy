use serde_json::{json, Value};
use std::{
    io::Read,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
const RELEASES: &str = "https://github.com/bitlukaa/Questro/releases";
const ENDPOINT: &str = "https://api.github.com/repos/bitlukaa/Questro/releases/latest";
static CACHE: OnceLock<Mutex<Option<(Instant, Value)>>> = OnceLock::new();
pub fn info() -> Value {
    json!({"currentVersion":env!("CARGO_PKG_VERSION"),"currentNotes":serde_json::from_str::<Value>(include_str!("../../public/release-notes.json")).unwrap_or(json!({})),"status":"idle","releasesUrl":RELEASES})
}
fn parse_release(release: &Value, current: &str) -> Result<Value, String> {
    if release["draft"] == true || release["prerelease"] == true {
        return Err("Unstable release".into());
    }
    let tag = release["tag_name"]
        .as_str()
        .ok_or("Missing release version")?;
    let version = semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| "Invalid release version")?;
    if !version.pre.is_empty() {
        return Err("Unstable release".into());
    }
    let expected = format!("Questro-{version}.zip");
    let asset = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"] == expected)
        .ok_or("Portable release unavailable")?;
    let url = url::Url::parse(
        asset["browser_download_url"]
            .as_str()
            .ok_or("Missing download")?,
    )
    .map_err(|_| "Invalid download")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .path()
            .starts_with("/bitlukaa/Questro/releases/download/")
        || !url.path().ends_with(&format!("/{expected}"))
    {
        return Err("Invalid download".into());
    }
    let current = semver::Version::parse(current).map_err(|_| "Invalid current version")?;
    Ok(
        json!({"status":if version>current {"available"}else{"up_to_date"},"latestVersion":version.to_string(),"notes":release["body"].as_str().unwrap_or("").chars().take(8000).collect::<String>(),"downloadUrl":url.as_str()}),
    )
}
fn fetch() -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("Questro/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(ENDPOINT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?;
    match response.status().as_u16() {
        404 => return Ok(json!({"status":"restricted"})),
        403 | 429 => return Ok(json!({"status":"rate_limited"})),
        200 => {}
        _ => return Err("Update source unavailable".into()),
    }
    let mut bytes = Vec::new();
    (&mut response)
        .take(512 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512 * 1024 {
        return Err("Update response too large".into());
    }
    parse_release(
        &serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
        env!("CARGO_PKG_VERSION"),
    )
}
pub fn check() -> Value {
    let mut cached = match CACHE.get_or_init(|| Mutex::new(None)).lock() {
        Ok(lock) => lock,
        Err(_) => return json!({"status":"offline"}),
    };
    if let Some((when, value)) = &*cached {
        if when.elapsed() < Duration::from_secs(60) {
            return value.clone();
        }
    }
    let mut value = info();
    let result = fetch().unwrap_or(json!({"status":"offline"}));
    for (key, item) in result.as_object().unwrap() {
        value[key] = item.clone();
    }
    value["checkedAt"] = json!(chrono::Utc::now().to_rfc3339());
    *cached = Some((Instant::now(), value.clone()));
    value
}
pub fn open_releases() -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let verb = wide("open");
        let url = wide(RELEASES);
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
        if result as isize <= 32 {
            return Err("Could not open releases".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    Err("Opening releases is only supported on Windows".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Value {
        json!({"tag_name":format!("v{version}"),"draft":false,"prerelease":false,"body":"New features","assets":[{"name":format!("Questro-{version}.zip"),"browser_download_url":format!("{RELEASES}/download/v{version}/Questro-{version}.zip")}]})
    }
    #[test]
    fn numeric_versions_and_current_releases_are_compared_correctly() {
        assert_eq!(
            parse_release(&release("3.10.0"), "3.9.0").unwrap()["status"],
            "available"
        );
        assert_eq!(
            parse_release(&release("3.2.0"), "3.2.0").unwrap()["status"],
            "up_to_date"
        );
        assert_eq!(
            parse_release(&release("3.1.2"), "3.2.0").unwrap()["status"],
            "up_to_date"
        );
    }
    #[test]
    fn unstable_missing_and_foreign_downloads_are_rejected() {
        let mut value = release("3.3.0");
        value["assets"][0]["browser_download_url"] =
            json!("https://evil.example/Questro-3.3.0.zip");
        assert!(parse_release(&value, "3.2.0").is_err());
        value = release("3.3.0-beta.1");
        assert!(parse_release(&value, "3.2.0").is_err());
        value = release("3.3.0");
        value["draft"] = json!(true);
        assert!(parse_release(&value, "3.2.0").is_err());
        value = release("3.3.0");
        value["assets"] = json!([]);
        assert!(parse_release(&value, "3.2.0").is_err());
    }
}
