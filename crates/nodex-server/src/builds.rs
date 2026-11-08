//! Builds the structure graph and knowledge without holding the project lock.
//!
//! Linking a large project takes long enough that holding the lock meanwhile would stall every
//! other request (the project summary, layouts, the Unity card's poll). So the input is taken
//! under the lock in a moment, the build runs on a blocking thread, and the result is kept
//! only if the project did not change in between. Concurrent requests wait for the same build,
//! and the JSON of each payload is serialized once per build.
//!
//! [`prebuild`] starts that work as soon as a project is opened or changes, so the UI's first
//! requests usually find everything ready.

use std::sync::Arc;

use axum::body::Bytes;
use nodex_ir::StructureGraph;
use nodex_ir::knowledge::ProjectKnowledge;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::AppState;

/// Serialized payloads of the current build. Each slot stays locked while it is serialized,
/// so concurrent requests (and the background build) serialize it once.
#[derive(Default)]
pub struct JsonCache {
    structure: Mutex<Option<(Arc<StructureGraph>, Bytes)>>,
    knowledge: Mutex<Option<(Arc<ProjectKnowledge>, Bytes)>>,
}

/// The current structure graph and knowledge, building them (once, off the lock) if needed.
pub async fn graphs(state: &Arc<AppState>) -> Result<(Arc<StructureGraph>, Arc<ProjectKnowledge>), String> {
    loop {
        if let Some(built) = state.project.lock().map_err(|_| "project lock poisoned")?.built() {
            return Ok(built);
        }
        // One build at a time; whoever waited here finds it done.
        let _building = state.build_lock.lock().await;
        let input = {
            let project = state.project.lock().map_err(|_| "project lock poisoned")?;
            if let Some(built) = project.built() {
                return Ok(built);
            }
            project.build_input()
        };
        let built = tokio::task::spawn_blocking(move || input.build()).await.map_err(|e| e.to_string())?;
        let mut project = state.project.lock().map_err(|_| "project lock poisoned")?;
        if project.install_build(built) {
            if let Some(built) = project.built() {
                return Ok(built);
            }
        }
        // A file or semantic facts changed during the build: build again from the new state.
    }
}

/// `/api/structure` as JSON bytes.
pub async fn structure_json(state: &Arc<AppState>) -> Result<Bytes, String> {
    let (graph, _) = graphs(state).await?;
    cached_json(&state.json.structure, graph, "api.structure.json").await
}

/// `/api/knowledge` as JSON bytes.
pub async fn knowledge_json(state: &Arc<AppState>) -> Result<Bytes, String> {
    let (_, knowledge) = graphs(state).await?;
    cached_json(&state.json.knowledge, knowledge, "api.knowledge.json").await
}

async fn cached_json<T: Serialize + Send + Sync + 'static>(
    slot: &Mutex<Option<(Arc<T>, Bytes)>>,
    value: Arc<T>,
    phase: &'static str,
) -> Result<Bytes, String> {
    let mut slot = slot.lock().await;
    if let Some((cached, bytes)) = slot.as_ref()
        && Arc::ptr_eq(cached, &value)
    {
        return Ok(bytes.clone());
    }
    let for_thread = value.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let timer = nodex_core::perf::phase(phase);
        let bytes = serde_json::to_vec(&*for_thread).map(Bytes::from);
        if let Ok(b) = &bytes {
            timer.finish(format_args!("({:.1} MB)", b.len() as f64 / 1_048_576.0));
        }
        bytes
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    *slot = Some((value, bytes.clone()));
    Ok(bytes)
}

/// Builds the graphs and their JSON in the background (after an open or a change), so the
/// UI's requests find them ready. Does nothing outside a Tokio runtime.
pub fn prebuild(state: &Arc<AppState>) {
    let Some(runtime) = state.runtime.clone() else { return };
    if !state.project.lock().is_ok_and(|p| p.is_open()) {
        return;
    }
    let state = state.clone();
    runtime.spawn(async move {
        if let Err(e) = structure_json(&state).await {
            tracing::warn!("background build failed: {e}");
            return;
        }
        let _ = knowledge_json(&state).await;
    });
}
