use serde_json::Value;
use std::{collections::HashSet, io::Read, time::Duration};
pub fn windows_launch_names(payload: &Value, steam_id: &str) -> Vec<String> {
    let Some(launches) = payload["data"][steam_id]["config"]["launch"].as_object() else {
        return vec![];
    };
    let mut seen = HashSet::new();
    launches
        .values()
        .filter(|entry| {
            entry["config"]["betakey"]
                .as_str()
                .is_none_or(str::is_empty)
                && entry["config"]["oslist"]
                    .as_str()
                    .is_none_or(|os| os.split(',').any(|os| os.trim() == "windows"))
        })
        .filter_map(|entry| entry["executable"].as_str())
        .filter_map(|name| {
            crate::storage::executable(name.replace('\\', "/").trim_start_matches('/')).ok()
        })
        .filter(|name| seen.insert(name.to_lowercase()))
        .take(64)
        .collect()
}
pub fn steam_launch_names(steam_id: &str) -> Result<Vec<String>, String> {
    if steam_id.is_empty() || steam_id.len() > 12 || !steam_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid Steam application id".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!("https://api.steamcmd.net/v1/info/{steam_id}"))
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Executable metadata is too large".into());
    }
    let payload: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if payload["status"] != "success" {
        return Err("Executable metadata is unavailable".into());
    }
    Ok(windows_launch_names(&payload, steam_id))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_safe_public_windows_launch_paths_are_kept() {
        let data = json!({"data":{"3393110":{"config":{"launch":{"0":{"executable":"/Aion2/Binaries/Win64/AION2.exe","arguments":"not retained","config":{"oslist":"windows"}},"1":{"executable":"/Aion2/Binaries/Win64/aion2.exe","config":{"oslist":"windows"}},"2":{"executable":"linux-game","config":{"oslist":"linux"}},"3":{"executable":"../outside.exe","config":{"oslist":"windows"}},"4":{"executable":"private-beta.exe","config":{"betakey":"private","oslist":"windows"}}}}}}});
        assert_eq!(
            windows_launch_names(&data, "3393110"),
            vec!["Aion2/Binaries/Win64/AION2.exe"]
        );
    }
}
