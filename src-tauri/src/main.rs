#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod artwork;
mod backend;
mod diagnostics;
mod executables;
mod feed;
mod metadata;
mod platform_download;
mod platform_environment;
mod presence;
mod rotation;
mod steam_accounts;
mod steam_client;
mod steam_runtime;
mod steam_store;
mod storage;
mod update_signature;
mod updates;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
type Shared = Arc<Mutex<backend::Backend>>;
fn trusted_main_label(label: &str) -> bool {
    label == "main"
}
fn require_main_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    if trusted_main_label(window.label()) {
        Ok(())
    } else {
        Err("Untrusted application window".into())
    }
}

// Lower WebView memory pressure in the tray without suspending scripts or native timers.
fn set_webview_memory(app: &tauri::AppHandle, inactive: bool) {
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.with_webview(move |view| {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
            };
            use windows_core::Interface;
            unsafe {
                if let Ok(core) = view.controller().CoreWebView2() {
                    if let Ok(memory) = core.cast::<ICoreWebView2_19>() {
                        let _ = memory.SetMemoryUsageTargetLevel(
                            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(if inactive { 1 } else { 0 }),
                        );
                    }
                }
            }
        });
    }
}

#[tauri::command]
fn quit(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    require_main_window(&window)?;
    app.exit(0);
    Ok(())
}
#[tauri::command]
fn window_close(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Shared>,
) -> Result<(), String> {
    require_main_window(&window)?;
    let minimize_to_tray = state
        .lock()
        .ok()
        .map(|b| {
            b.overrides["settings"]["minimize_to_tray"]
                .as_bool()
                .unwrap_or(false)
        })
        .unwrap_or(false);
    if minimize_to_tray {
        let _ = window.hide();
        set_webview_memory(window.app_handle(), true);
    } else {
        window.app_handle().exit(0);
    }
    Ok(())
}
#[tauri::command]
fn notify_desktop(title: String, body: String, window: tauri::WebviewWindow) -> Result<(), String> {
    require_main_window(&window)?;
    let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
    #[cfg(windows)]
    {
        std::thread::spawn(move || {
            use std::os::windows::process::CommandExt;
            let script = format!(
                "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
                 $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
                 $s = $t.GetElementsByTagName('text'); \
                 $s.Item(0).AppendChild($t.CreateTextNode('{}')) | Out-Null; \
                 $s.Item(1).AppendChild($t.CreateTextNode('{}')) | Out-Null; \
                 $n = [Windows.UI.Notifications.ToastNotification]::new($t); \
                 [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('LootLazy').Show($n)",
                title.replace('\'', "''"),
                body.replace('\'', "''")
            );
            let _ = std::process::Command::new("powershell")
                .env_remove("PSModulePath")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
                .creation_flags(0x08000000)
                .status();
        });
    }
    Ok(())
}
static UPDATING: AtomicBool = AtomicBool::new(false);
static CATALOG_REFRESH: Mutex<()> = Mutex::new(());
static EXECUTABLE_LOOKUP: Mutex<Option<std::collections::HashMap<String, std::time::Instant>>> =
    Mutex::new(None);
fn refresh_game_catalog(state: &Shared) -> Result<(), String> {
    let (meta, has_saved) = {
        let backend = state.lock().map_err(|_| "Backend unavailable")?;
        (backend.meta.clone(), !backend.catalog.is_empty())
    };
    let download = backend::fetch_catalog(&meta, has_saved)?;
    state
        .lock()
        .map_err(|_| "Backend unavailable")?
        .apply_catalog_refresh(download)
}
#[tauri::command]
async fn api(
    route: String,
    body: Value,
    state: tauri::State<'_, Shared>,
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<Value, String> {
    require_main_window(&window)?;
    if !route.starts_with("/api/") || route.len() > 2000 {
        return Err("Invalid application route".into());
    }
    if UPDATING.load(Ordering::SeqCst) {
        return Err("updates_busy".into());
    }
    let state = state.inner().clone();
    let log_root = state
        .lock()
        .map_err(|_| "Backend unavailable")?
        .root
        .clone();
    let log_route = route.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let url =
            url::Url::parse(&format!("http://localhost{route}")).map_err(|e| e.to_string())?;
        if url.path() == "/api/steam-accounts/search" {
            let root = state.lock().map_err(|_| "Backend unavailable")?.root.clone();
            let query = body["query"].as_str().unwrap_or("").trim();
            if query.is_empty() { return steam_accounts::request(&root, "library", body); }
            let candidates = steam_store::search(query)?;
            let mut owned = body.clone(); owned["query"] = serde_json::json!("");
            owned["appIds"] = serde_json::json!(candidates.iter().filter_map(|game|game["steamId"].as_str()?.parse::<u32>().ok()).collect::<Vec<_>>());
            let library = steam_accounts::request(&root, "library", owned)?;
            let games = candidates.into_iter().map(|mut game| {
                let id = game["steamId"].as_str().unwrap_or("").parse::<u64>().unwrap_or(0);
                game["owned"] = serde_json::json!(library["games"].as_array().is_some_and(|items| items.iter().any(|item| item["id"].as_u64() == Some(id))));
                game
            }).collect::<Vec<_>>();
            return Ok(serde_json::json!({"games":games,"loading":library["loading"],"error":library["error"]}));
        }
        if url.path() == "/api/steam-accounts/store" {
            platform_download::open_store(body["id"].as_u64().filter(|id| *id > 0 && *id <= u32::MAX as u64).ok_or("accounts_invalid")? as u32)?;
            return Ok(serde_json::json!({"ok":true}));
        }
        if let Some(command) = url.path().strip_prefix("/api/steam-accounts/") {
            let root = state.lock().map_err(|_| "Backend unavailable")?.root.clone();
            return steam_accounts::request(&root, command, body);
        }
        match url.path() {
            "/api/platforms/status" => return Ok(serde_json::json!({"discordInstalled":presence::installed(),"steamInstalled":steam_client::running() || steam_client::installed()})),
            "/api/platforms/download" => {
                platform_download::open(body["platform"].as_str().unwrap_or(""))?;
                return Ok(serde_json::json!({"success":true}));
            }
            "/api/updates" | "/api/updates/check" | "/api/updates/channel" => {
                let root = state.lock().map_err(|_| "Backend unavailable")?.root.clone();
                return match url.path() {
                    "/api/updates/check" => Ok(updates::check_for(&root)),
                    "/api/updates/channel" => updates::set_channel(&root, body["channel"].as_str().unwrap_or("")),
                    _ => Ok(updates::info_for(&root)),
                };
            }
            "/api/updates/install" => {
                let (root, busy) = {
                    let backend = state.lock().map_err(|_| "Backend unavailable")?;
                    (
                        backend.root.clone(),
                        backend.session.is_some()
                            || backend.queue.active()
                            || backend.quest_queue.active()
                            || backend.variety.active()
                            || backend.steam_idle.active(),
                    )
                };
                if busy || steam_accounts::active() {
                    return Err("updates_stop_sessions".into());
                }
                if UPDATING.swap(true, Ordering::SeqCst) {
                    return Err("updates_busy".into());
                }
                let result = (|| {
                    steam_accounts::shutdown();
                    updates::stage(&root)?;
                    updates::launch_installer(&root)?;
                    state.lock().map_err(|_| "Backend unavailable")?.shutdown();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(350));
                        app.exit(0);
                    });
                    Ok(serde_json::json!({"success":true,"restarting":true}))
                })();
                if result.is_err() {
                    UPDATING.store(false, Ordering::SeqCst);
                }
                return result;
            }
            _ => {}
        }
        if [
            "/api/refresh-catalog",
            "/api/games",
            "/api/catalog-stats",
            "/api/variety/shuffle",
            "/api/steam/games",
        ]
        .contains(&url.path())
        {
            let _refresh = CATALOG_REFRESH.lock().map_err(|_| "Catalog unavailable")?;
            let missing = state
                .lock()
                .map_err(|_| "Backend unavailable")?
                .catalog
                .is_empty();
            if missing || url.path() == "/api/refresh-catalog" {
                refresh_game_catalog(&state)?;
            }
        }
        if url.path() == "/api/steam/games" {
            let query = url
                .query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_default();
            if !query.trim().is_empty() {
                if let Ok(games) = steam_store::search(&query) {
                    state
                        .lock()
                        .map_err(|_| "Backend unavailable")?
                        .remember_steam_search(games);
                }
            }
        }
        if url.path() == "/api/quests"
            && url
                .query_pairs()
                .any(|(k, v)| k == "refresh" && v == "true")
        {
            let data = backend::fetch_json(feed::SOURCE)?;
            state
                .lock()
                .map_err(|_| "Backend unavailable")?
                .install_quests(data)?;
        }
        if ["/api/play", "/api/quests/start"].contains(&url.path())
            && body["customProcess"] != true
            && (body["executableAutomatic"] == true
                || body["executableName"]
                    .as_str()
                    .is_none_or(|name| name.is_empty()))
        {
            let app_id = {
                let backend = state.lock().map_err(|_| "Backend unavailable")?;
                if url.path() == "/api/play" {
                    body["appId"].as_str().map(str::to_string)
                } else {
                    backend.quest_application(body["questId"].as_str().unwrap_or(""))
                }
            };
            if let Some(id) = app_id {
                let (missing, steam_id) = {
                    let backend = state.lock().map_err(|_| "Backend unavailable")?;
                    (
                        backend.missing_game_executable(&id),
                        backend.metadata_lookup(&id),
                    )
                };
                if missing || steam_id.is_some() {
                    let _catalog = CATALOG_REFRESH.lock().map_err(|_| "Catalog unavailable")?;
                    let mut last = EXECUTABLE_LOOKUP
                        .lock()
                        .map_err(|_| "Catalog unavailable")?;
                    let last = last.get_or_insert_with(std::collections::HashMap::new);
                    if last
                        .get(&id)
                        .is_none_or(|time| time.elapsed() >= std::time::Duration::from_secs(60))
                    {
                        if last.len() > 256 {
                            last.clear();
                        }
                        last.insert(id.clone(), std::time::Instant::now());
                        if missing {
                            let _ = refresh_game_catalog(&state);
                        }
                        if let Some(steam_id) = steam_id {
                            match metadata::steam_launch_names(&steam_id) {
                                Ok(names) => state
                                    .lock()
                                    .map_err(|_| "Backend unavailable")?
                                    .remember_metadata(&id, &steam_id, names),
                                Err(error) => {
                                    let root = state
                                        .lock()
                                        .map_err(|_| "Backend unavailable")?
                                        .root
                                        .clone();
                                    diagnostics::record(
                                        &root,
                                        "executable_metadata/lookup",
                                        &error,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        state
            .lock()
            .map_err(|_| "Backend unavailable")?
            .route(&route, body)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Err(error) = &result {
        diagnostics::record(&log_root, &log_route, error);
    }
    result.map_err(|error| format!("{}: {}", diagnostics::code(&error), error))
}
fn root() -> PathBuf {
    let diagnostic = std::env::args().any(|a| a == "--verify-ui" || a == "--verify-backend");
    if diagnostic {
        std::env::var_os("LOOTLAZY_DATA_ROOT")
    } else {
        None
    }
    .map(PathBuf::from)
    .unwrap_or_else(|| {
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    })
}
fn single_instance() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
            System::Threading::CreateMutexW,
            UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE},
        };
        let name: Vec<u16> = "Local\\LootLazyPortable"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let title: Vec<u16> = "LootLazy".encode_utf16().chain(Some(0)).collect();
            let win = FindWindowW(std::ptr::null(), title.as_ptr());
            if !win.is_null() {
                ShowWindow(win, SW_RESTORE);
                SetForegroundWindow(win);
            }
            return false;
        }
    }
    true
}
fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .get(1)
        .is_some_and(|a| a == "--steam-session-worker")
    {
        std::process::exit(rotation::worker(&arguments));
    }
    if arguments
        .get(1)
        .is_some_and(|arg| arg == "--verify-update-signature")
        && arguments.len() == 5
    {
        use sha2::{Digest, Sha256};
        let result = (|| -> Result<(), String> {
            let path = std::path::Path::new(&arguments[2]);
            if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
                return Err("File too large".into());
            }
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let document = std::fs::read(&arguments[3]).map_err(|e| e.to_string())?;
            update_signature::verify(
                &document,
                &arguments[4],
                &format!("{:x}", Sha256::digest(&bytes)),
                bytes.len() as u64,
            )
        })();
        std::process::exit(if result.is_ok() { 0 } else { 2 });
    }
    if arguments
        .get(1)
        .is_some_and(|arg| arg == "--verify-steam-runtime")
        && arguments.len() == 3
    {
        std::process::exit(
            if steam_runtime::lock_verified(std::path::Path::new(&arguments[2]), None).is_ok() {
                0
            } else {
                2
            },
        );
    }
    let root = root();
    platform_environment::restore_inherited_temp(&root);
    if std::env::args().any(|a| a == "--verify-backend") {
        let state = backend::Backend::new(root.clone()).unwrap();
        let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"root":root,"catalogCount":state.catalog.len(),"runtime":"Tauri + Rust","nodeBundled":false,"temporaryDirectory":std::env::temp_dir(),"storage":root.join("data"),"profile":root.join("data/webview"),"status":state.status()});
        storage::save(&root.join("data/tauri-verification.json"), &result).unwrap();
        return;
    }
    let isolated_ui_test = std::env::args().any(|a| a == "--verify-ui")
        && std::env::var_os("LOOTLAZY_DATA_ROOT").is_some()
        && root
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("lootlazy-tauri-ui-"))
        && root
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some(".test-temp");
    if !isolated_ui_test && !single_instance() {
        return;
    }
    let mut backend = backend::Backend::new(root.clone()).expect("Cannot load LootLazy data");
    if isolated_ui_test {
        backend.disable_presence_for_diagnostics();
    }
    let state = Arc::new(Mutex::new(backend));
    let running = Arc::new(AtomicBool::new(true));
    let ticker = state.clone();
    let tick_running = running.clone();
    std::thread::spawn(move || {
        while tick_running.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if !tick_running.load(Ordering::SeqCst) {
                break;
            }
            ticker.lock().unwrap().tick();
        }
    });
    let art_root = root.clone();
    let art_workers = artwork::DownloadPool::new(4);
    let setup_root = root.clone();
    let quit_state = state.clone();
    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            api,
            quit,
            window_close,
            notify_desktop
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<Shared>();
                let minimize_to_tray = state
                    .lock()
                    .ok()
                    .map(|b| {
                        b.overrides["settings"]["minimize_to_tray"]
                            .as_bool()
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    set_webview_memory(window.app_handle(), true);
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .register_asynchronous_uri_scheme_protocol(
            "artwork",
            move |_context, request, responder| {
                let root = art_root.clone();
                art_workers.dispatch(move || {
                    let value = url::Url::parse(&request.uri().to_string())
                        .ok()
                        .and_then(|u| {
                            u.query_pairs()
                                .find(|(k, _)| k == "url")
                                .map(|(_, v)| v.into_owned())
                        });
                    let result = value
                        .ok_or("Missing artwork".to_string())
                        .and_then(|v| artwork::get(&root, &v));
                    let response = match result {
                        Ok((bytes, kind)) => tauri::http::Response::builder()
                            .status(200)
                            .header(
                                "Content-Type",
                                if kind == "jpg" {
                                    "image/jpeg"
                                } else if kind == "webp" {
                                    "image/webp"
                                } else {
                                    "image/png"
                                },
                            )
                            .header("Cache-Control", "no-store")
                            .body(bytes)
                            .unwrap(),
                        Err(_) => tauri::http::Response::builder()
                            .status(404)
                            .body(vec![])
                            .unwrap(),
                    };
                    responder.respond(response);
                });
            },
        )
        .setup(move |app| {
            let verify_ui = std::env::args().any(|a| a == "--verify-ui");
            let start_minimized = std::env::args().any(|a| a == "--minimized");
            let mut builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("LootLazy")
                    .inner_size(1100., 750.)
                    .min_inner_size(800., 600.)
                    .decorations(false)
                    .data_directory(setup_root.join("data/webview"))
                    .on_navigation(|url| {
                        url.port().is_none()
                            && ((url.scheme() == "http"
                                && url.host_str() == Some("tauri.localhost"))
                                || (url.scheme() == "tauri" && url.host_str() == Some("localhost")))
                    });
            if verify_ui || start_minimized {
                builder = builder.visible(false);
            }
            if verify_ui {
                builder = builder
                    .initialization_script("window.lootlazyDiagnostic = true;")
                    .additional_browser_args("--remote-debugging-port=9387");
            }
            let window = builder.build()?;
            window.set_icon(tauri::image::Image::from_app_icon_resource(32)?)?;
            if verify_ui || start_minimized {
                set_webview_memory(app.handle(), true);
            }
            use tauri::{
                menu::{Menu, MenuItem},
                tray::TrayIconBuilder,
            };
            let show = MenuItem::with_id(app, "show", "Show LootLazy", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &exit])?;
            TrayIconBuilder::new()
                .icon(tauri::image::Image::from_app_icon_resource(32)?)
                .menu(&menu)
                .tooltip("LootLazy")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            set_webview_memory(w.app_handle(), false);
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            set_webview_memory(w.app_handle(), false);
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("LootLazy startup failed");
    app.run(move |_app, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            steam_accounts::shutdown();
            running.store(false, Ordering::SeqCst);
            quit_state.lock().unwrap().shutdown();
        }
    });
}

#[cfg(test)]
mod window_security_tests {
    #[test]
    fn external_login_windows_cannot_access_main_commands() {
        assert!(super::trusted_main_label("main"));
        for name in ["steam-login", "main/steam", "", "Main"] {
            assert!(!super::trusted_main_label(name));
        }
    }
}
