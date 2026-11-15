//! Optional refinement jobs and project-local approved versions.

mod context;
mod provider;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{Path as RoutePath, Query, State};
use axum::http::StatusCode;
use nodex_uml::refinement::{self, Inventory, Refined, Reviewed, Target};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::AppState;

type Error = (StatusCode, String);
type Result<T> = std::result::Result<T, Error>;
const DIRECTORY: &str = ".nodex/diagram-refinements";
const SCHEMA: u32 = 1;

struct Basis {
    root: PathBuf,
    key: String,
    digest: String,
    generation: u64,
    source_revision: u64,
    graph: Arc<nodex_ir::StructureGraph>,
    config_digest: String,
    documents: BTreeMap<String, Option<String>>,
    target: Target,
    inventory: Inventory,
}

struct Prepared {
    created: Instant,
    basis: Arc<Basis>,
    context: context::Context,
    digest: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub(crate) enum Progress {
    Running,
    Ready { suggestions: Vec<Reviewed> },
    Failed { message: String },
    Cancelled,
}

struct Job {
    basis: Arc<Basis>,
    cancel: watch::Sender<bool>,
    progress: Progress,
}

#[derive(Default)]
struct StateData {
    previews: BTreeMap<String, Prepared>,
    jobs: BTreeMap<String, Job>,
    watched_documents: BTreeMap<String, Option<String>>,
}

#[derive(Default)]
pub(crate) struct Manager(Mutex<StateData>, #[cfg(test)] Mutex<Option<PathBuf>>);

impl Manager {
    pub(crate) fn cancel_jobs(&self) {
        if let Ok(mut data) = self.0.lock() {
            data.previews.clear();
            data.jobs.clear();
        }
    }

    pub(crate) fn reset(&self) {
        if let Ok(mut data) = self.0.lock() {
            *data = StateData::default();
        }
    }
}

pub(crate) fn refresh_documents(state: &AppState, root: &Path, generation: u64) -> bool {
    if state.generation.load(Ordering::Acquire) != generation {
        return false;
    }
    let Ok(data) = state.refinements.0.lock() else {
        return false;
    };
    let tracked = data.watched_documents.clone();
    drop(data);

    // The watcher may report a symlink target or another hard link to a document.
    let changes: Vec<_> = tracked
        .into_iter()
        .filter_map(|(path, old)| {
            let revision = context::source(root, &path).map(hash);
            (old != revision).then_some((path, revision))
        })
        .collect();
    if changes.is_empty() {
        return false;
    }
    let Ok(mut data) = state.refinements.0.lock() else {
        return false;
    };
    if state.generation.load(Ordering::Acquire) != generation {
        return false;
    }
    data.watched_documents.extend(changes);
    true
}

fn configured_provider(_state: &AppState) -> Result<PathBuf> {
    #[cfg(test)]
    if let Some(path) = _state.refinements.1.lock().map_err(internal)?.clone() {
        return Ok(path);
    }
    let settings = crate::settings::load();
    if !settings.diagram_ai {
        return Err(bad("Enable diagram refinement in Settings first."));
    }
    provider::executable(settings.codex_path.as_deref()).map_err(bad)
}

fn hash(bytes: impl AsRef<[u8]>) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(internal)?;
    Ok(hex(&bytes))
}

fn bad(message: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, message.to_string())
}
fn internal(message: impl ToString) -> Error {
    (StatusCode::INTERNAL_SERVER_ERROR, message.to_string())
}
fn conflict() -> Error {
    (
        StatusCode::CONFLICT,
        "The project or saved revision changed. Reload and prepare a new preview.".into(),
    )
}
fn missing() -> Error {
    (
        StatusCode::NOT_FOUND,
        "The refinement session is no longer available.".into(),
    )
}

fn config_bytes(root: &Path) -> Result<Vec<u8>> {
    match std::fs::read(root.join(crate::architecture::CONFIG_PATH)) {
        Ok(bytes) => Ok(bytes),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(Vec::new())
        }
        Err(e) => Err(internal(e)),
    }
}

pub(crate) fn is_document(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            context::DOCUMENTS.iter().any(|document| {
                *document == name || (cfg!(windows) && document.eq_ignore_ascii_case(name))
            })
        })
        && context::path_allowed(&path.to_string_lossy().replace('\\', "/"))
}

