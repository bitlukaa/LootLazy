//! Bounded reader for Steam's local application metadata; no account tokens are read.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, fs, path::PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub app_id: u32,
    pub name: String,
}
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    strings: &'a [String],
    nodes: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self
            .position
            .checked_add(n)
            .ok_or("achievements_library_invalid")?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or("achievements_library_invalid")?;
        self.position = end;
        Ok(value)
    }
    fn number(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn text(&mut self) -> Result<String, String> {
        let rest = self
            .bytes
            .get(self.position..)
            .ok_or("achievements_library_invalid")?;
        let n = rest
            .iter()
            .take(65537)
            .position(|v| *v == 0)
            .ok_or("achievements_library_invalid")?;
        let value = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.position += n + 1;
        Ok(value)
    }
    fn object(&mut self, depth: usize) -> Result<Value, String> {
        if depth > 32 {
            return Err("achievements_library_invalid".into());
        }
        let mut map = Map::new();
        loop {
            self.nodes += 1;
            if self.nodes > 500000 {
                return Err("achievements_library_invalid".into());
            }
            let kind = self.take(1)?[0];
            if kind == 8 {
                return Ok(Value::Object(map));
            }
            let key = if self.strings.is_empty() {
                self.text()?
            } else {
                let index = self.number()? as usize;
                self.strings
                    .get(index)
                    .ok_or("achievements_library_invalid")?
                    .clone()
            };
            let value = match kind {
                0 => self.object(depth + 1)?,
                1 => Value::String(self.text()?),
                2 => Value::from(self.number()?),
                3 => {
                    self.take(4)?;
                    Value::Null
                }
                4 | 6 => {
                    self.take(4)?;
                    Value::Null
                }
                7 | 9 => {
                    self.take(8)?;
                    Value::Null
                }
                5 => {
                    let mut value = String::new();
                    loop {
                        let n = u16::from_le_bytes(self.take(2)?.try_into().unwrap());
                        if n == 0 {
                            break;
                        }
                        if value.len() > 65536 {
                            return Err("achievements_library_invalid".into());
                        }
                        value.push(char::from_u32(n as u32).unwrap_or('\u{fffd}'));
                    }
                    Value::String(value)
                }
                _ => return Err("achievements_library_invalid".into()),
            };
            map.insert(key, value);
        }
    }
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Game>, String> {
    let mut file = Reader {
        bytes,
        position: 0,
        strings: &[],
        nodes: 0,
    };
    let magic = file.number()?;
    if ![0x07564427, 0x07564428, 0x07564429].contains(&magic) {
        return Err("achievements_library_invalid".into());
    }
    file.number()?;
    let mut strings = Vec::new();
    let mut limit = bytes.len();
    if magic == 0x07564429 {
        limit = u64::from_le_bytes(file.take(8)?.try_into().unwrap())
            .try_into()
            .map_err(|_| "achievements_library_invalid")?;
        let mut table = Reader {
            bytes: bytes.get(limit..).ok_or("achievements_library_invalid")?,
            position: 0,
            strings: &[],
            nodes: 0,
        };
        let count = table.number()?;
        if count > 500000 {
            return Err("achievements_library_invalid".into());
        }
        for _ in 0..count {
            strings.push(table.text()?);
        }
    }
    let mut games = Vec::new();
    while file.position < limit {
        let app_id = file.number()?;
        if app_id == 0 {
            break;
        }
        let size = file.number()? as usize;
        let start = file.position;
        let end = start
            .checked_add(size)
            .filter(|end| *end <= limit)
            .ok_or("achievements_library_invalid")?;
        let header = if magic >= 0x07564428 { 60 } else { 40 };
        if size < header {
            return Err("achievements_library_invalid".into());
        }
        let mut section = Reader {
            bytes: &bytes[start + header..end],
            position: 0,
            strings: &strings,
            nodes: 0,
        };
        if let Ok(data) = section.object(0) {
            let data = data.get("appinfo").unwrap_or(&data);
            let common = &data["common"];
            let kind = common["type"].as_str().unwrap_or("").to_ascii_lowercase();
            if ["game", "application", "demo"].contains(&kind.as_str()) {
                if let Some(name) = common["name"].as_str().filter(|name| !name.is_empty()) {
                    games.push(Game {
                        app_id,
                        name: name.chars().take(256).collect(),
                    });
                }
            }
        }
        file.position = end;
        if games.len() > 100000 {
            return Err("achievements_library_invalid".into());
        }
    }
    Ok(games)
}
pub fn read(roots: &[PathBuf]) -> Result<Vec<Game>, String> {
    let mut result = BTreeMap::new();
    let mut found = false;
    for root in roots {
        let path = root.join("appcache/appinfo.vdf");
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.len() > 128 * 1024 * 1024 {
            continue;
        }
        if let Ok(games) = fs::read(path)
            .map_err(|_| "achievements_library_invalid".to_string())
            .and_then(|bytes| parse(&bytes))
        {
            found = true;
            for game in games {
                result.entry(game.app_id).or_insert(game);
            }
        }
    }
    if !found {
        return Err("achievements_library_unavailable".into());
    }
    Ok(result.into_values().collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_and_truncated_metadata() {
        for data in [vec![], vec![0; 32], 0x07564429u32.to_le_bytes().to_vec()] {
            assert!(parse(&data).is_err());
        }
    }
    #[test]
    fn parses_application_candidates_without_account_data() {
        let mut bytes = Vec::new();
        bytes.extend(0x07564428u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        let kv=b"\x00appinfo\0\x00common\0\x01name\0Fixture game\0\x01type\0Game\0\x00category\0\x01category_1\x001\0\x08\x08\x08\x08";
        bytes.extend(42u32.to_le_bytes());
        bytes.extend(((60 + kv.len()) as u32).to_le_bytes());
        bytes.extend([0; 60]);
        bytes.extend(kv);
        bytes.extend(0u32.to_le_bytes());
        let games = parse(&bytes).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Fixture game");
        assert_eq!(
            serde_json::to_value(&games[0]).unwrap(),
            serde_json::json!({"appId":42,"name":"Fixture game"})
        );
    }
    #[test]
    #[ignore = "Requires an explicitly selected local Steam cache"]
    fn probe_local_library() {
        let root = std::env::var_os("LOOTLAZY_TEST_STEAM_ROOT").expect("Select a Steam root");
        let games = read(&[root.into()]).unwrap();
        assert!(!games.is_empty());
        let file =
            std::env::var_os("LOOTLAZY_TEST_CATALOG").expect("Select a diagnostic output file");
        fs::write(file, serde_json::to_vec(&games).unwrap()).unwrap();
        eprintln!("Parsed {} application candidates", games.len());
    }
}
