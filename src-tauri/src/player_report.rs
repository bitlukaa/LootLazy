//! Read-only report preparation. All actual submissions remain in Steam's own UI.
use serde_json::{json, Value};
use std::{io::Read, time::Duration};
fn valid_id(id: &str) -> bool {
    id.parse::<u64>().is_ok_and(|value| {
        value > 76561197960265728 && value <= 76561197960265728 + u64::from(u32::MAX)
    })
}
pub fn profile(input: &str) -> Result<url::Url, String> {
    let input = input.trim();
    if input.len() > 250 {
        return Err("report_profile_invalid".into());
    }
    let input = if valid_id(input) {
        format!("https://steamcommunity.com/profiles/{input}/")
    } else {
        input.to_string()
    };
    let mut url = url::Url::parse(&input).map_err(|_| "report_profile_invalid")?;
    if url.scheme() != "https"
        || url.host_str() != Some("steamcommunity.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("report_profile_invalid".into());
    }
    let parts: Vec<_> = url
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 2
        || !matches!(parts[0], "profiles" | "id")
        || parts[1].len() > 64
        || !parts[1]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        || (parts[0] == "profiles" && !valid_id(parts[1]))
    {
        return Err("report_profile_invalid".into());
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}
fn field(xml: &str, name: &str) -> Option<String> {
    let start = format!("<{name}>");
    let end = format!("</{name}>");
    let text = xml.split_once(&start)?.1.split_once(&end)?.0.trim();
    let text = text
        .strip_prefix("<![CDATA[")
        .and_then(|text| text.strip_suffix("]]>"))
        .unwrap_or(text);
    Some(
        text.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .chars()
            .take(256)
            .collect(),
    )
}
fn download(client: &reqwest::blocking::Client, url: url::Url) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|_| "report_profile_unavailable")?;
    let mut bytes = Vec::new();
    response
        .take(524289)
        .read_to_end(&mut bytes)
        .map_err(|_| "report_profile_unavailable")?;
    if bytes.len() > 524288 {
        return Err("report_profile_unavailable".into());
    }
    String::from_utf8(bytes).map_err(|_| "report_profile_unavailable".into())
}
pub fn preview(input: &str) -> Result<Value, String> {
    let mut target = profile(input)?;
    target.query_pairs_mut().append_pair("xml", "1");
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(7))
        .build()
        .map_err(|_| "report_profile_unavailable")?;
    let xml = download(&client, target)?;
    let id = field(&xml, "steamID64")
        .filter(|id| valid_id(id))
        .ok_or("report_profile_unavailable")?;
    let name = field(&xml, "steamID").unwrap_or_else(|| "Steam profile".into());
    let recent = url::Url::parse(&format!(
        "https://steamcommunity.com/profiles/{id}/games/?tab=recent&xml=1"
    ))
    .unwrap();
    let mut games = Vec::new();
    if let Ok(xml) = download(&client, recent) {
        for part in xml.split("<game>").skip(1).take(2000) {
            let Some(id) = field(part, "appID")
                .and_then(|id| id.parse::<u32>().ok())
                .filter(|id| *id > 0)
            else {
                continue;
            };
            if field(part, "hoursLast2Weeks")
                .and_then(|hours| hours.replace(',', "").parse::<f64>().ok())
                .unwrap_or(0.0)
                > 0.0
            {
                games.push(
                    json!({"id":id,"name":field(part,"name").unwrap_or_else(||id.to_string())}),
                );
            }
            if games.len() >= 20 {
                break;
            }
        }
    }
    Ok(
        json!({"steamId":id,"name":name,"profileUrl":format!("https://steamcommunity.com/profiles/{id}/"),"games":games,"submission":"steam_ui"}),
    )
}
pub fn open(input: &str) -> Result<(), String> {
    let url = profile(input)?;
    crate::platform_download::open_allowed(url.as_str())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_only_open_canonical_steam_profiles() {
        for input in [
            "https://evil.test/id/x",
            "https://steamcommunity.com.evil.test/id/x",
            "https://steamcommunity.com@evil.test/id/x",
            "http://steamcommunity.com/id/x",
            "https://steamcommunity.com:444/id/x",
            "https://steamcommunity.com/id/../login",
            "https://steamcommunity.com/profiles/1",
            "https://steamcommunity.com/id/x/extra",
        ] {
            assert!(profile(input).is_err(), "{input}");
        }
        assert_eq!(
            profile("76561198000000000").unwrap().as_str(),
            "https://steamcommunity.com/profiles/76561198000000000/"
        );
        assert!(profile("https://steamcommunity.com/id/example/?xml=1#x")
            .unwrap()
            .query()
            .is_none());
    }
    #[test]
    fn public_xml_fields_are_text_only() {
        assert_eq!(
            field("<steamID><![CDATA[<Example>]]></steamID>", "steamID").unwrap(),
            "<Example>"
        );
    }
}