async fn capture(state: &Arc<AppState>, mut target: Target) -> Result<Basis> {
    target.normalize();
    let generation = state.generation.load(Ordering::Acquire);
    let (graph, knowledge) = crate::builds::graphs(state).await.map_err(internal)?;
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let (root, name, units, source_revision) = {
            let project = state.project.lock().map_err(internal)?;
            if generation != state.generation.load(Ordering::Acquire)
                || !project
                    .built()
                    .is_some_and(|(current, _)| Arc::ptr_eq(&current, &graph))
            {
                return Err(conflict());
            }
            (
                project.root().to_path_buf(),
                project.name(),
                project.units(),
                project.source_revision(),
            )
        };
        let applies_config = matches!(target, Target::Architecture { .. });
        let bytes = if applies_config {
            config_bytes(&root)?
        } else {
            Vec::new()
        };
        let config = serde_json::from_slice(&bytes)
            .ok()
            .filter(|c| crate::architecture::validate(c).is_ok())
            .unwrap_or_default();
        let input = nodex_uml::UmlInput {
            graph: &graph,
            knowledge: &knowledge,
            units: &units,
            project: &name,
            flows: &state.project,
            runtime: None,
        };
        let mut inventory = refinement::inventory(&input, &target, &config).map_err(bad)?;
        let docs_folder = match target.scope() {
            nodex_uml::UmlScope::Project => Some("".into()),
            nodex_uml::UmlScope::Folder { path } => Some(path.clone()),
            _ => None,
        };
        let mut documents = BTreeMap::new();
        if let Some(dir) = docs_folder {
            for name in context::DOCUMENTS {
                let path = if dir.is_empty() {
                    name.into()
                } else {
                    format!("{dir}/{name}")
                };
                let revision = context::source(&root, &path).map(hash);
                if revision.is_some() {
                    inventory.evidence.push(refinement::Evidence {
                        id: format!("e{}", inventory.evidence.len()),
                        text: "Project documentation".into(),
                        site: None,
                        path: Some(path.clone()),
                    });
                }
                documents.insert(path, revision);
            }
        }
        let key = hash(serde_json::to_vec(&target).map_err(internal)?);
        let config_digest = hash(&bytes);
        let mut digest = Sha256::new();
        digest.update(SCHEMA.to_le_bytes());
        digest.update(key.as_bytes());
        digest.update(config_digest.as_bytes());
        digest.update(serde_json::to_vec(&inventory).map_err(internal)?);
        // Stable across process restarts, including when evidence text is unchanged
        // but a file supplying it has changed beneath the displayed spans.
        for path in inventory
            .evidence
            .iter()
            .filter_map(|e| e.path.as_deref())
            .collect::<BTreeSet<_>>()
        {
            if let Some(revision) = documents.get(path) {
                if let Some(revision) = revision {
                    digest.update(revision.as_bytes());
                }
            } else if let Some(text) = context::source(&root, path) {
                digest.update(hash(text).as_bytes());
            }
        }
        let basis = Basis {
            root,
            key,
            digest: hex(&digest.finalize()),
            generation,
            source_revision,
            graph,
            config_digest,
            documents,
            target,
            inventory,
        };
        check_basis(&state, &basis)?;
        if !basis.documents.is_empty() {
            let mut data = state.refinements.0.lock().map_err(internal)?;
            if state.generation.load(Ordering::Acquire) != basis.generation {
                return Err(conflict());
            }
            for (path, revision) in &basis.documents {
                data.watched_documents
                    .entry(path.clone())
                    .or_insert_with(|| revision.clone());
            }
        }
        Ok(basis)
    })
    .await
    .map_err(internal)?
}

