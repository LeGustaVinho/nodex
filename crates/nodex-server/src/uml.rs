//! UML diagrams over HTTP: `GET /api/uml` and `GET /api/uml/availability`.
//!
//! The project lock is held only to take the (cached) structure and knowledge graphs, and then
//! briefly for each function body an activity or sequence diagram reads; diagrams are built on a
//! blocking thread and cached until the graphs change.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use nodex_core::units::BuildUnit;
use nodex_ir::StructureGraph;
use nodex_ir::knowledge::ProjectKnowledge;
use nodex_uml::{Runtime, UmlAvailability, UmlDiagram, UmlError, UmlInput, UmlKind, UmlOptions, UmlScope};
use serde::Deserialize;

use crate::AppState;

const CACHE_SIZE: usize = 32;

enum Cached {
    Diagram(Arc<UmlDiagram>),
    Availability(Arc<UmlAvailability>),
}

/// Recent diagrams, valid while the structure graph they were built from is current.
#[derive(Default)]
pub struct UmlCache {
    entries: Mutex<VecDeque<(String, Arc<StructureGraph>, Cached)>>,
}

impl UmlCache {
    fn get(&self, key: &str, graph: &Arc<StructureGraph>) -> Option<Response> {
        let entries = self.entries.lock().ok()?;
        let (_, _, value) = entries.iter().find(|(k, g, _)| k == key && Arc::ptr_eq(g, graph))?;
        Some(match value {
            Cached::Diagram(d) => Json(&**d).into_response(),
            Cached::Availability(a) => Json(&**a).into_response(),
        })
    }

    fn put(&self, key: String, graph: Arc<StructureGraph>, value: Cached) {
        let Ok(mut entries) = self.entries.lock() else { return };
        // Entries of an older graph are useless.
        entries.retain(|(_, g, _)| Arc::ptr_eq(g, &graph));
        entries.retain(|(k, _, _)| *k != key);
        entries.push_back((key, graph, value));
        while entries.len() > CACHE_SIZE {
            entries.pop_front();
        }
    }
}

type ApiResult = Result<Response, (StatusCode, String)>;

#[derive(Deserialize)]
pub struct UmlQuery {
    kind: UmlKind,
    /// `UmlScope` as JSON (`{"kind":"Folder","path":"src"}`).
    scope: String,
    /// `UmlOptions` as JSON; defaults when absent.
    #[serde(default)]
    opts: Option<String>,
}

#[derive(Deserialize)]
pub struct AvailabilityQuery {
    scope: String,
}

struct Inputs {
    graph: Arc<StructureGraph>,
    knowledge: Arc<ProjectKnowledge>,
    units: Vec<BuildUnit>,
    name: String,
}

fn inputs(state: &AppState) -> Result<Inputs, (StatusCode, String)> {
    let mut project = state.project.lock().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "project lock poisoned".to_owned()))?;
    let graph = project.structure();
    let knowledge = project.knowledge();
    let units = project.units();
    let name = project.root().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Inputs { graph, knowledge, units, name })
}

/// What the current recording adds to activity and sequence diagrams, with its event count.
fn runtime_of(state: &AppState, kind: UmlKind, opts: &UmlOptions) -> Option<(Runtime, u64)> {
    if !matches!(kind, UmlKind::Activity | UmlKind::Sequence) {
        return None;
    }
    let recorder = state.run.recorder()?;
    let recorder = recorder.lock().ok()?;
    let frame = recorder.snapshot();
    let mut runtime = Runtime { counts: frame.counts, calls: frame.calls, spans: Vec::new() };
    if kind == UmlKind::Sequence && opts.runtime {
        runtime.spans = recorder.calls(MAX_SPANS).spans;
        // The chosen call may be past the first spans of a long recording.
        let root = opts.focus.as_deref().and_then(|f| f.strip_prefix("span:")).and_then(|n| n.parse::<u32>().ok());
        if let Some(root) = root
            && !runtime.spans.iter().any(|s| s.id == root)
        {
            runtime.spans.extend(recorder.span_tree(root, MAX_SPANS).spans);
        }
    }
    Some((runtime, frame.events))
}

const MAX_SPANS: usize = 20_000;

fn bad(what: &str, e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, format!("invalid {what}: {e}"))
}

fn uml_error(e: UmlError) -> (StatusCode, String) {
    match e {
        UmlError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        UmlError::Unsupported(_) => (StatusCode::BAD_REQUEST, e.to_string()),
    }
}

pub async fn diagram(State(state): State<Arc<AppState>>, Query(q): Query<UmlQuery>) -> ApiResult {
    let scope: UmlScope = serde_json::from_str(&q.scope).map_err(|e| bad("scope", e))?;
    let opts: UmlOptions = match q.opts.as_deref().filter(|o| !o.is_empty()) {
        Some(o) => serde_json::from_str(o).map_err(|e| bad("options", e))?,
        None => UmlOptions::default(),
    };
    // A recorded run changes activity and sequence diagrams: its size is part of the key.
    let runtime = runtime_of(&state, q.kind, &opts);
    let events = runtime.as_ref().map(|(_, e)| *e).unwrap_or(0);
    let key = format!("diagram|{:?}|{}|{}|{events}", q.kind, serde_json::to_string(&scope).unwrap_or_default(), serde_json::to_string(&opts).unwrap_or_default());
    let inputs = inputs(&state)?;
    if let Some(hit) = state.uml.get(&key, &inputs.graph) {
        return Ok(hit);
    }
    let kind = q.kind;
    let graph = inputs.graph.clone();
    let st = state.clone();
    let built = tokio::task::spawn_blocking(move || {
        let runtime = runtime.map(|(r, _)| r);
        let input = UmlInput { graph: &inputs.graph, knowledge: &inputs.knowledge, units: &inputs.units, project: &inputs.name, flows: &st.project, runtime: runtime.as_ref() };
        nodex_uml::build(&input, kind, &scope, &opts)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(uml_error)?;
    let built = Arc::new(built);
    state.uml.put(key, graph, Cached::Diagram(built.clone()));
    Ok(Json(&*built).into_response())
}

pub async fn availability(State(state): State<Arc<AppState>>, Query(q): Query<AvailabilityQuery>) -> ApiResult {
    let scope: UmlScope = serde_json::from_str(&q.scope).map_err(|e| bad("scope", e))?;
    let key = format!("availability|{}", serde_json::to_string(&scope).unwrap_or_default());
    let inputs = inputs(&state)?;
    if let Some(hit) = state.uml.get(&key, &inputs.graph) {
        return Ok(hit);
    }
    let graph = inputs.graph.clone();
    let st = state.clone();
    let built = tokio::task::spawn_blocking(move || {
        let input = UmlInput { graph: &inputs.graph, knowledge: &inputs.knowledge, units: &inputs.units, project: &inputs.name, flows: &st.project, runtime: None };
        nodex_uml::availability(&input, &scope)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(uml_error)?;
    let built = Arc::new(built);
    state.uml.put(key, graph, Cached::Availability(built.clone()));
    Ok(Json(&*built).into_response())
}
