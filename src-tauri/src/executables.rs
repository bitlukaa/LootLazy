use crate::storage;
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Resolver {
    steam: HashMap<String, Vec<String>>,
    local: RefCell<HashMap<String, (Instant, Vec<String>)>>,
    roots: Vec<PathBuf>,
}
fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| storage::executable(&value).ok())
        .filter(|value| seen.insert(value.to_lowercase()))
        .take(128)
        .collect()
}
fn windows_names(game: &Value) -> Vec<String> {
    unique(
        game["executables"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|exe| exe["os"].is_null() || exe["os"] == "win32")
            .filter_map(|exe| exe["name"].as_str().map(str::to_owned)),
    )
}
fn steam_ids(game: &Value) -> Vec<String> {
    game["third_party_skus"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|sku| sku["distributor"] == "steam")
        .filter_map(|sku| sku["id"].as_str())
        .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_owned)
        .collect()
}
impl Resolver {
    pub fn new(catalog: &[Value]) -> Self {
        let mut steam: HashMap<String, Vec<String>> = HashMap::new();
        for game in catalog {
            let names = windows_names(game);
            if names.is_empty() {
                continue;
            }
            for id in steam_ids(game) {
                steam.entry(id).or_default().extend(names.clone());
            }
        }
        for names in steam.values_mut() {
            *names = unique(std::mem::take(names));
        }
        Self {
            steam,
            local: RefCell::new(HashMap::new()),
            roots: detected_roots(),
        }
    }
    pub fn clear_local(&self) {
        self.local.borrow_mut().clear();
    }
    pub fn resolve(
        &self,
        game: &Value,
        curated: &Value,
        overrides: &Value,
        learned: &Value,
    ) -> Value {
        let app = game["id"].as_str().unwrap_or("");
        let answer = |names: Vec<String>, status: &str, reliable: bool| json!({"appId":app,"gameName":game["name"],"primaryExecutable":names.first(),"executables":names,"status":status,"isReliable":reliable});
        if let Some(name) = overrides
            .pointer(&format!("/custom_executables/{app}"))
            .and_then(Value::as_str)
            .and_then(|s| storage::executable(s).ok())
        {
            return answer(vec![name], "custom_override", false);
        }
        let names = windows_names(game);
        if !names.is_empty() {
            return answer(names, "official_verified", true);
        }
        let entry = &curated["games"][app];
        let mut ids = steam_ids(game);
        if let Some(id) = entry["steam_appid"]
            .as_str()
            .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        {
            if !ids.iter().any(|known| known == id) {
                ids.push(id.to_string());
            }
        }
        let names = unique(
            ids.iter()
                .filter_map(|id| self.steam.get(id))
                .flatten()
                .cloned(),
        );
        if !names.is_empty() {
            return answer(names, "official_steam_match", true);
        }
        let names = unique(
            learned[app]["executables"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned)),
        );
        if !names.is_empty() {
            return answer(names, "steam_metadata", false);
        }
        if overrides.pointer("/settings/scan_local_steam") != Some(&json!(false)) {
            let cached = {
                self.local
                    .borrow()
                    .get(app)
                    .filter(|(when, _)| when.elapsed() < Duration::from_secs(600))
                    .map(|(_, names)| names.clone())
            };
            let names = cached.unwrap_or_else(|| {
                let names = installed_names(game, &ids, overrides, &self.roots);
                self.local
                    .borrow_mut()
                    .insert(app.to_string(), (Instant::now(), names.clone()));
                names
            });
            if !names.is_empty() {
                return answer(names, "local_steam_discovered", false);
            }
        }
        let names = unique(
            entry["executables"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|name| name.as_str().map(str::to_owned)),
        );
        if !names.is_empty() {
            return answer(names, "unverified_catalog", false);
        }
        answer(vec![], "unresolved", false)
    }
}
pub fn detected_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = [
        "C:/Program Files (x86)/Steam",
        "C:/Program Files/Steam",
        "D:/Steam",
        "D:/SteamLibrary",
        "E:/Steam",
        "E:/SteamLibrary",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let reg = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()))
            .join("System32/reg.exe");
        for (key, value) in [
            (r"HKCU\Software\Valve\Steam", "SteamPath"),
            (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
        ] {
            if let Ok(output) = std::process::Command::new(&reg)
                .args(["query", key, "/v", value])
                .creation_flags(0x08000000)
                .output()
            {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    if let Some(path) = line
                        .split("REG_SZ")
                        .nth(1)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        roots.push(path.into());
                    }
                }
            }
        }
    }
    roots
}
fn installed_names(
    game: &Value,
    ids: &[String],
    overrides: &Value,
    defaults: &[PathBuf],
) -> Vec<String> {
    let mut roots = defaults.to_vec();
    roots.extend(
        overrides
            .pointer("/settings/steam_paths")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(PathBuf::from)),
    );
    let initial = roots.clone();
    for root in initial {
        if let Ok(vdf) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in vdf.lines() {
                let parts: Vec<_> = line.split('"').collect();
                if parts.get(1) == Some(&"path") {
                    if let Some(path) = parts.get(3) {
                        roots.push(PathBuf::from(path.replace("\\\\", "\\")));
                    }
                }
            }
        }
    }
    let mut seen = HashSet::new();
    roots.retain(|root| seen.insert(root.to_string_lossy().to_lowercase()));
    for root in roots {
        for id in ids {
            let Ok(manifest) =
                fs::read_to_string(root.join(format!("steamapps/appmanifest_{id}.acf")))
            else {
                continue;
            };
            for line in manifest.lines() {
                let parts: Vec<_> = line.split('"').collect();
                if parts.get(1) != Some(&"installdir") {
                    continue;
                }
                let Some(folder) = parts.get(3) else {
                    continue;
                };
                if folder.is_empty()
                    || folder.contains(['/', '\\', ':'])
                    || *folder == "."
                    || *folder == ".."
                {
                    continue;
                }
                let mut names = vec![];
                let mut budget = 256;
                scan(
                    &root.join("steamapps/common").join(folder),
                    "",
                    0,
                    &mut budget,
                    &mut names,
                );
                let title: String = game["name"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect();
                names.sort_by_key(|name| {
                    let lower = name.to_lowercase();
                    let stem: String = Path::new(&lower)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect();
                    let score = if !title.is_empty() && stem == title {
                        100
                    } else {
                        0
                    } + if lower.contains("shipping") { 50 } else { 0 }
                        + if lower.contains("win64") { 15 } else { 0 }
                        - if lower.contains("launcher") { 25 } else { 0 };
                    (-score, lower)
                });
                let names = unique(names);
                if !names.is_empty() {
                    return names;
                }
            }
        }
    }
    vec![]
}
fn scan(root: &Path, prefix: &str, depth: usize, budget: &mut usize, out: &mut Vec<String>) {
    if depth > 4 || *budget == 0 || out.len() >= 128 {
        return;
    }
    *budget -= 1;
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().take(512).collect();
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
    let mut dirs = vec![];
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if entry
                .metadata()
                .is_ok_and(|m| m.file_attributes() & 0x400 != 0)
            {
                continue;
            }
        }
        if kind.is_file() && lower.ends_with(".exe") {
            let utility = [
                "crash",
                "unins",
                "redist",
                "directx",
                "dxsetup",
                "anticheat",
                "battleye",
                "cef",
            ]
            .iter()
            .any(|s| lower.contains(s))
                || [
                    "setup.exe",
                    "install.exe",
                    "installer.exe",
                    "update.exe",
                    "updater.exe",
                    "patcher.exe",
                    "helper.exe",
                    "reporter.exe",
                ]
                .contains(&lower.as_str());
            if !utility {
                out.push(format!("{prefix}{name}"));
                if out.len() >= 128 {
                    break;
                }
            }
        } else if kind.is_dir()
            && !lower.ends_with("_data")
            && !["_commonredist", "redist", "support", "tools", "installer"]
                .contains(&lower.as_str())
        {
            dirs.push((entry.path(), name));
        }
    }
    dirs.sort_by_key(|(_, name)| {
        !["bin", "binaries", "win64", "bin64", "x64", "client"]
            .contains(&name.to_lowercase().as_str())
    });
    for (path, name) in dirs {
        scan(&path, &format!("{prefix}{name}/"), depth + 1, budget, out);
    }
}
