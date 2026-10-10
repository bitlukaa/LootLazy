#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod artwork;
mod backend;
mod feed;
mod presence;
mod storage;
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
static CATALOG_REFRESH: Mutex<()> = Mutex::new(());
static EXECUTABLE_LOOKUP: Mutex<Option<std::time::Instant>> = Mutex::new(None);
#[tauri::command]
async fn api(route: String, body: Value, state: tauri::State<'_, Shared>) -> Result<Value, String> {
    if !route.starts_with("/api/") || route.len() > 2000 {
        return Err("Invalid application route".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let url =
            url::Url::parse(&format!("http://localhost{route}")).map_err(|e| e.to_string())?;
        if ["/api/refresh-catalog", "/api/games", "/api/catalog-stats"].contains(&url.path()) {
            let _refresh = CATALOG_REFRESH.lock().map_err(|_| "Catalog unavailable")?;
            let missing = state
                .lock()
                .map_err(|_| "Backend unavailable")?
                .catalog
                .is_empty();
            if missing || url.path() == "/api/refresh-catalog" {
                let data = backend::fetch_json("https://discord.com/api/applications/detectable")?;
                state
                    .lock()
                    .map_err(|_| "Backend unavailable")?
                    .install_catalog(data)?;
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
            && body["executableName"]
                .as_str()
                .is_none_or(|name| name.is_empty())
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
                let needs = state
                    .lock()
                    .map_err(|_| "Backend unavailable")?
                    .missing_game_executable(&id);
                if needs {
                    let _catalog = CATALOG_REFRESH.lock().map_err(|_| "Catalog unavailable")?;
                    let mut last = EXECUTABLE_LOOKUP
                        .lock()
                        .map_err(|_| "Catalog unavailable")?;
                    if last.is_none_or(|time| time.elapsed() >= std::time::Duration::from_secs(60))
                    {
                        *last = Some(std::time::Instant::now());
                        if let Ok(data) =
                            backend::fetch_json("https://discord.com/api/applications/detectable")
                        {
                            let _ = state
                                .lock()
                                .map_err(|_| "Backend unavailable")?
                                .install_catalog(data);
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
    .map_err(|e| e.to_string())?
}
fn root() -> PathBuf {
    let diagnostic = std::env::args().any(|a| a == "--verify-ui" || a == "--verify-backend");
    if diagnostic {
        std::env::var_os("QUESTRO_DATA_ROOT")
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
        let name: Vec<u16> = "Local\\QuestroTauriPortable"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let title: Vec<u16> = "Questro".encode_utf16().chain(Some(0)).collect();
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
    let root = root();
    let temp = root.join("data/temp");
    std::fs::create_dir_all(&temp).expect("Questro folder must be writable");
    for key in ["TEMP", "TMP", "TMPDIR"] {
        std::env::set_var(key, &temp);
    }
    if std::env::args().any(|a| a == "--verify-backend") {
        let state = backend::Backend::new(root.clone()).unwrap();
        let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"root":root,"catalogCount":state.catalog.len(),"runtime":"Tauri + Rust","nodeBundled":false,"storage":root.join("data"),"profile":root.join("data/webview"),"status":state.status()});
        storage::save(&root.join("data/tauri-verification.json"), &result).unwrap();
        return;
    }
    let isolated_ui_test = std::env::args().any(|a| a == "--verify-ui")
        && std::env::var_os("QUESTRO_DATA_ROOT").is_some()
        && root
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("questro-tauri-ui-"))
        && root
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some(".test-temp");
    if !isolated_ui_test && !single_instance() {
        return;
    }
    let mut backend = backend::Backend::new(root.clone()).expect("Cannot load Questro data");
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
        .invoke_handler(tauri::generate_handler![api])
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
            let mut builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Questro")
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
            if verify_ui {
                builder = builder
                    .visible(false)
                    .additional_browser_args("--remote-debugging-port=9387");
            }
            let window = builder.build()?;
            window.set_icon(tauri::image::Image::from_app_icon_resource(32)?)?;
            use tauri::{
                menu::{Menu, MenuItem},
                tray::TrayIconBuilder,
            };
            let show = MenuItem::with_id(app, "show", "Show Questro", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &exit])?;
            TrayIconBuilder::new()
                .icon(tauri::image::Image::from_app_icon_resource(32)?)
                .menu(&menu)
                .tooltip("Questro")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Questro startup failed");
    app.run(move |_app, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            running.store(false, Ordering::SeqCst);
            quit_state.lock().unwrap().shutdown();
        }
    });
}
