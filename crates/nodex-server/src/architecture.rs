//! Architectural diagrams and local declarations for the currently open project.

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use nodex_ir::StructureGraph;
use nodex_uml::architecture::{
    self, ArchitectureConfig, ArchitectureDiagram, ArchitectureRequest, ArchitectureView,
};
use nodex_uml::{UmlInput, UmlScope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::{AppState, ServerEvent};

type Error = (StatusCode, String);
type Result<T> = std::result::Result<T, Error>;

pub(crate) const CONFIG_PATH: &str = ".nodex/architecture.json";

#[derive(Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ArchitectureSettings {
    pub config: ArchitectureConfig,
    pub revision: String,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Default)]
pub struct ArchitectureCache {
    entries: Mutex<VecDeque<CacheEntry>>,
}

struct CacheEntry {
    graph: Arc<StructureGraph>,
    key: String,
    diagram: Arc<ArchitectureDiagram>,
}

impl ArchitectureCache {
    fn get(
        &self,
        graph: &Arc<StructureGraph>,
        key: &str,
    ) -> Result<Option<Arc<ArchitectureDiagram>>> {
        let entries = self.entries.lock().map_err(internal)?;
        Ok(entries
            .iter()
            .find(|e| Arc::ptr_eq(&e.graph, graph) && e.key == key)
            .map(|e| e.diagram.clone()))
    }

