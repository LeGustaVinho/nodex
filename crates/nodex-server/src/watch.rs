//! File watching: re-lowers changed files and notifies browsers.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use crate::{AppState, ServerEvent, semantic_worker};

/// Quiet period that groups bursts of file-system events (editors save in several steps).
const DEBOUNCE: Duration = Duration::from_millis(150);

/// Starts watching the project root on a background thread. The returned watcher must be kept
/// alive for as long as watching should continue.
pub fn spawn(state: Arc<AppState>) -> notify::Result<notify::RecommendedWatcher> {
    let root = state.project.lock().map(|p| p.root().to_path_buf()).unwrap_or_default();
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            for p in event.paths {
                let _ = tx.send(p);
            }
        }
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;

    std::thread::Builder::new()
        .name("nodex-watch".into())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = BTreeSet::from([first]);
                while let Ok(p) = rx.recv_timeout(DEBOUNCE) {
                    batch.insert(p);
                }
                let (changed, changed_paths): (Vec<String>, Vec<PathBuf>) = match state.project.lock() {
                    Ok(mut project) => batch
                        .iter()
                        .filter_map(|p| project.refresh_path(p).map(|rel| (rel, p.clone())))
                        .unzip(),
                    Err(_) => break,
                };
                semantic_worker::notify(&state, &changed_paths);
                state.run.files_changed(&state, &changed_paths);
                if !changed.is_empty() {
                    tracing::info!(files = ?changed, "re-lowered");
                    let _ = state.events.send(ServerEvent::GraphChanged { files: changed });
                }
            }
        })
        .map_err(|e| notify::Error::generic(&e.to_string()))?;
    Ok(watcher)
}
