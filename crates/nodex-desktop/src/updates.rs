//! Automatic updates from GitHub Releases (tauri-plugin-updater).
//!
//! The release workflow publishes `latest.json` with the installer and its signature; the app
//! accepts an update only if the signature matches the public key in `tauri.conf.json`. Checks
//! run at start-up (release builds, when Settings allow it) and from Help → Check for Updates.

use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

use crate::error_dialog;

/// Wait after start-up before the automatic check, so it never slows the first screen.
const STARTUP_DELAY: Duration = Duration::from_secs(8);

/// Checks once after start-up when the user allows it (never in debug builds).
pub fn check_at_startup(app: &AppHandle) {
    if cfg!(debug_assertions) || !nodex_server::settings::load().check_updates {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        check(app, false).await;
    });
}

/// Looks for a newer version and offers to install it. A manual check also reports "up to date"
/// and errors; the automatic one only logs them.
pub async fn check(app: AppHandle, manual: bool) {
    let shutdown_app = app.clone();
    let updater = app
        .updater_builder()
        // Windows: the updater runs the installer and exits the process. Stop the server first,
        // then let the installer outlive Nodex (the job object would kill it on exit).
        .on_before_exit(move || {
            crate::shutdown(&shutdown_app);
            nodex_lang_api::process::let_children_outlive_exit();
        })
        .build();
    let result = match updater {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let current = env!("CARGO_PKG_VERSION");
    match result {
        Ok(Some(update)) => {
            tracing::info!(current, available = %update.version, "update available");
            let question = format!(
                "Nodex {} is available (you have {current}).\n\nInstall it now? Nodex closes, updates and opens again.",
                update.version
            );
            if !ask(&app, "Update available", &question, "Install and restart", "Later").await {
                return;
            }
            if let Err(e) = update.download_and_install(|_, _| {}, || tracing::info!("update downloaded")).await {
                tracing::warn!("update failed: {e}");
                error_dialog(&app, "Update failed", &format!("The update could not be installed: {e}"));
                return;
            }
            // Other systems install in place; Windows has already exited to run the installer.
            crate::shutdown(&app);
            nodex_lang_api::process::let_children_outlive_exit();
            app.restart();
        }
        Ok(None) => {
            tracing::info!(current, "no update");
            if manual {
                app.dialog()
                    .message(format!("You have the latest version of Nodex ({current})."))
                    .title("No update")
                    .kind(MessageDialogKind::Info)
                    .show(|_| {});
            }
        }
        Err(e) => {
            tracing::warn!("update check failed: {e}");
            if manual {
                error_dialog(&app, "Cannot check for updates", &e.to_string());
            }
        }
    }
}

async fn ask(app: &AppHandle, title: &str, text: &str, yes: &str, no: &str) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(text)
        .title(title)
        .buttons(MessageDialogButtons::OkCancelCustom(yes.to_owned(), no.to_owned()))
        .show(move |answer| {
            let _ = tx.send(answer);
        });
    rx.await.unwrap_or(false)
}
