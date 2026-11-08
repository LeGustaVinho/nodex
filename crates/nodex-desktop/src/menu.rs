//! The native menu: File (open, recent, close, exit), View (reload, zoom) and Help.

use tauri::menu::{Menu, MenuBuilder, MenuEvent, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::{Desktop, MAIN, error_dialog, open_folder};

const DOCS_URL: &str = "https://github.com/LeGustaVinho/nodex#readme";
const RECENT_ITEMS: usize = 10;
const ZOOM_STEP: f64 = 0.1;

/// Builds the menu (again: Open Recent and Close Folder follow the open folder).
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = build(app)?;
    app.set_menu(menu)?;
    Ok(())
}

fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let desktop = app.state::<Desktop>();
    let open = desktop.state.project.lock().map(|p| p.is_open()).unwrap_or(false);
    let recent = nodex_server::projects::recent();
    if let Ok(mut paths) = desktop.recent.lock() {
        *paths = recent.iter().take(RECENT_ITEMS).map(|r| r.path.clone()).collect();
    }

    let file = SubmenuBuilder::new(app, "&File")
        .item(&MenuItemBuilder::with_id("open-folder", "&Open Folder…").accelerator("CmdOrCtrl+O").build(app)?)
        .item(&recent_menu(app, &recent)?)
        .item(&MenuItemBuilder::with_id("close-folder", "&Close Folder").enabled(open).build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("settings", "&Settings…").accelerator("CmdOrCtrl+,").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("exit", "E&xit").build(app)?)
        .build()?;

    let mut view = SubmenuBuilder::new(app, "&View")
        .item(&MenuItemBuilder::with_id("reload", "&Reload").accelerator("CmdOrCtrl+Shift+R").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("zoom-in", "Zoom &In").accelerator("CmdOrCtrl+=").build(app)?)
        .item(&MenuItemBuilder::with_id("zoom-out", "Zoom &Out").accelerator("CmdOrCtrl+-").build(app)?)
        .item(&MenuItemBuilder::with_id("zoom-reset", "&Actual Size").accelerator("CmdOrCtrl+0").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, Some("Toggle &Full Screen"))?);
    if cfg!(debug_assertions) {
        view = view.separator().item(&MenuItemBuilder::with_id("devtools", "&Developer Tools").accelerator("CmdOrCtrl+Shift+I").build(app)?);
    }
    let view = view.build()?;

    let help = SubmenuBuilder::new(app, "&Help")
        .text("docs", "&Documentation")
        .text("logs", "Open &Logs Folder")
        .separator()
        .text("updates", "Check for &Updates…")
        .text("about", "&About Nodex")
        .build()?;

    MenuBuilder::new(app).items(&[&file, &view, &help]).build()
}

fn recent_menu(app: &AppHandle, recent: &[nodex_server::projects::RecentProject]) -> tauri::Result<Submenu<Wry>> {
    let mut menu = SubmenuBuilder::new(app, "Open &Recent");
    if recent.is_empty() {
        menu = menu.item(&MenuItemBuilder::with_id("recent-none", "No recent folders").enabled(false).build(app)?);
    }
    for (i, r) in recent.iter().take(RECENT_ITEMS).enumerate() {
        // `&` marks a mnemonic in menu labels; double it in names and paths.
        menu = menu.text(format!("recent-{i}"), format!("{}    {}", r.name, r.path).replace('&', "&&"));
    }
    menu.build()
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        "open-folder" => {
            let app2 = app.clone();
            app.dialog().file().set_title("Open a project folder").pick_folder(move |folder| {
                if let Some(path) = folder.and_then(|f| f.into_path().ok()) {
                    open_folder(&app2, path);
                }
            });
        }
        "close-folder" => {
            let desktop = app.state::<Desktop>();
            if let Err(e) = nodex_server::projects::close(&desktop.state) {
                error_dialog(app, "Cannot close the folder", &e);
            }
        }
        "exit" => app.exit(0),
        "settings" => {
            // The web UI opens its Settings dialog on this event.
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.eval("window.dispatchEvent(new Event('nodex:settings'))");
            }
        }
        "updates" => {
            tauri::async_runtime::spawn(crate::updates::check(app.clone(), true));
        }
        "reload" => {
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.eval("location.reload()");
            }
        }
        "zoom-in" | "zoom-out" | "zoom-reset" => zoom(app, id),
        // Developer tools exist in debug builds only (the menu item too).
        #[cfg(debug_assertions)]
        "devtools" => {
            if let Some(w) = app.get_webview_window(MAIN) {
                w.open_devtools();
            }
        }
        "docs" => {
            let _ = app.opener().open_url(DOCS_URL, None::<&str>);
        }
        "logs" => {
            let dir = nodex_server::logging::logs_dir();
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = app.opener().open_path(dir.to_string_lossy(), None::<&str>) {
                error_dialog(app, "Cannot open the logs folder", &e.to_string());
            }
        }
        "about" => {
            let text = format!(
                "Nodex {}\n\nRead code in any language as node graphs.\n\nMIT License",
                env!("CARGO_PKG_VERSION")
            );
            app.dialog().message(text).title("About Nodex").show(|_| {});
        }
        _ => {
            if let Some(i) = id.strip_prefix("recent-").and_then(|i| i.parse::<usize>().ok()) {
                let path = app.state::<Desktop>().recent.lock().ok().and_then(|r| r.get(i).cloned());
                if let Some(path) = path {
                    open_folder(app, path.into());
                }
            }
        }
    }
}

fn zoom(app: &AppHandle, id: &str) {
    let desktop = app.state::<Desktop>();
    let Ok(mut zoom) = desktop.zoom.lock() else { return };
    *zoom = match id {
        "zoom-in" => (*zoom + ZOOM_STEP).min(3.0),
        "zoom-out" => (*zoom - ZOOM_STEP).max(0.5),
        _ => 1.0,
    };
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.set_zoom(*zoom);
    }
}