    fn put(
        &self,
        graph: Arc<StructureGraph>,
        key: String,
        diagram: Arc<ArchitectureDiagram>,
    ) -> Result<()> {
        let mut entries = self.entries.lock().map_err(internal)?;
        entries.retain(|e| Arc::ptr_eq(&e.graph, &graph) && e.key != key);
        entries.push_back(CacheEntry {
            graph,
            key,
            diagram,
        });
        if entries.len() > 16 {
            entries.pop_front();
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
pub struct ArchitectureQuery {
    view: ArchitectureView,
    scope: Option<String>,
    expanded: Option<String>,
    container: Option<String>,
}

pub async fn diagram(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ArchitectureQuery>,
) -> Result<Response> {
    let request = ArchitectureRequest {
        view: q.view,
        scope: q
            .scope
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(bad)?
            .unwrap_or(UmlScope::Project),
        expanded: q
            .expanded
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(bad)?
            .unwrap_or_default(),
        container: q.container.clone(),
    };
    let generation = state.generation.load(Ordering::Acquire);
    let (graph, knowledge) = crate::builds::graphs(&state).await.map_err(internal)?;
    tokio::task::spawn_blocking(move || {
        let (root, name, units) = {
            let project = state.project.lock().map_err(internal)?;
            if !project.is_open()
                || state.generation.load(Ordering::Acquire) != generation
                || !project
                    .built()
                    .is_some_and(|(g, _)| Arc::ptr_eq(&g, &graph))
            {
                return Err(conflict());
            }
            (
                project.root().to_owned(),
                project.summary().name,
                project.units(),
            )
        };
        let settings = read_settings(&root, generation)?;
        let key = format!(
            "{}|{:?}|{}",
            settings.revision,
            units,
            serde_json::to_string(&q).map_err(internal)?
        );
        let diagram = if let Some(hit) = state.architecture.get(&graph, &key)? {
            hit
        } else {
            let input = UmlInput {
                graph: &graph,
                knowledge: &knowledge,
                units: &units,
                project: &name,
                flows: &state.project,
                runtime: None,
            };
            let mut diagram = architecture::build(&input, &request, &settings.config)
                .map_err(|err| (StatusCode::NOT_FOUND, err.to_string()))?;
            diagram.warnings.extend(settings.warning);
            Arc::new(diagram)
        };
        let project = state.project.lock().map_err(internal)?;
        if state.generation.load(Ordering::Acquire) != generation
            || !project
                .built()
                .is_some_and(|(g, _)| Arc::ptr_eq(&g, &graph))
            || read_settings(&root, generation)?.revision != settings.revision
        {
            return Err(conflict());
        }
        state.architecture.put(graph, key, diagram.clone())?;
        Ok(Json(diagram.as_ref()).into_response())
    })
    .await
    .map_err(internal)?
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Result<Json<ArchitectureSettings>> {
    tokio::task::spawn_blocking(move || {
        let project = state.project.lock().map_err(internal)?;
        if !project.is_open() {
            return Err(conflict());
        }
        read_settings(project.root(), state.generation.load(Ordering::Acquire)).map(Json)
    })
    .await
    .map_err(internal)?
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(update): Json<ArchitectureSettings>,
) -> Result<Json<ArchitectureSettings>> {
    validate(&update.config).map_err(bad)?;
    tokio::task::spawn_blocking(move || {
        let project = state.project.lock().map_err(internal)?;
        if !project.is_open() {
            return Err(conflict());
        }
        let generation = state.generation.load(Ordering::Acquire);
        let previous = read_settings(project.root(), generation)?;
        if previous.revision != update.revision {
            return Err(conflict());
        }
        let path = project.root().join(CONFIG_PATH);
        let bytes = serde_json::to_vec_pretty(&update.config).map_err(internal)?;
        write_atomic(&path, &bytes).map_err(internal)?;
        let _ = state.events.send(ServerEvent::ArchitectureChanged);
        Ok(Json(ArchitectureSettings {
            config: update.config,
            revision: revision(project.root(), generation, Some(&bytes)),
            warning: None,
        }))
    })
    .await
    .map_err(internal)?
}

fn read_settings(root: &Path, generation: u64) -> Result<ArchitectureSettings> {
    let file = match fs::read(root.join(CONFIG_PATH)) {
        Ok(bytes) => Some(bytes),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            None
        }
        Err(err) => return Err(internal(err)),
    };
    let revision = revision(root, generation, file.as_deref());
    let mut settings = ArchitectureSettings {
        config: ArchitectureConfig::default(),
        revision,
        warning: None,
    };
    if let Some(bytes) = file {
        let parsed = serde_json::from_slice(&bytes)
            .map_err(|e| e.to_string())
            .and_then(|config| {
                validate(&config)?;
                Ok(config)
            });
        match parsed {
            Ok(config) => settings.config = config,
            Err(err) => {
                settings.warning = Some(format!(
                    "Architecture settings are invalid. Showing the inferred view. {err}"
                ))
            }
        }
    }
    Ok(settings)
}

fn revision(root: &Path, generation: u64, bytes: Option<&[u8]>) -> String {
    let mut hash = Sha256::new();
    hash.update(generation.to_le_bytes());
    hash.update(root.as_os_str().as_encoded_bytes());
    if let Some(bytes) = bytes {
        hash.update([0]);
        hash.update(bytes);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().expect("settings path has a parent");
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)?;
    Ok(())
}

fn validate(config: &ArchitectureConfig) -> std::result::Result<(), String> {
    let path_valid = |p: &str| {
        p.split('/').all(|part| {
            !part.trim().is_empty() && !matches!(part, "." | "..") && !part.contains(['\\', ':'])
        })
    };
    if config.layers.keys().any(|p| !path_valid(p))
        || config
            .libraries
            .iter()
            .any(|l| !l.application.starts_with("app:") || !path_valid(&l.path))
    {
        return Err("Use paths relative to the opened project.".into());
    }
    let mut roots = BTreeSet::new();
    for app in &config.applications {
        if (!app.root.is_empty() && !path_valid(&app.root))
            || app.name.trim().is_empty()
            || !roots.insert(&app.root)
        {
            return Err("Each application needs a name and a unique project-relative root.".into());
        }
    }
    for items in [&config.actors, &config.external_systems, &config.stores] {
        let mut ids = BTreeSet::new();
        for item in items {
            if item.id.is_empty()
                || !item
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
                || item.name.trim().is_empty()
                || !ids.insert(&item.id)
            {
                return Err("Each declared element needs a name and a unique ID containing letters, digits, hyphens or underscores.".into());
            }
        }
    }
    if config.relations.iter().any(|r| {
        r.from.is_empty() || r.to.is_empty() || r.from == r.to || r.label.trim().is_empty()
    }) {
        return Err("Each relation needs distinct endpoints and a label.".into());
    }
    Ok(())
}

fn bad(err: impl std::fmt::Display) -> Error {
    (StatusCode::BAD_REQUEST, err.to_string())
}

fn internal(err: impl std::fmt::Display) -> Error {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn conflict() -> Error {
    (
        StatusCode::CONFLICT,
        "The project or its architecture settings changed. Reload the view before retrying.".into(),
    )
}
