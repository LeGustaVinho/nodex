//! # Nodex desktop
//!
//! Runs the Nodex server inside this process and shows its UI in a native window: no command
//! line, no browser. The window loads the server's own origin (`http://127.0.0.1:<port>/`), so
//! the web UI and the server's Host/Origin checks work exactly as in a browser; the session
//! token is injected into that origin only. Native features (folder picker, save dialog,
//! opening links in the system browser) are Tauri plugins granted to that origin alone.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod menu;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nodex_server::{AppState, ServerEvent, ServerHandle, ServerOptions};
use tauri::ipc::CapabilityBuilder;
use tauri::webview::NewWindowResponse;
use tauri::window::Color;
use tauri::{AppHandle, DragDropEvent, Manager, RunEvent, Url, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

/// Label of the one window.
pub const MAIN: &str = "main";

/// Background of the UI (`--bg`), shown before the page paints.
const BACKGROUND: Color = Color(18, 20, 24, 255);

/// What the app keeps while it runs.
pub struct Desktop {
    pub server: Mutex<Option<ServerHandle>>,
    pub state: Arc<AppState>,
    /// Paths behind the File → Open Recent items, by index.
    pub recent: Mutex<Vec<String>>,
    pub zoom: Mutex<f64>,
}

fn main() {
    let log = nodex_server::logging::init(cfg!(debug_assertions));
    nodex_lang_api::process::kill_children_on_exit();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Nodex desktop starting");

    let app = tauri::Builder::default()
        // First, so a second launch hands its folder to this one and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            focus_main(app);
            if let Some(dir) = folder_arg(&argv, Path::new(&cwd)) {
                open_folder(app, dir);
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(setup)
        .on_menu_event(menu::handle)
        .on_window_event(on_window_event)
        .build(tauri::generate_context!())
        .expect("cannot build the Nodex window");

    let code = app.run_return(|app, event| {
        if let RunEvent::Exit = event {
            shutdown(app);
        }
    });
    drop(log);
    std::process::exit(code);
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(dir) = app.path().resource_dir() {
        nodex_lang_api::sidecar::set_resource_dir(dir);
    }
    let handle = app.handle().clone();
    let server = match tauri::async_runtime::block_on(nodex_server::start(ServerOptions::default())) {
        Ok(server) => server,
        Err(e) => {
            tracing::error!("cannot start the Nodex server: {e:#}");
            let url = format!("index.html?error={}", percent_encode(&format!("{e:#}")));
            WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App(url.into())).title("Nodex").background_color(BACKGROUND).build()?;
            return Ok(());
        }
    };
    let origin = format!("http://127.0.0.1:{}", server.port);
    app.add_capability(
        CapabilityBuilder::new("nodex-ui")
            .remote(format!("{origin}/*"))
            .local(false)
            .window(MAIN)
            .permission("core:default")
            .permission("dialog:default")
            .permission("opener:default")
            .permission("clipboard-manager:allow-write-text"),
    )?;
    let state = server.state.clone();
    let token = server.token.clone();
    app.manage(Desktop { server: Mutex::new(Some(server)), state: state.clone(), recent: Mutex::new(Vec::new()), zoom: Mutex::new(1.0) });

    main_window(&handle, &origin, &token)?;
    menu::install(&handle)?;
    follow_project(&handle, state);

    let cwd = std::env::current_dir().unwrap_or_default();
    if let Some(dir) = folder_arg(&std::env::args().collect::<Vec<_>>(), &cwd) {
        open_folder(&handle, dir);
    }
    Ok(())
}

/// The window on the server's origin. Pages elsewhere open in the system browser.
fn main_window(app: &AppHandle, origin: &str, token: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url: Url = format!("{origin}/").parse()?;
    let js = |s: &str| serde_json::to_string(s).unwrap_or_default();
    // Only the Nodex origin gets the token (the script runs on every page load).
    let script = format!("if (location.origin === {}) {{ window.__NODEX_TOKEN__ = {}; }}", js(origin), js(token));
    let allowed = origin.to_owned();
    let nav = app.clone();
    let popup = app.clone();
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::External(url))
        .title("Nodex")
        .inner_size(1440.0, 900.0)
        .min_inner_size(900.0, 600.0)
        .background_color(BACKGROUND)
        .initialization_script(script)
        .on_navigation(move |url| {
            if url.origin().ascii_serialization() == allowed || url.scheme() == "about" {
                return true;
            }
            open_external(&nav, url);
            false
        })
        .on_new_window(move |url, _| {
            open_external(&popup, &url);
            NewWindowResponse::Deny
        })
        .build()?;
    Ok(())
}

/// Opens web and mail links in the system's default app; anything else is ignored.
fn open_external(app: &AppHandle, url: &Url) {
    if matches!(url.scheme(), "http" | "https" | "mailto")
        && let Err(e) = app.opener().open_url(url.as_str(), None::<&str>)
    {
        tracing::warn!(%url, "cannot open the link: {e}");
    }
}

/// Keeps the window title and the Open Recent menu in step with the open folder.
fn follow_project(app: &AppHandle, state: Arc<AppState>) {
    let mut events = state.events.subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match events.recv().await {
                Ok(ServerEvent::ProjectChanged) => {
                    let summary = state.project.lock().map(|p| p.summary()).ok();
                    let title = match summary {
                        Some(s) if s.open => format!("{} — Nodex", s.name),
                        _ => "Nodex".to_owned(),
                    };
                    if let Some(w) = app.get_webview_window(MAIN) {
                        let _ = w.set_title(&title);
                    }
                    if let Err(e) = menu::install(&app) {
                        tracing::warn!("cannot update the menu: {e}");
                    }
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    });
}

/// Opens `dir` as the project, reporting a failure in a dialog.
pub fn open_folder(app: &AppHandle, dir: PathBuf) {
    let Some(desktop) = app.try_state::<Desktop>() else { return };
    let state = desktop.state.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = nodex_server::projects::open(&state, dir.to_string_lossy().into_owned()).await {
            error_dialog(&app, "Cannot open the folder", &e);
        }
    });
}

pub fn error_dialog(app: &AppHandle, title: &str, message: &str) {
    app.dialog().message(message).title(title).kind(MessageDialogKind::Error).show(|_| {});
}

pub fn focus_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The folder named on a command line (`nodex-desktop <folder>`, or Explorer's "Open with").
fn folder_arg(argv: &[String], cwd: &Path) -> Option<PathBuf> {
    argv.iter().skip(1).filter(|a| !a.starts_with('-')).map(|a| cwd.join(a)).find(|p| p.is_dir())
}

fn on_window_event(window: &Window, event: &WindowEvent) {
    // A folder (or a file inside one) dropped on the window opens it.
    if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event
        && let Some(first) = paths.first()
    {
        let dir = if first.is_dir() { Some(first.clone()) } else { first.parent().map(Path::to_path_buf) };
        if let Some(dir) = dir {
            open_folder(window.app_handle(), dir);
        }
    }
}

/// Stops the server and everything it started before the process exits.
fn shutdown(app: &AppHandle) {
    let Some(desktop) = app.try_state::<Desktop>() else { return };
    let server = desktop.server.lock().ok().and_then(|mut s| s.take());
    if let Some(server) = server {
        tauri::async_runtime::block_on(server.shutdown());
    }
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