fn check_basis(state: &AppState, basis: &Basis) -> Result<()> {
    if basis
        .documents
        .iter()
        .any(|(path, revision)| context::source(&basis.root, path).map(hash) != *revision)
    {
        return Err(conflict());
    }
    let project = state.project.lock().map_err(internal)?;
    if state.generation.load(Ordering::Acquire) != basis.generation
        || project.root() != basis.root
        || project.source_revision() != basis.source_revision
        || !project
            .built()
            .is_some_and(|(current, _)| Arc::ptr_eq(&current, &basis.graph))
        || (matches!(basis.target, Target::Architecture { .. })
            && hash(config_bytes(&basis.root)?) != basis.config_digest)
    {
        return Err(conflict());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContextRequest {
    target: Target,
    #[serde(default)]
    exclude: Vec<context::ExcludedExcerpt>,
    focus: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct Preview {
    id: String,
    digest: String,
    input: String,
    snippets: Vec<context::Snippet>,
    omitted: usize,
    covered_files: BTreeSet<String>,
    focus_nodes: Vec<(String, String)>,
}

fn excerpt_regions(
    state: &AppState,
    basis: &Basis,
    requested: Vec<context::ExcludedExcerpt>,
) -> Result<Vec<context::ExcerptRegion>> {
    let data = state.refinements.0.lock().map_err(internal)?;
    requested
        .into_iter()
        .map(|exclusion| match exclusion {
            context::ExcludedExcerpt::Region(region) => Ok(region),
            context::ExcludedExcerpt::EvidenceId(id) => {
                let paths: BTreeSet<_> = data
                    .previews
                    .values()
                    .filter(|p| {
                        p.basis.generation == basis.generation
                            && p.basis.key == basis.key
                            && p.created.elapsed().as_secs() < 900
                    })
                    .flat_map(|p| &p.context.snippets)
                    .filter(|s| s.id == id)
                    .map(|s| s.path.as_str())
                    .collect();
                if paths.len() != 1 {
                    return Err(conflict());
                }
                Ok(context::ExcerptRegion {
                    path: (*paths.first().unwrap()).to_owned(),
                    line: 1,
                    revision: None,
                })
            }
        })
        .collect()
}

pub(crate) async fn context(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ContextRequest>,
) -> Result<Json<Preview>> {
    configured_provider(&state)?;
    let basis = Arc::new(capture(&state, request.target).await?);
    if request
        .focus
        .as_ref()
        .is_some_and(|id| !basis.inventory.nodes.iter().any(|n| &n.id == id))
    {
        return Err(bad("The selected focus is outside this diagram."));
    }
    let excluded = excerpt_regions(&state, &basis, request.exclude)?;
    let snapshot = basis.clone();
    let context = tokio::task::spawn_blocking(move || {
        context::pack(
            &snapshot.inventory,
            &snapshot.root,
            &excluded,
            request.focus.as_deref(),
        )
    })
    .await
    .map_err(internal)?;
    if context.input.len() > context::BUDGET {
        return Err(bad("The context exceeded its maximum size."));
    }
    check_basis(&state, &basis)?;
    let id = id()?;
    let digest = hash(context.input.as_bytes());
    let covered_files = basis
        .inventory
        .evidence
        .iter()
        .filter(|e| context.evidence.contains(&e.id))
        .filter_map(|e| context::evidence_path(e).map(str::to_owned))
        .collect();
    let focus_nodes = basis
        .inventory
        .nodes
        .iter()
        .map(|n| (n.id.clone(), context::scrub(&n.label)))
        .collect();
    let preview = Preview {
        id: id.clone(),
        digest: digest.clone(),
        input: context.input.clone(),
        snippets: context.snippets.clone(),
        omitted: context.omitted,
        covered_files,
        focus_nodes,
    };
    let mut data = state.refinements.0.lock().map_err(internal)?;
    data.previews
        .retain(|_, p| p.created.elapsed().as_secs() < 900);
    if data.previews.len() >= 4
        && let Some(oldest) = data
            .previews
            .iter()
            .min_by_key(|(_, p)| p.created)
            .map(|(id, _)| id.clone())
    {
        data.previews.remove(&oldest);
    }
    data.previews.insert(
        id,
        Prepared {
            created: Instant::now(),
            basis,
            context,
            digest,
        },
    );
    Ok(Json(preview))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    preview: String,
    digest: String,
    confirmed: bool,
}

pub(crate) async fn start(
    State(state): State<Arc<AppState>>,
    Json(request): Json<Start>,
) -> Result<(StatusCode, Json<String>)> {
    if !request.confirmed {
        return Err(bad("Confirm the context before sending it to Codex."));
    }
    let binary = configured_provider(&state)?;
    let basis = {
        let data = state.refinements.0.lock().map_err(internal)?;
        let p = data.previews.get(&request.preview).ok_or_else(missing)?;
        if p.digest != request.digest || p.created.elapsed().as_secs() >= 900 {
            return Err(conflict());
        }
        p.basis.clone()
    };
    let fresh = capture(&state, basis.target.clone()).await?;
    if fresh.generation != basis.generation || fresh.digest != basis.digest {
        return Err(conflict());
    }
    let job_id = id()?;
    let lock = lock_generation(&basis.root)?;
    let (cancel, cancelled) = watch::channel(false);
    let prepared = {
        let mut data = state.refinements.0.lock().map_err(internal)?;
        if state.generation.load(Ordering::Acquire) != basis.generation
            || data
                .jobs
                .values()
                .any(|job| matches!(job.progress, Progress::Running))
        {
            return Err(conflict());
        }
        let prepared = data.previews.remove(&request.preview).ok_or_else(missing)?;
        if data.jobs.len() >= 8
            && let Some(oldest) = data.jobs.keys().next().cloned()
        {
            data.jobs.remove(&oldest);
        }
        data.jobs.insert(
            job_id.clone(),
            Job {
                basis: basis.clone(),
                cancel,
                progress: Progress::Running,
            },
        );
        prepared
    };
    let key = job_id.clone();
    tokio::spawn(async move {
        let _project_lock = lock;
        let context = prepared.context;
        let result = provider::generate(&binary, &context.input, cancelled).await;
        let new = match result {
            Ok(proposal) => Progress::Ready {
                suggestions: refinement::review(
                    &basis.inventory,
                    context.resolve(proposal),
                    &context.evidence,
                    &context.nodes,
                    &context.edges,
                ),
            },
            Err(message) => Progress::Failed { message },
        };
        if let Ok(mut data) = state.refinements.0.lock()
            && let Some(job) = data.jobs.get_mut(&key)
            && matches!(job.progress, Progress::Running)
        {
            job.progress = new;
        }
    });
    Ok((StatusCode::ACCEPTED, Json(job_id)))
}

fn lock_generation(root: &Path) -> Result<std::fs::File> {
    let dir = std::env::temp_dir().join("nodex-refinement-job-locks");
    std::fs::create_dir_all(&dir).map_err(internal)?;
    let key = hash(root.as_os_str().as_encoded_bytes());
    let path = dir.join(format!("{key}.lock"));
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
        .open(path).map_err(internal)?;
    lock.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => (StatusCode::CONFLICT, "This project already has a refinement running.".into()),
        std::fs::TryLockError::Error(error) => internal(error),
    })?;
    Ok(lock)
}

pub(crate) async fn status(
    State(state): State<Arc<AppState>>,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<Progress>> {
    let data = state.refinements.0.lock().map_err(internal)?;
    Ok(Json(
        data.jobs.get(&id).ok_or_else(missing)?.progress.clone(),
    ))
}

pub(crate) async fn cancel(
    State(state): State<Arc<AppState>>,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<Progress>> {
    let mut data = state.refinements.0.lock().map_err(internal)?;
    let job = data.jobs.get_mut(&id).ok_or_else(missing)?;
    if matches!(job.progress, Progress::Running) {
        let _ = job.cancel.send(true);
        job.progress = Progress::Cancelled;
    }
    Ok(Json(job.progress.clone()))
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Version {
    id: String,
    created: u64,
    basis: String,
    diagram: Refined,
}

impl Version {
    fn contains_legacy_association(&self) -> bool {
        self.diagram
            .suggestions
            .iter()
            .any(|r| r.suggestion.kind == "association")
    }

    fn restore_unresolved_routes(&mut self) {
        if !self.contains_legacy_association() {
            return;
        }
        let mut associations = BTreeSet::new();
        self.diagram.suggestions.retain(|r| {
            if r.suggestion.kind == "association" {
                associations.insert(r.suggestion.target.clone());
                false
            } else {
                true
            }
        });
        let diagram = &mut self.diagram;
        let labels: BTreeMap<_, _> = diagram
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.label.as_str()))
            .collect();
        diagram.edges.retain(|edge| {
            if !associations.contains(&edge.id) {
                return true;
            }
            let source = diagram
                .evidence
                .iter()
                .find(|e| edge.evidence.first() == Some(&e.id));
            let id = format!("legacy-association:{}", edge.id);
            diagram.evidence.push(refinement::Evidence {
                id: id.clone(),
                text: format!(
                    "Ambiguous HTTP destination {} → {} ({})",
                    labels
                        .get(edge.from.as_str())
                        .copied()
                        .unwrap_or(&edge.from),
                    labels.get(edge.to.as_str()).copied().unwrap_or(&edge.to),
                    edge.label,
                ),
                site: source.and_then(|e| e.site.clone()),
                path: source.and_then(|e| e.path.clone()),
            });
            diagram.pending.push(id);
            false
        });
    }
}

#[derive(Serialize, Deserialize)]
struct Stored {
    schema: u32,
    active: Option<String>,
    versions: Vec<Version>,
}

#[derive(Serialize)]
pub(crate) struct History {
    revision: String,
    current: Option<String>,
    active: Option<String>,
    versions: Vec<Version>,
}

fn folder(root: &Path, create: bool) -> Result<PathBuf> {
    let mut expected = root.canonicalize().map_err(internal)?;
    let mut dir = root.to_owned();
    for component in Path::new(DIRECTORY).components() {
        dir.push(component.as_os_str());
        expected.push(component.as_os_str());
        if create
            && !dir.exists()
            && let Err(error) = std::fs::create_dir(&dir)
            && error.kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(internal(error));
        }
        if dir.exists() && dir.canonicalize().map_err(internal)? != expected {
            return Err(bad("Refinement storage must stay inside the project."));
        }
    }
    Ok(dir)
}

fn index(path: &Path) -> Result<(Stored, String)> {
    if path.is_symlink() {
        return Err(bad("The refinement index must be a local file."));
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let stored: Stored = serde_json::from_slice(&bytes)
                .map_err(|_| bad("The refinement history is unreadable."))?;
            if stored.schema != SCHEMA {
                return Err(bad(
                    "This refinement history was created by another schema version.",
                ));
            }
            Ok((stored, hash(bytes)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((
            Stored {
                schema: SCHEMA,
                active: None,
                versions: Vec::new(),
            },
            hash([]),
        )),
        Err(e) => Err(internal(e)),
    }
}

impl History {
    fn from(mut stored: Stored, revision: String, current: Option<String>) -> Self {
        for version in &mut stored.versions {
            version.restore_unresolved_routes();
        }
        Self {
            revision,
            current,
            active: stored.active,
            versions: stored.versions,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct HistoryQuery {
    target: String,
}

pub(crate) async fn history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<History>> {
    let mut target: Target = serde_json::from_str(&query.target).map_err(bad)?;
    target.normalize();
    let key = hash(serde_json::to_vec(&target).map_err(internal)?);
    let (root, generation) = {
        let p = state.project.lock().map_err(internal)?;
        (
            p.root().to_path_buf(),
            state.generation.load(Ordering::Acquire),
        )
    };
    let (store, revision) = index(&folder(&root, false)?.join(format!("{key}.json")))?;
    let current = if store.versions.is_empty() {
        None
    } else {
        capture(&state, target)
            .await
            .ok()
            .filter(|b| b.generation == generation)
            .map(|b| b.digest)
    };
    if state.generation.load(Ordering::Acquire) != generation {
        return Err(conflict());
    }
    Ok(Json(History::from(store, revision, current)))
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Save {
    Approve {
        job: String,
        selected: BTreeSet<usize>,
        revision: String,
    },
    Activate {
        target: Target,
        version: String,
        revision: String,
    },
}

pub(crate) async fn save(
    State(state): State<Arc<AppState>>,
    Json(request): Json<Save>,
) -> Result<Json<History>> {
    let (basis, version, revision, new) = match request {
        Save::Approve {
            job,
            selected,
            revision,
        } => {
            let (original, suggestions) = {
                let data = state.refinements.0.lock().map_err(internal)?;
                let job = data.jobs.get(&job).ok_or_else(missing)?;
                let Progress::Ready { suggestions } = &job.progress else {
                    return Err(conflict());
                };
                (job.basis.clone(), suggestions.clone())
            };
            if selected.is_empty() {
                return Err(bad("Select at least one suggestion to save."));
            }
            let current = capture(&state, original.target.clone()).await?;
            if current.generation != original.generation || current.digest != original.digest {
                return Err(conflict());
            }
            let diagram =
                refinement::apply(&current.inventory, &suggestions, &selected).map_err(bad)?;
            let version = id()?;
            let new = Version {
                id: version.clone(),
                created: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                basis: current.digest.clone(),
                diagram,
            };
            (current, version, revision, Some(new))
        }
        Save::Activate {
            target,
            version,
            revision,
        } => (capture(&state, target).await?, version, revision, None),
    };
    let history = tokio::task::spawn_blocking(move || {
        let dir = folder(&basis.root, true)?;
        let lock_path = dir.join(format!("{}.lock", basis.key));
        if lock_path.is_symlink() {
            return Err(bad("Invalid refinement lock."));
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(internal)?;
        lock.lock().map_err(internal)?;
        let path = dir.join(format!("{}.json", basis.key));
        let (mut store, existing) = index(&path)?;
        if existing != revision {
            return Err(conflict());
        }
        check_basis(&state, &basis)?;
        if let Some(new) = new {
            store.versions.push(new);
        }
        let selected = store
            .versions
            .iter()
            .find(|v| v.id == version)
            .ok_or_else(missing)?;
        if selected.basis != basis.digest || selected.contains_legacy_association() {
            return Err((
                StatusCode::CONFLICT,
                "This historical version is available for viewing only.".into(),
            ));
        }
        store.active = Some(version);
        let bytes = serde_json::to_vec_pretty(&store).map_err(internal)?;
        crate::architecture::write_atomic(&path, &bytes).map_err(internal)?;
        Ok(History::from(store, hash(bytes), Some(basis.digest)))
    })
    .await
    .map_err(internal)??;
    Ok(Json(history))
}
