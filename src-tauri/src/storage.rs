use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub fn read(file: &Path, fallback: Value) -> Value {
    fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(fallback)
}
pub fn save(file: &Path, value: &Value) -> Result<(), String> {
    fs::create_dir_all(file.parent().ok_or("Invalid storage path")?).map_err(|e| e.to_string())?;
    let temp = file.with_extension("json.tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, file).map_err(|e| e.to_string())
}
pub fn id(value: &str) -> Result<String, String> {
    if (5..=22).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_digit()) {
        Ok(value.into())
    } else {
        Err("Invalid application ID".into())
    }
}
pub fn executable(value: &str) -> Result<String, String> {
    let value = value.trim().replace('\\', "/");
    if value.len() > 240
        || ![".exe", ".bin"]
            .iter()
            .any(|extension| value.to_lowercase().ends_with(extension))
        || value.chars().any(|c| c < ' ' || ":<>\"|?*".contains(c))
    {
        return Err("Invalid executable filename".into());
    }
    for part in value.split('/') {
        let base = part.split('.').next().unwrap_or("").to_ascii_lowercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || ["con", "prn", "aux", "nul"].contains(&base.as_str())
            || ((base.starts_with("com") || base.starts_with("lpt"))
                && base.len() == 4
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
        {
            return Err("Use a relative Windows executable path".into());
        }
    }
    Ok(value)
}
#[allow(dead_code)]
pub fn target(root: &Path, app: &str, exe: &str) -> Result<PathBuf, String> {
    Ok(root.join(id(app)?).join(executable(exe)?))
}
pub fn defaults() -> Value {
    json!({"saved_games":[],"custom_executables":{},"settings":{}})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_confined() {
        for bad in [
            "../game.exe",
            "C:\\game.exe",
            "/game.exe",
            "win/../game.exe",
            "con.exe",
            "game.exe:evil",
            "a//game.exe",
            "aux/game.exe",
        ] {
            assert!(executable(bad).is_err(), "{bad}");
        }
        assert_eq!(executable("win64\\cs2.exe").unwrap(), "win64/cs2.exe");
        assert!(id("abc").is_err());
    }
}
