//! Runs semantic providers on background threads.
//!
//! The UI is usable immediately with syntax-only graphs; when a provider finishes (seconds for
//! MSBuild), its facts are applied and browsers are told to refresh. File changes are forwarded
//! to the providers so the facts stay current.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use nodex_ir::semantic::SemanticState;

use crate::{AppState, ServerEvent};

/// Extra quiet time before re-analyzing, so a burst of saves costs one analysis.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Starts one worker per language that has a semantic provider (none when `NODEX_NO_SEMANTIC`
/// is set: graphs stay syntax-only, as in tests).
pub fn spawn(state: &Arc<AppState>) {
    if std::env::var_os("NODEX_NO_SEMANTIC").is_some() {
        return;
    }
    let (root, frontends) = match state.project.lock() {
        Ok(p) => (p.root().to_path_buf(), p.active_frontends()),
        Err(_) => return,
    };
    let generation = state.generation.load(std::sync::atomic::Ordering::SeqCst);
    for frontend in frontends {
        let (tx, rx) = mpsc::channel::<(u64, Vec<PathBuf>)>();
        if let Ok(mut senders) = state.semantic_tx.lock() {
            senders.push(tx);
        }
        let state = state.clone();
        let root = root.clone();
        let language = frontend.id();
        let spawned = std::thread::Builder::new().name(format!("nodex-semantic-{language:?}")).spawn(move || {
            // Another project was opened meanwhile: this worker's results belong to the old one.
            let current = || state.generation.load(std::sync::atomic::Ordering::SeqCst) == generation;
            let set = |s: SemanticState, detail: String| {
                let graph = {
                    let Ok(mut project) = state.project.lock() else { return };
                    if !current() { return }
                    project.set_semantic_status(language, s, detail)
                };
                if graph {
                    crate::builds::prebuild(&state);
                }
                let _ = state.events.send(ServerEvent::SemanticChanged { graph });
            };
            set(SemanticState::Loading, "loading the project for semantic analysis…".into());
            let mut revision = match state.project.lock() {
                Ok(project) if current() => project.source_revision(),
                _ => return,
            };
            let mut provider = match frontend.semantic(&root) {
                None => {
                    set(SemanticState::Unavailable, "no semantic provider for this language yet".into());
                    return;
                }
                Some(Err(reason)) => {
                    tracing::warn!(?language, %reason, "semantic analysis unavailable");
                    set(SemanticState::Unavailable, reason);
                    return;
                }
                Some(Ok(p)) => p,
            };
            tracing::info!(?language, provider = %provider.describe(), "semantic provider started");
            let analyzing = nodex_core::perf::phase("semantic.analyze");
            let mut result = provider.analyze();
            analyzing.finish(format_args!("({language:?})"));
            loop {
                match result {
                    Ok(facts) => {
                        let applied = {
                            let Ok(mut project) = state.project.lock() else { return };
                            if !current() { return }
                            if project.source_revision() == revision {
                                tracing::info!(?language, refs = facts.refs.len(), "semantic facts ready");
                                let applying = nodex_core::perf::phase("semantic.apply");
                                project.apply_semantic(language, facts);
                                applying.finish(format_args!("({language:?})"));
                                true
                            } else {
                                false
                            }
                        };
                        if applied {
                            crate::builds::prebuild(&state);
                            let _ = state.events.send(ServerEvent::SemanticChanged { graph: true });
                        }
                    }
                    Err(reason) => {
                        tracing::warn!(?language, %reason, "semantic analysis failed");
                        set(SemanticState::Unavailable, reason);
                        return;
                    }
                }
                // Wait for file changes; merge bursts.
                let Ok((mut next_revision, mut changed)) = rx.recv() else { return };
                while let Ok((next, more)) = rx.recv_timeout(DEBOUNCE) {
                    next_revision = next_revision.max(next);
                    changed.extend(more);
                }
                revision = next_revision;
                changed.sort();
                changed.dedup();
                result = provider.update(&changed);
            }
        });
        if let Err(e) = spawned {
            tracing::warn!("cannot start semantic worker: {e}");
        }
    }
}

/// Forwards changed files (absolute paths) to every semantic worker.
pub fn notify(state: &AppState, revision: u64, changed: &[PathBuf]) {
    if changed.is_empty() {
        return;
    }
    if let Ok(senders) = state.semantic_tx.lock() {
        for tx in senders.iter() {
            let _ = tx.send((revision, changed.to_vec()));
        }
    }
}
