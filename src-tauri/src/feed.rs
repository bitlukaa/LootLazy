use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
pub const SOURCE: &str = "https://api.discordquest.com/api/quests";
fn timestamp(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v.as_str()?)
        .ok()
        .map(|d| d.timestamp_millis())
}
fn asset(v: &Value, id: &str) -> Option<String> {
    let s = v.as_str()?;
    if s.is_empty() || s == "PLACEHOLDER" {
        return None;
    }
    let candidate = if s.starts_with("https://") {
        s.to_string()
    } else if s.trim_start_matches('/').starts_with("quests/") {
        format!("https://cdn.discordapp.com/{}", s.trim_start_matches('/'))
    } else {
        format!("https://cdn.discordapp.com/quests/{id}/{s}")
    };
    let url = url::Url::parse(&candidate).ok()?;
    let path = url.path();
    let parts: Vec<_> = path.split('/').collect();
    if url.scheme() == "https"
        && url.host_str() == Some("cdn.discordapp.com")
        && url.query().is_none()
        && url.fragment().is_none()
        && parts.len() >= 4
        && parts[1] == "quests"
        && crate::storage::id(parts[2]).is_ok()
        && [".png", ".jpg", ".jpeg", ".webp"]
            .iter()
            .any(|ext| path.to_lowercase().ends_with(ext))
    {
        Some(candidate)
    } else {
        None
    }
}
pub fn normalize(payload: &Value, now: i64) -> Result<Vec<Value>, String> {
    let list = payload
        .as_array()
        .or_else(|| payload["quests"].as_array())
        .ok_or("Invalid quest feed")?;
    let mut result: Vec<Value> = vec![];
    let mut grouped = HashMap::<String, usize>::new();
    let mut seen = std::collections::HashSet::new();
    for raw in list {
        let c = raw.get("config").unwrap_or(raw);
        let id = raw["id"].as_str().or(c["id"].as_str()).unwrap_or("");
        if crate::storage::id(id).is_err() || !seen.insert(id.to_string()) {
            continue;
        }
        let (Some(start), Some(end)) = (timestamp(&c["starts_at"]), timestamp(&c["expires_at"]))
        else {
            continue;
        };
        if start > now || end <= now || end - start > 366 * 86400000 {
            continue;
        }
        let tasks = c
            .pointer("/task_config_v2/tasks")
            .or_else(|| c.pointer("/task_config/tasks"));
        let Some(tasks) = tasks.and_then(Value::as_object) else {
            continue;
        };
        if tasks
            .keys()
            .any(|key| key.contains("WATCH") || key.contains("ACTIVITY"))
        {
            continue;
        }
        let Some(task) = tasks.get("PLAY_ON_DESKTOP") else {
            continue;
        };
        if task["target"].as_f64() != Some(900.0) {
            continue;
        }
        let app = task
            .pointer("/applications/0/id")
            .or_else(|| c.pointer("/application/id"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let name = c
            .pointer("/messages/game_title")
            .or_else(|| c.pointer("/application/name"))
            .or_else(|| c.pointer("/messages/quest_name"))
            .and_then(Value::as_str)
            .unwrap_or("Game");
        let title = c
            .pointer("/messages/quest_name")
            .and_then(Value::as_str)
            .unwrap_or(name);
        let lower = title.to_lowercase();
        if ["placeholder", "test quest", "quest test"]
            .iter()
            .any(|s| lower.contains(s))
        {
            continue;
        }
        let image = ["hero", "game_tile_dark", "game_tile_light", "game_tile"]
            .iter()
            .find_map(|key| asset(&c["assets"][*key], id));
        let rewards:Vec<Value>=c.pointer("/rewards_config/rewards").and_then(Value::as_array).into_iter().flatten().map(|r|json!({"name":r.pointer("/messages/name").and_then(Value::as_str).unwrap_or(""),"type":r["type"],"quantity":r.get("orb_quantity").or_else(||r.get("quantity")).unwrap_or(&Value::Null)})).collect();
        let start = DateTime::<Utc>::from_timestamp_millis(start)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let end = DateTime::<Utc>::from_timestamp_millis(end)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let key = json!([title, app, start, end, rewards]).to_string();
        if let Some(index) = grouped.get(&key) {
            result[*index]["variantIds"]
                .as_array_mut()
                .unwrap()
                .push(json!(id));
            continue;
        }
        grouped.insert(key, result.len());
        result.push(json!({"id":id,"variantIds":[id],"appId":if crate::storage::id(app).is_ok(){Some(app)}else{None},"gameName":name.chars().take(200).collect::<String>(),"questName":title.chars().take(250).collect::<String>(),"kind":"play","durationSeconds":900,"durationMinutes":15,"target":null,"startsAt":start,"expiresAt":end,"image":image,"rewards":rewards}));
    }
    result.sort_by(|a, b| b["startsAt"].as_str().cmp(&a["startsAt"].as_str()));
    Ok(result)
}
pub fn current(q: &Value) -> bool {
    q["kind"] == "play"
        && q["durationSeconds"] == 900
        && timestamp(&q["expiresAt"]).is_some_and(|t| t > Utc::now().timestamp_millis())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_desktop_fifteen_minutes() {
        let now = Utc::now().timestamp_millis();
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let q = json!({"id":"123456","config":{"starts_at":start.to_rfc3339(),"expires_at":end.to_rfc3339(),"application":{"id":"1234567","name":"Game"},"task_config_v2":{"tasks":{"PLAY_ON_DESKTOP":{"target":900}}}}});
        assert_eq!(normalize(&json!([q.clone()]), now).unwrap().len(), 1);
        let mut watch = q.clone();
        watch["config"]["task_config_v2"]["tasks"]["WATCH_VIDEO"] = json!({"target":30});
        assert!(normalize(&json!([watch]), now).unwrap().is_empty());
        let mut long = q;
        long["config"]["task_config_v2"]["tasks"]["PLAY_ON_DESKTOP"]["target"] = json!(1800);
        assert!(normalize(&json!([long]), now).unwrap().is_empty());
    }
}
