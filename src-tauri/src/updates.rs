use serde_json::{json, Value};
use std::{
    io::Read,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
const RELEASES: &str = "https://github.com/bitlukaa/LootLazy/releases";
const ENDPOINT: &str = "https://api.github.com/repos/bitlukaa/LootLazy/releases/latest";
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
    let current_name = format!("LootLazy-{version}.zip");
    let asset = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"] == current_name)
        .ok_or("Portable release unavailable")?;
    let expected = asset["name"].as_str().ok_or("Missing asset name")?;
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
            .starts_with("/bitlukaa/LootLazy/releases/download/")
        || !url.path().ends_with(&format!("/{expected}"))
    {
        return Err("Invalid download".into());
    }
    let binary = executable_metadata(release).ok();
    let current = semver::Version::parse(current).map_err(|_| "Invalid current version")?;
    Ok(
        json!({"status":if version>current {"available"}else{"up_to_date"},"latestVersion":version.to_string(),"notes":release["body"].as_str().unwrap_or("").chars().take(8000).collect::<String>(),"downloadUrl":url.as_str(),"canInstall":binary.is_some(),"binaryUrl":binary.as_ref().map(|value|value["binaryUrl"].clone()),"sha256":binary.as_ref().map(|value|value["sha256"].clone()),"binarySize":binary.as_ref().map(|value|value["binarySize"].clone()),"signatureUrl":binary.as_ref().map(|value|value["signatureUrl"].clone())}),
    )
}
fn fetch() -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("LootLazy/{}", env!("CARGO_PKG_VERSION")))
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
fn trusted_asset_url(value: &str, name: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "Invalid download")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .path()
            .starts_with("/bitlukaa/LootLazy/releases/download/")
        || !url.path().ends_with(&format!("/{name}"))
    {
        return Err("Invalid download".into());
    }
    Ok(url)
}
fn executable_metadata(release: &Value) -> Result<Value, String> {
    let asset = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|asset| asset["name"] == "LootLazy.exe")
        .ok_or("Update executable unavailable")?;
    let url = trusted_asset_url(
        asset["browser_download_url"]
            .as_str()
            .ok_or("Missing download")?,
        "LootLazy.exe",
    )?;
    let hash = asset["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or("Missing update checksum")?;
    let size = asset["size"]
        .as_u64()
        .filter(|size| (512..=64 * 1024 * 1024).contains(size))
        .ok_or("Invalid update size")?;
    let signature = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|asset| asset["name"] == "LootLazy.exe.sig")
        .ok_or("Signed update unavailable")?;
    let signature_url = trusted_asset_url(
        signature["browser_download_url"]
            .as_str()
            .ok_or("Missing signature URL")?,
        "LootLazy.exe.sig",
    )?;
    if !signature["size"]
        .as_u64()
        .is_some_and(|size| (1..=4096).contains(&size))
    {
        return Err("Invalid signature size".into());
    }
    Ok(
        json!({"binaryUrl":url.as_str(),"sha256":hash,"binarySize":size,"signatureUrl":signature_url.as_str(),"canInstall":true}),
    )
}
pub fn stage(root: &std::path::Path) -> Result<Value, String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let release = check();
    if release["status"] != "available" || release["canInstall"] != true {
        return Err("updates_unavailable".into());
    }
    let url = trusted_asset_url(
        release["binaryUrl"].as_str().ok_or("Invalid download")?,
        "LootLazy.exe",
    )?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 3
                || attempt.url().scheme() != "https"
                || !matches!(
                    attempt.url().host_str(),
                    Some(
                        "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                )
            {
                attempt.error("Untrusted update redirect")
            } else {
                attempt.follow()
            }
        }))
        .user_agent(format!("LootLazy/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| error.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|_| "updates_download_failed")?
        .error_for_status()
        .map_err(|_| "updates_download_failed")?;
    let expected = release["binarySize"]
        .as_u64()
        .ok_or("Invalid update size")?;
    let mut bytes = Vec::new();
    (&mut response)
        .take(expected + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "updates_download_failed")?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    if bytes.len() as u64 != expected
        || hash
            != release["sha256"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase()
        || !bytes.starts_with(b"MZ")
    {
        return Err("updates_checksum_failed".into());
    }
    let signature_url = trusted_asset_url(
        release["signatureUrl"]
            .as_str()
            .ok_or("updates_signature_failed")?,
        "LootLazy.exe.sig",
    )?;
    let mut signature_response = client
        .get(signature_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|_| "updates_signature_failed")?;
    let mut signature = Vec::new();
    (&mut signature_response)
        .take(4097)
        .read_to_end(&mut signature)
        .map_err(|_| "updates_signature_failed")?;
    crate::update_signature::verify(
        &signature,
        release["latestVersion"].as_str().unwrap_or(""),
        &hash,
        expected,
    )?;
    let directory = root.join("data/updates");
    std::fs::create_dir_all(&directory).map_err(|_| "updates_save_failed")?;
    let source = directory.join("LootLazy-update.exe");
    let partial = directory.join("download.partial");
    let mut file = std::fs::File::create(&partial).map_err(|_| "updates_save_failed")?;
    file.write_all(&bytes).map_err(|_| "updates_save_failed")?;
    file.sync_all().map_err(|_| "updates_save_failed")?;
    drop(file);
    std::fs::rename(&partial, &source).map_err(|_| "updates_save_failed")?;
    let target = std::env::current_exe().map_err(|_| "updates_save_failed")?;
    let manifest = json!({"source":source,"target":target,"parentPid":std::process::id(),"sha256":hash,"version":release["latestVersion"],"restart":true});
    crate::storage::save(&directory.join("install.json"), &manifest)?;
    std::fs::write(
        directory.join("install.ps1"),
        include_str!("../../tools/install-update.ps1"),
    )
    .map_err(|_| "updates_save_failed")?;
    Ok(json!({"version":release["latestVersion"],"ready":true}))
}
pub fn launch_installer(root: &std::path::Path) -> Result<(), String> {
    let directory = root.join("data/updates");
    let mut command = std::process::Command::new("powershell.exe");
    command.env_remove("PSModulePath");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(directory.join("install.ps1"))
        .arg("-ManifestPath")
        .arg(directory.join("install.json"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.spawn().map_err(|_| "updates_install_failed")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Value {
        json!({"tag_name":format!("v{version}"),"draft":false,"prerelease":false,"body":"New features","assets":[{"name":format!("LootLazy-{version}.zip"),"browser_download_url":format!("{RELEASES}/download/v{version}/LootLazy-{version}.zip")}]})
    }
    #[test]
    fn release_assets_use_application_names() {
        let mut value = release("99.0.0");
        value["assets"][0]["name"] = json!("LootLazy-99.0.0.zip");
        value["assets"][0]["browser_download_url"] =
            json!(format!("{RELEASES}/download/v99.0.0/LootLazy-99.0.0.zip"));
        value["assets"].as_array_mut().unwrap().extend([
            json!({"name":"LootLazy.exe","browser_download_url":format!("{RELEASES}/download/v99.0.0/LootLazy.exe"),"digest":format!("sha256:{}","a".repeat(64)),"size":4096}),
            json!({"name":"LootLazy.exe.sig","browser_download_url":format!("{RELEASES}/download/v99.0.0/LootLazy.exe.sig"),"size":300})
        ]);
        assert_eq!(parse_release(&value, "3.6.8").unwrap()["canInstall"], true);
        assert!(trusted_asset_url(
            "https://github.com/bitlukaa/LootLazy-other/releases/download/v99/LootLazy.exe",
            "LootLazy.exe"
        )
        .is_err());
        assert!(parse_release(&release("3.6.8"), "3.6.9").is_ok());
    }
    #[test]
    fn executable_updates_require_a_trusted_url_digest_and_bounded_size() {
        let mut value = release("99.0.0");
        assert_eq!(parse_release(&value, "3.5.4").unwrap()["canInstall"], false);
        value["assets"].as_array_mut().unwrap().push(json!({"name":"LootLazy.exe","browser_download_url":format!("{RELEASES}/download/v99.0.0/LootLazy.exe"),"digest":format!("sha256:{}","a".repeat(64)),"size":4096}));
        assert_eq!(parse_release(&value, "3.5.4").unwrap()["canInstall"], false);
        value["assets"].as_array_mut().unwrap().push(json!({"name":"LootLazy.exe.sig","browser_download_url":format!("{RELEASES}/download/v99.0.0/LootLazy.exe.sig"),"size":300}));
        assert_eq!(parse_release(&value, "3.5.4").unwrap()["canInstall"], true);
        value["assets"][1]["digest"] = json!("sha256:invalid");
        assert!(executable_metadata(&value).is_err());
        value["assets"][1]["digest"] = json!(format!("sha256:{}", "a".repeat(64)));
        value["assets"][1]["size"] = json!(65 * 1024 * 1024);
        assert!(executable_metadata(&value).is_err());
        value["assets"][1]["size"] = json!(4096);
        value["assets"][1]["browser_download_url"] = json!("https://evil.example/LootLazy.exe");
        assert!(executable_metadata(&value).is_err());
        assert!(trusted_asset_url(
            "https://github.com/another/repo/releases/download/v99/LootLazy.exe",
            "LootLazy.exe"
        )
        .is_err());
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
            json!("https://evil.example/LootLazy-3.3.0.zip");
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
