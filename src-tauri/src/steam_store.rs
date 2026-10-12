use serde_json::{json, Value};
use std::{collections::HashMap, io::Read, sync::Mutex, time::Duration, time::Instant};
static CACHE: Mutex<Option<HashMap<String, (Instant, Vec<Value>)>>> = Mutex::new(None);
pub fn search(query: &str) -> Result<Vec<Value>, String> {
    let query = query.trim();
    if query.is_empty() || query.len() > 160 {
        return Ok(vec![]);
    }
    {
        let mut cache = CACHE.lock().map_err(|_| "Search unavailable")?;
        let cache = cache.get_or_insert_with(HashMap::new);
        if let Some((time, games)) = cache.get(query) {
            if time.elapsed() < Duration::from_secs(300) {
                return Ok(games.clone());
            }
        }
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let exact = query.parse::<u32>().ok().filter(|id| *id > 0);
    let mut url = url::Url::parse(if exact.is_some() {
        "https://store.steampowered.com/api/appdetails"
    } else {
        "https://store.steampowered.com/api/storesearch/"
    })
    .unwrap();
    if exact.is_some() {
        url.query_pairs_mut()
            .append_pair("appids", query)
            .append_pair("filters", "basic");
    } else {
        url.query_pairs_mut()
            .append_pair("term", query)
            .append_pair("l", "english")
            .append_pair("cc", "US");
    }
    let response = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    response
        .take(524289)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 524288 {
        return Err("Steam search response too large".into());
    }
    let response: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let candidates = if exact.is_some() {
        response[query]["data"]
            .as_object()
            .map(|_| vec![response[query]["data"].clone()])
            .unwrap_or_default()
    } else {
        response["items"].as_array().cloned().unwrap_or_default()
    };
    let mut games = Vec::new();
    for item in candidates.into_iter().take(30) {
        let Some(id) = item["id"]
            .as_u64()
            .or_else(|| item["steam_appid"].as_u64())
            .filter(|id| *id > 0 && *id <= u32::MAX as u64)
        else {
            continue;
        };
        let Some(name) = item["name"]
            .as_str()
            .filter(|name| !name.is_empty() && name.len() < 500)
        else {
            continue;
        };
        let id = id.to_string();
        let image = item["tiny_image"]
            .as_str()
            .or_else(|| item["header_image"].as_str())
            .and_then(|value| {
                let mut url = url::Url::parse(value).ok()?;
                url.set_query(None);
                crate::artwork::validated(url.as_str())
                    .ok()
                    .map(|u| u.to_string())
            });
        games.push(json!({"id":id,"steamId":id,"name":name,"image":image,"installed":false,"third_party_skus":[{"distributor":"steam","id":id}]}));
    }
    let mut cache = CACHE.lock().map_err(|_| "Search unavailable")?;
    let cache = cache.get_or_insert_with(HashMap::new);
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.insert(query.to_string(), (Instant::now(), games.clone()));
    Ok(games)
}
