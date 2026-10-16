use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Default)]
struct Identity {
    session: Option<Value>,
    checked: Option<Instant>,
    loaded: bool,
    attempt: u64,
    pending: bool,
    error: Option<String>,
}
static STATE: OnceLock<Mutex<Identity>> = OnceLock::new();
fn state() -> &'static Mutex<Identity> {
    STATE.get_or_init(|| Mutex::new(Identity::default()))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn encode(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let (mut bits, mut n) = (0u32, 0);
    for byte in bytes {
        bits = (bits << 8) | u32::from(*byte);
        n += 8;
        while n >= 6 {
            n -= 6;
            out.push(ABC[((bits >> n) & 63) as usize] as char);
        }
    }
    if n > 0 {
        out.push(ABC[((bits << (6 - n)) & 63) as usize] as char);
    }
    out
}
fn random() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    encode(&bytes)
}
fn opaque(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn base() -> Result<url::Url, String> {
    let config: Value =
        serde_json::from_str(include_str!("../platform-auth.json").trim_start_matches('\u{feff}'))
            .map_err(|_| "platform_auth_unconfigured")?;
    let value = config["serviceUrl"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("platform_auth_unconfigured")?;
    let url = url::Url::parse(value).map_err(|_| "platform_auth_unconfigured")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("platform_auth_unconfigured".into());
    }
    Ok(url)
}
fn remote(path: &str, body: Option<&Value>, access: Option<&str>) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "platform_auth_error")?;
    let target = base()?.join(path).map_err(|_| "platform_auth_error")?;
    let mut req = if let Some(body) = body {
        client.post(target).json(body)
    } else {
        client.get(target)
    };
    if let Some(token) = access {
        req = req.bearer_auth(token);
    }
    let response = req.send().map_err(|_| "platform_auth_unavailable")?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("platform_auth_required".into());
    }
    if !response.status().is_success() {
        return Err("platform_auth_error".into());
    }
    let mut bytes = Vec::new();
    response
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "platform_auth_error")?;
    if bytes.len() > 65536 {
        return Err("platform_auth_error".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "platform_auth_error".into())
}
fn checked_path(path: &Path) -> Result<(), String> {
    let mut current = Some(path);
    while let Some(value) = current {
        if let Ok(meta) = fs::symlink_metadata(value) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("platform_auth_storage".into());
                }
            }
            if meta.file_type().is_symlink() {
                return Err("platform_auth_storage".into());
            }
        }
        current = value.parent();
    }
    Ok(())
}
#[cfg(windows)]
fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    crypt(
        bytes,
        encrypt,
        &format!("LootLazy platform session v1\0{}", base()?),
    )
}
#[cfg(windows)]
fn crypt(bytes: &[u8], encrypt: bool, context: &str) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            input: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(value: *mut c_void) -> *mut c_void;
    }
    if bytes.len() > 65536 {
        return Err("platform_auth_storage".into());
    }
    let input = Blob {
        len: bytes.len() as u32,
        data: bytes.as_ptr() as *mut u8,
    };
    let entropy = Blob {
        len: context.len() as u32,
        data: context.as_ptr() as *mut u8,
    };
    let mut output = Blob {
        len: 0,
        data: std::ptr::null_mut(),
    };
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null_mut(),
                std::ptr::null(),
                1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null_mut(),
                std::ptr::null(),
                1,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err("platform_auth_storage".into());
    }
    let result = unsafe { std::slice::from_raw_parts(output.data, output.len as usize).to_vec() };
    unsafe {
        LocalFree(output.data as *mut c_void);
    }
    Ok(result)
}
#[cfg(not(windows))]
fn protect(_bytes: &[u8], _encrypt: bool) -> Result<Vec<u8>, String> {
    Err("platform_auth_storage".into())
}
fn file(root: &Path) -> PathBuf {
    root.join("data/platform-session.bin")
}
fn persist(root: &Path, session: &Value) -> Result<(), String> {
    let file = file(root);
    let temp = file.with_extension("bin.tmp");
    checked_path(&file)?;
    checked_path(&temp)?;
    fs::create_dir_all(file.parent().ok_or("platform_auth_storage")?)
        .map_err(|_| "platform_auth_storage")?;
    let mut plain = serde_json::to_vec(session).map_err(|_| "platform_auth_storage")?;
    let encrypted = protect(&plain, true);
    plain.fill(0);
    fs::write(&temp, encrypted?).map_err(|_| "platform_auth_storage")?;
    fs::rename(&temp, &file).map_err(|_| "platform_auth_storage".into())
}
fn load(root: &Path) -> Option<Value> {
    let file = file(root);
    checked_path(&file).ok()?;
    let mut plain = protect(&fs::read(file).ok()?, false).ok()?;
    let result = serde_json::from_slice(&plain).ok();
    plain.fill(0);
    result
}
fn forget(root: &Path) {
    let path = file(root);
    if checked_path(&path).is_ok() {
        let _ = fs::remove_file(path);
    }
}
fn accept(root: &Path, mut session: Value) -> Result<Value, String> {
    for key in ["accessToken", "refreshToken"] {
        if !opaque(session[key].as_str().unwrap_or("")) {
            return Err("platform_auth_error".into());
        }
    }
    if session["user"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .is_none()
    {
        return Err("platform_auth_error".into());
    }
    session["accessUntil"] = json!(now() + session["expiresIn"].as_u64().unwrap_or(900).min(900));
    persist(root, &session)?;
    Ok(session)
}
fn verified(root: &Path, identity: &mut Identity) -> Result<Value, String> {
    if !identity.loaded {
        identity.session = load(root);
        identity.loaded = true;
    }
    let session = identity.session.as_ref().ok_or("platform_auth_required")?;
    if identity
        .checked
        .is_some_and(|time| time.elapsed() < Duration::from_secs(60))
        && session["accessUntil"].as_u64().unwrap_or(0) > now()
    {
        return Ok(json!({"user":session["user"],"plan":session["plan"]}));
    }
    let result = match remote("/v1/session", None, session["accessToken"].as_str()) {
        Ok(value) => value,
        Err(error) if error == "platform_auth_required" => {
            let refresh = session["refreshToken"].as_str().unwrap_or("").to_string();
            match remote(
                "/v1/session/refresh",
                Some(&json!({"refreshToken":refresh})),
                None,
            ) {
                Ok(value) => {
                    let value = accept(root, value)?;
                    let result = json!({"user":value["user"],"plan":value["plan"]});
                    identity.session = Some(value);
                    result
                }
                Err(error) => {
                    if error == "platform_auth_required" {
                        forget(root);
                        identity.session = None;
                    }
                    return Err(error);
                }
            }
        }
        Err(error) => return Err(error),
    };
    identity.checked = Some(Instant::now());
    Ok(result)
}
pub fn enforced() -> bool {
    cfg!(feature = "platform-login")
}
pub fn require(root: &Path) -> Result<(), String> {
    if !enforced() {
        return Ok(());
    }
    base()?;
    let mut identity = state().lock().map_err(|_| "platform_auth_error")?;
    verified(root, &mut identity).map(|_| ())
}
pub fn status(root: &Path) -> Value {
    if base().is_err() {
        return json!({"enforced":enforced(),"configured":false,"authenticated":false,"providers":[]});
    }
    let mut identity = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            return json!({"enforced":enforced(),"configured":true,"authenticated":false,"error":"platform_auth_error"})
        }
    };
    match verified(root, &mut identity) {
        Ok(value) => {
            json!({"enforced":enforced(),"configured":true,"authenticated":true,"user":value["user"],"plan":value["plan"]})
        }
        Err(error) => {
            json!({"enforced":enforced(),"configured":true,"authenticated":false,"pending":identity.pending,"error":if identity.pending{None}else{identity.error.clone().or_else(||if error=="platform_auth_required"{None}else{Some(error)})}})
        }
    }
}
pub fn config() -> Result<Value, String> {
    remote("/v1/config", None, None)
}
pub fn cancel() {
    if let Ok(mut identity) = state().lock() {
        identity.attempt += 1;
        identity.pending = false;
    }
}
pub fn signout(root: &Path) -> Result<(), String> {
    let mut identity = state().lock().map_err(|_| "platform_auth_error")?;
    if let Some(session) = identity.session.as_ref() {
        let _ = remote(
            "/v1/session/logout",
            Some(&json!({"refreshToken":session["refreshToken"]})),
            session["accessToken"].as_str(),
        );
    }
    forget(root);
    identity.session = None;
    identity.loaded = true;
    identity.checked = None;
    identity.attempt += 1;
    identity.pending = false;
    identity.error = None;
    Ok(())
}
pub fn begin(root: &Path, provider: &str) -> Result<(), String> {
    if !["google", "discord", "steam"].contains(&provider) {
        return Err("platform_auth_error".into());
    }
    base()?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "platform_auth_error")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "platform_auth_error")?;
    let verifier = random();
    let client_state = random();
    let route = format!("/lootlazy/callback/{}", random());
    let callback = format!(
        "http://127.0.0.1:{}{}",
        listener
            .local_addr()
            .map_err(|_| "platform_auth_error")?
            .port(),
        route
    );
    let mut identity = state().lock().map_err(|_| "platform_auth_error")?;
    if identity.pending {
        return Err("platform_auth_busy".into());
    }
    let response = remote(
        "/v1/login/start",
        Some(
            &json!({"provider":provider,"challenge":encode(&Sha256::digest(verifier.as_bytes())),"state":client_state,"callback":callback}),
        ),
        None,
    )?;
    let target = url::Url::parse(
        response["authorizeUrl"]
            .as_str()
            .ok_or("platform_auth_error")?,
    )
    .map_err(|_| "platform_auth_error")?;
    if target.origin() != base()?.origin()
        || target.path() != "/v1/authorize"
        || !target.username().is_empty()
        || target.password().is_some()
        || target.fragment().is_some()
    {
        return Err("platform_auth_error".into());
    }
    crate::platform_download::open_allowed(target.as_str())?;
    identity.attempt += 1;
    let attempt = identity.attempt;
    identity.pending = true;
    identity.error = None;
    drop(identity);
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let started = Instant::now();
        let result = (|| -> Result<Value, String> {
            while started.elapsed() < Duration::from_secs(180) {
                if state().lock().map_err(|_| "platform_auth_error")?.attempt != attempt {
                    return Err("platform_auth_cancelled".into());
                }
                match listener.accept() {
                    Ok((mut stream, address)) => {
                        if !address.ip().is_loopback() {
                            continue;
                        }
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        let mut bytes = Vec::new();
                        let mut byte = [0u8; 1];
                        while bytes.len() < 4096 {
                            match stream.read(&mut byte) {
                                Ok(0) => break,
                                Ok(_) => {
                                    bytes.push(byte[0]);
                                    if byte[0] == b'\n' {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        let request =
                            std::str::from_utf8(&bytes).map_err(|_| "platform_auth_error")?;
                        let parts = request
                            .lines()
                            .next()
                            .unwrap_or("")
                            .split_whitespace()
                            .collect::<Vec<_>>();
                        if parts.len() != 3 || parts[0] != "GET" {
                            let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                            continue;
                        }
                        let url = url::Url::parse(&format!("http://127.0.0.1{}", parts[1]))
                            .map_err(|_| "platform_auth_error")?;
                        let query = url
                            .query_pairs()
                            .collect::<std::collections::HashMap<_, _>>();
                        if url.path() != route
                            || query.get("state").map(|s| s.as_ref()) != Some(client_state.as_str())
                        {
                            let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                            continue;
                        }
                        let html="<!doctype html><title>LootLazy</title><p>Return to LootLazy to continue.</p>";
                        let _=stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",html.len(),html).as_bytes());
                        if query.contains_key("error") {
                            return Err("platform_auth_cancelled".into());
                        }
                        let code = query
                            .get("code")
                            .map(|s| s.as_ref())
                            .filter(|s| opaque(s))
                            .ok_or("platform_auth_error")?;
                        return remote(
                            "/v1/login/exchange",
                            Some(&json!({"code":code,"verifier":verifier})),
                            None,
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100))
                    }
                    Err(_) => return Err("platform_auth_error".into()),
                }
            }
            Err("platform_auth_timeout".into())
        })();
        if let Ok(mut identity) = state().lock() {
            if identity.attempt != attempt {
                return;
            }
            identity.pending = false;
            match result.and_then(|value| accept(&root, value)) {
                Ok(session) => {
                    identity.session = Some(session);
                    identity.loaded = true;
                    identity.checked = Some(Instant::now());
                    identity.error = None;
                }
                Err(error) => identity.error = Some(error),
            }
        }
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_sessions_are_protected_and_bound_to_service() {
        let plain = b"isolated-fixture-refresh-token";
        let encrypted = crypt(plain, true, "fixture-service").unwrap();
        assert!(!encrypted.windows(plain.len()).any(|part| part == plain));
        assert_eq!(crypt(&encrypted, false, "fixture-service").unwrap(), plain);
        assert!(crypt(&encrypted, false, "different-service").is_err());
        assert!(crypt(plain, false, "fixture-service").is_err());
    }
    #[test]
    fn pkce_vectors() {
        assert_eq!(encode(b"abc"), "YWJj");
        assert!(opaque(&random()));
        assert_eq!(
            encode(&Sha256::digest(
                b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
            )),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn unconfigured_preview_does_not_lock_users_out() {
        if !enforced() && base().is_err() {
            assert!(require(Path::new(".")).is_ok());
            assert_eq!(status(Path::new("."))["enforced"], false);
        }
    }
}
