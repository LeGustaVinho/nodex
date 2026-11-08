//! Runs the project instrumented and streams its execution to the browsers.
//!
//! 1. The language's `TraceInstrumenter` builds and instruments into `<project>/.nodex/run`.
//! 2. A loopback TCP listener with a fresh token receives the program's trace events.
//! 3. The program runs with `NODEX_TRACE`/`NODEX_TRACE_TOKEN`; its output is forwarded.
//!    Runtimes that cannot flush on an abrupt exit (Node.js `process.exit`) write the rest to
//!    `NODEX_TRACE_TAIL/<pid>.bin`, replayed after the program ends.
//! 4. A `Recorder` aggregates events; changes go out as `trace-frame` events ~20 times a second.

use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nodex_trace::{Decoder, Recorder, Resolver, TraceFrame, TraceMap};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use ts_rs::TS;

use crate::{AppState, ServerEvent, new_token};

const MAX_OUTPUT_LINES: usize = 2000;
const FRAME_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum RunState {
    Idle,
    Preparing,
    Running,
    Exited,
    Failed,
}

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct RunStatus {
    pub state: RunState,
    /// What runs, or why it failed.
    pub detail: String,
    pub exit_code: Option<i32>,
    /// The app's address, for dev servers (browser apps).
    pub url: Option<String>,
}

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct OutputLine {
    /// `nodex` (progress), `build`, `out` (stdout) or `err` (stderr).
    pub stream: String,
    pub text: String,
}

/// Everything a client needs after (re)connecting.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct RunSnapshot {
    pub status: RunStatus,
    pub output: Vec<OutputLine>,
    pub trace: Option<TraceFrame>,
}

pub struct RunManager {
    inner: Mutex<Inner>,
}

struct Inner {
    status: RunStatus,
    output: VecDeque<OutputLine>,
    recorder: Option<Arc<Mutex<Recorder>>>,
    kill: Option<oneshot::Sender<()>>,
    /// Token of the current run's trace intake (also accepted on the `/trace` WebSocket).
    trace_token: Option<String>,
    /// A dev-server run whose copy follows the project's files.
    live: Option<LiveRun>,
    /// Id of the current run. A run task only writes the state while its id is current: after a
    /// reset (another project) or a newer run, an old task still tearing down changes nothing.
    run_id: u64,
}

impl Default for RunManager {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: RunStatus { state: RunState::Idle, detail: String::new(), exit_code: None, url: None },
                output: VecDeque::new(),
                recorder: None,
                kill: None,
                trace_token: None,
                live: None,
                run_id: 0,
            }),
        }
    }
}

impl RunManager {
    pub fn snapshot(&self) -> RunSnapshot {
        let inner = self.inner.lock().expect("run lock");
        RunSnapshot {
            status: inner.status.clone(),
            output: inner.output.iter().cloned().collect(),
            trace: inner.recorder.as_ref().and_then(|r| r.lock().ok().map(|r| r.snapshot())),
        }
    }

    pub fn recorder(&self) -> Option<Arc<Mutex<Recorder>>> {
        self.inner.lock().ok().and_then(|i| i.recorder.clone())
    }

    pub fn is_busy(&self) -> bool {
        self.inner
            .lock()
            .map(|i| matches!(i.status.state, RunState::Preparing | RunState::Running))
            .unwrap_or(true)
    }

    /// The current run's recorder, when `token` is its trace token (the `/trace` WebSocket).
    pub fn recorder_for(&self, token: &str) -> Option<Arc<Mutex<Recorder>>> {
        let inner = self.inner.lock().ok()?;
        let expected = inner.trace_token.as_deref()?;
        let same = expected.len() == token.len() && expected.bytes().zip(token.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0;
        if same { inner.recorder.clone() } else { None }
    }

    /// Files changed while a browser app runs: instrument them again into its copy (the dev
    /// server reloads them) and teach the recorder their new methods.
    pub fn files_changed(&self, state: &AppState, paths: &[std::path::PathBuf]) {
        let (live, recorder) = match self.inner.lock() {
            Ok(i) => match (&i.live, &i.recorder) {
                (Some(l), Some(r)) => (l.clone(), r.clone()),
                _ => return,
            },
            Err(_) => return,
        };
        let mut added = Vec::new();
        for path in paths {
            let first = recorder.lock().map(|r| r.max_method_id() + 1).unwrap_or(1) + added.len() as i32;
            match live.instrumenter.reinstrument(&live.root, &live.out_dir, path, first) {
                Some(Ok(json)) => {
                    if let Ok(methods) = serde_json::from_str::<Vec<nodex_trace::map::MethodMap>>(&json) {
                        added.extend(methods);
                    }
                }
                Some(Err(e)) => tracing::warn!("re-instrumenting {} failed: {e}", path.display()),
                None => {}
            }
        }
        let resolver = match state.project.lock() {
            Ok(p) => Resolver::new(p.flows()),
            Err(_) => return,
        };
        if let Ok(mut r) = recorder.lock() {
            r.extend(added, resolver);
        }
    }

    /// Forgets the last run (another project was opened).
    pub fn reset(&self) {
        if let Ok(mut i) = self.inner.lock() {
            if let Some(k) = i.kill.take() {
                let _ = k.send(());
            }
            i.run_id += 1;
            i.output.clear();
            i.recorder = None;
            i.live = None;
            i.trace_token = None;
            i.status = RunStatus { state: RunState::Idle, detail: String::new(), exit_code: None, url: None };
        }
    }

    pub fn stop(&self) -> bool {
        self.inner.lock().ok().and_then(|mut i| i.kill.take()).map(|k| k.send(()).is_ok()).unwrap_or(false)
    }
}

/// Sets the status of run `id`; a run that is no longer current changes nothing.
fn set_status(state: &AppState, id: u64, status: RunStatus) {
    let Ok(mut inner) = state.run.inner.lock() else { return };
    if inner.run_id != id {
        return;
    }
    inner.status = status.clone();
    drop(inner);
    let _ = state.events.send(ServerEvent::RunStatus(status));
}

/// Runs `f` on the run state while run `id` is current; `None` once it is not.
fn with_current<T>(state: &AppState, id: u64, f: impl FnOnce(&mut Inner) -> T) -> Option<T> {
    let mut inner = state.run.inner.lock().ok()?;
    (inner.run_id == id).then(|| f(&mut inner))
}

pub(crate) fn push_output(state: &AppState, lines: Vec<OutputLine>) {
    if lines.is_empty() {
        return;
    }
    if let Ok(mut inner) = state.run.inner.lock() {
        for l in &lines {
            if inner.output.len() == MAX_OUTPUT_LINES {
                inner.output.pop_front();
            }
            inner.output.push_back(l.clone());
        }
    }
    let _ = state.events.send(ServerEvent::RunOutput { lines });
}

/// Forwards output lines in batches so a chatty program does not flood the socket.
fn output_forwarder(state: Arc<AppState>) -> mpsc::UnboundedSender<OutputLine> {
    let (tx, mut rx) = mpsc::unbounded_channel::<OutputLine>();
    tokio::spawn(async move {
        let mut batch = Vec::new();
        loop {
            tokio::select! {
                line = rx.recv() => match line {
                    Some(l) => {
                        batch.push(l);
                        if batch.len() >= 200 {
                            push_output(&state, std::mem::take(&mut batch));
                        }
                    }
                    None => break,
                },
                _ = tokio::time::sleep(Duration::from_millis(100)), if !batch.is_empty() => {
                    push_output(&state, std::mem::take(&mut batch));
                }
            }
        }
        push_output(&state, batch);
    });
    tx
}

pub(crate) fn line(stream: &str, text: impl Into<String>) -> OutputLine {
    OutputLine { stream: stream.into(), text: text.into() }
}

/// Starts a run. Returns an error if one is already in progress or nothing can run.
pub fn start(state: &Arc<AppState>) -> Result<(), String> {
    if state.run.is_busy() || state.debug.is_busy() {
        return Err("a run or a debug session is already in progress".into());
    }
    let (root, instrumenter) = {
        let project = state.project.lock().map_err(|_| "project lock poisoned")?;
        let instrumenter = project
            .active_frontends()
            .into_iter()
            .find_map(|f| f.instrumenter(project.root()))
            .ok_or("no language in this project supports live traces yet")?;
        (project.root().to_path_buf(), instrumenter)
    };
    let id = {
        let mut inner = state.run.inner.lock().map_err(|_| "run lock poisoned")?;
        inner.run_id += 1;
        inner.output.clear();
        inner.recorder = None;
        inner.run_id
    };
    set_status(state, id, RunStatus { state: RunState::Preparing, detail: "building and instrumenting…".into(), exit_code: None, url: None });
    let state = state.clone();
    tokio::spawn(async move {
        if let Err(reason) = run(&state, id, root, instrumenter).await {
            // A failed run leaves nothing behind: no file following, no trace token, no kill.
            let current = with_current(&state, id, |inner| {
                inner.live = None;
                inner.trace_token = None;
                inner.kill = None;
            });
            if current.is_some() {
                push_output(&state, vec![line("nodex", format!("✗ {reason}"))]);
                set_status(&state, id, RunStatus { state: RunState::Failed, detail: reason, exit_code: None, url: None });
            }
        }
    });
    Ok(())
}

/// What a running dev server needs to follow the project's files.
#[derive(Clone)]
struct LiveRun {
    instrumenter: Arc<dyn nodex_lang_api::TraceInstrumenter>,
    root: std::path::PathBuf,
    out_dir: std::path::PathBuf,
}

async fn run(
    state: &Arc<AppState>,
    id: u64,
    root: std::path::PathBuf,
    instrumenter: Box<dyn nodex_lang_api::TraceInstrumenter>,
) -> Result<(), String> {
    let instrumenter: Arc<dyn nodex_lang_api::TraceInstrumenter> = Arc::from(instrumenter);
    let out = output_forwarder(state.clone());

    // 1. Trace intake first: browser apps bake its address and token into the page.
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let endpoint = listener.local_addr().map_err(|e| e.to_string())?;
    let trace_token = new_token();
    let env = nodex_lang_api::RunEnv {
        tcp: endpoint.to_string(),
        ws_url: format!("ws://127.0.0.1:{}/trace?run={trace_token}", state.port),
        token: trace_token.clone(),
    };
    with_current(state, id, |inner| inner.trace_token = Some(trace_token.clone()));

    // 2. Build and instrument (blocking).
    let log_tx = out.clone();
    let out_dir = root.join(".nodex").join("run");
    let tail_dir = out_dir.join("tail");
    let _ = std::fs::remove_dir_all(&tail_dir);
    let prep_root = root.clone();
    let prep_env = env.clone();
    let prep_out = out_dir.clone();
    let prep_instrumenter = instrumenter.clone();
    let command = tokio::task::spawn_blocking(move || {
        prep_instrumenter.prepare(&prep_root, &prep_out, &prep_env, &mut |l| {
            let _ = log_tx.send(line("build", l));
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    // 2. Recorder over the current IR.
    let map_text = tokio::fs::read_to_string(&command.trace_map).await.map_err(|e| format!("cannot read trace map: {e}"))?;
    let map = TraceMap::parse(&map_text)?;
    let resolver = {
        let project = state.project.lock().map_err(|_| "project lock poisoned")?;
        Resolver::new(project.flows())
    };
    let recorder = Arc::new(Mutex::new(Recorder::new(&map, resolver)));
    // Another project was opened (or the run stopped) while building: do not launch.
    if with_current(state, id, |inner| inner.recorder = Some(recorder.clone())).is_none() {
        return Ok(());
    }

    // 3. Trace intake over TCP (programs); the `/trace` WebSocket serves browser pages.
    if command.kind == nodex_lang_api::RunKind::Attached {
        return attached(state, id, root, out_dir, instrumenter, command, listener, trace_token, recorder, out).await;
    }
    let intake = tokio::spawn(accept_traces(listener, trace_token.clone(), recorder.clone(), out.clone(), None));

    // 4. Launch.
    let mut cmd = tokio::process::Command::from(nodex_lang_api::process::command(&command.program));
    cmd.args(&command.args)
        .current_dir(&command.cwd)
        .env("NODEX_TRACE", endpoint.to_string())
        .env("NODEX_TRACE_TOKEN", &trace_token)
        .env("NODEX_TRACE_TAIL", &tail_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in &command.env {
        cmd.env(k, v);
    }
    let dev_server = command.kind == nodex_lang_api::RunKind::DevServer;
    if dev_server {
        // Plain output (the URL is read from it), and no browser opened by the tool itself.
        cmd.env("NO_COLOR", "1").env("FORCE_COLOR", "0").env("BROWSER", "none");
    }
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd.spawn().map_err(|e| format!("cannot start {}: {e}", command.program))?;
    // If this task ends without reaching the normal exit below (Nodex closes, a panic), the
    // program and everything it started are killed: a dev server must not outlive its run.
    let mut tree_guard = TreeGuard(child.id());
    let (kill_tx, kill_rx) = oneshot::channel();
    let live = dev_server.then(|| LiveRun { instrumenter: instrumenter.clone(), root: root.clone(), out_dir: out_dir.clone() });
    if with_current(state, id, |inner| {
        inner.kill = Some(kill_tx);
        inner.live = live;
    })
    .is_none()
    {
        // Superseded while starting: the guard ends the program.
        return Ok(());
    }
    let _ = out.send(line("nodex", format!("▶ {}", command.description)));
    set_status(state, id, RunStatus { state: RunState::Running, detail: command.description.clone(), exit_code: None, url: None });
    let found_url = Arc::new(std::sync::atomic::AtomicBool::new(false));
    for (stream, reader) in [
        ("out", child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)),
        ("err", child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)),
    ] {
        if let Some(reader) = reader {
            let tx = out.clone();
            let state = state.clone();
            let found_url = found_url.clone();
            let description = command.description.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(reader).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let l = strip_ansi(&l);
                    if dev_server
                        && !found_url.load(std::sync::atomic::Ordering::Relaxed)
                        && let Some(url) = app_url(&l)
                    {
                        found_url.store(true, std::sync::atomic::Ordering::Relaxed);
                        let _ = tx.send(line("nodex", format!("● the app is at {url}")));
                        set_status(&state, id, RunStatus { state: RunState::Running, detail: description.clone(), exit_code: None, url: Some(url) });
                    }
                    let _ = tx.send(line(stream, l));
                }
            });
        }
    }
    // 5. Stream frames until the program exits.
    let ticker_state = state.clone();
    let ticker_recorder = recorder.clone();
    let (stop_ticker, mut ticker_done) = oneshot::channel::<()>();
    let ticker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(FRAME_INTERVAL);
        loop {
            tokio::select! {
                _ = interval.tick() => send_frame(&ticker_state, &ticker_recorder),
                _ = &mut ticker_done => break,
            }
        }
    });
    let mut stopped = false;
    let status = tokio::select! {
        status = child.wait() => status.map_err(|e| e.to_string())?,
        _ = kill_rx => {
            kill_tree(&mut child).await;
            stopped = true;
            child.wait().await.map_err(|e| e.to_string())?
        }
    };
    tree_guard.0 = None;
    // Let the last trace batches arrive before the final frame.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = stop_ticker.send(());
    let _ = ticker.await;
    intake.abort();
    replay_tails(&tail_dir, &recorder, &out);
    send_frame(state, &recorder);
    // A program stopped from Nodex did not fail: its exit code is the kill's, not its own.
    let code = if stopped { Some(0) } else { status.code() };
    let message = if stopped { "■ stopped".to_owned() } else { format!("■ exited with code {}", code.map_or("?".into(), |c| c.to_string())) };
    let _ = out.send(line("nodex", message));
    with_current(state, id, |inner| {
        inner.kill = None;
        inner.trace_token = None;
        inner.live = None;
    });
    set_status(state, id, RunStatus { state: RunState::Exited, detail: command.description, exit_code: code, url: None });
    Ok(())
}

/// Called when a program connects to the trace intake (before its events are read).
type OnConnect = Arc<dyn Fn() + Send + Sync>;

/// A run with nothing to start: the program (the Unity Editor in Play Mode) connects by itself,
/// any number of times (each Play Mode, each domain reload), until Stop.
#[allow(clippy::too_many_arguments)]
async fn attached(
    state: &Arc<AppState>,
    id: u64,
    root: std::path::PathBuf,
    out_dir: std::path::PathBuf,
    instrumenter: Arc<dyn nodex_lang_api::TraceInstrumenter>,
    command: nodex_lang_api::RunCommand,
    listener: TcpListener,
    trace_token: String,
    recorder: Arc<Mutex<Recorder>>,
    out: mpsc::UnboundedSender<OutputLine>,
) -> Result<(), String> {
    let (kill_tx, kill_rx) = oneshot::channel();
    if with_current(state, id, |inner| inner.kill = Some(kill_tx)).is_none() {
        instrumenter.finish(&root);
        return Ok(());
    }
    // Each connection follows a compilation that may have changed the trace maps: re-read them
    // before its events are recorded.
    let (connected_tx, mut connected_rx) = mpsc::unbounded_channel::<()>();
    let on_connect: OnConnect = {
        let state = state.clone();
        let recorder = recorder.clone();
        let instrumenter = instrumenter.clone();
        let root = root.clone();
        let out_dir = out_dir.clone();
        Arc::new(move || {
            if let Some(Ok(json)) = instrumenter.refresh(&root, &out_dir)
                && let Ok(methods) = serde_json::from_str::<Vec<nodex_trace::map::MethodMap>>(&json)
                && let Ok(project) = state.project.lock()
            {
                let resolver = Resolver::new(project.flows());
                drop(project);
                if let Ok(mut r) = recorder.lock() {
                    r.extend(methods, resolver);
                }
            }
            let _ = connected_tx.send(());
        })
    };
    let intake = tokio::spawn(accept_traces(listener, trace_token, recorder.clone(), out.clone(), Some(on_connect)));
    let _ = out.send(line("nodex", format!("◌ waiting for {}", command.description)));
    set_status(state, id, RunStatus { state: RunState::Running, detail: format!("waiting for {}…", command.description), exit_code: None, url: None });
    let ticker_state = state.clone();
    let ticker_recorder = recorder.clone();
    let (stop_ticker, mut ticker_done) = oneshot::channel::<()>();
    let ticker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(FRAME_INTERVAL);
        loop {
            tokio::select! {
                _ = interval.tick() => send_frame(&ticker_state, &ticker_recorder),
                _ = &mut ticker_done => break,
            }
        }
    });
    let mut kill_rx = kill_rx;
    let mut first = true;
    loop {
        tokio::select! {
            _ = &mut kill_rx => break,
            Some(()) = connected_rx.recv() => {
                if first {
                    first = false;
                    let _ = out.send(line("nodex", format!("▶ {}", command.description)));
                    set_status(state, id, RunStatus { state: RunState::Running, detail: command.description.clone(), exit_code: None, url: None });
                }
            }
        }
    }
    let finish_root = root.clone();
    let _ = tokio::task::spawn_blocking(move || instrumenter.finish(&finish_root)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = stop_ticker.send(());
    let _ = ticker.await;
    intake.abort();
    send_frame(state, &recorder);
    let _ = out.send(line("nodex", "■ stopped"));
    with_current(state, id, |inner| {
        inner.kill = None;
        inner.trace_token = None;
    });
    set_status(state, id, RunStatus { state: RunState::Exited, detail: command.description, exit_code: Some(0), url: None });
    Ok(())
}

/// Kills a process tree when dropped, unless disarmed (its pid set to `None`).
struct TreeGuard(Option<u32>);

impl Drop for TreeGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0.take() {
            kill_tree_now(pid);
        }
    }
}

/// Synchronous tree kill, for drops.
fn kill_tree_now(pid: u32) {
    #[cfg(windows)]
    let _ = nodex_lang_api::process::command("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    #[cfg(unix)]
    let _ = nodex_lang_api::process::command("kill").args(["-TERM", &format!("-{pid}")]).status();
}

/// Ends the process and everything it started (a dev server runs `npm` → `node` → workers).
async fn kill_tree(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        #[cfg(windows)]
        {
            let _ = tokio::process::Command::from(nodex_lang_api::process::command("taskkill"))
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        #[cfg(unix)]
        {
            let _ = tokio::process::Command::from(nodex_lang_api::process::command("kill")).args(["-TERM", &format!("-{pid}")]).status().await;
        }
    }
    let _ = child.kill().await;
}

/// Removes terminal color codes (`ESC [ … m`).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// The local address a dev server prints (`➜  Local:   http://localhost:5173/`).
fn app_url(line: &str) -> Option<String> {
    for prefix in ["http://localhost:", "http://127.0.0.1:", "https://localhost:", "http://[::1]:"] {
        if let Some(i) = line.find(prefix) {
            let url: String = line[i..].chars().take_while(|c| !c.is_whitespace() && *c != ',' && *c != ')').collect();
            if url.len() > prefix.len() {
                return Some(url);
            }
        }
    }
    None
}

/// Feeds the frames a program wrote to its tail file when it ended abruptly.
fn replay_tails(dir: &std::path::Path, recorder: &Arc<Mutex<Recorder>>, out: &mpsc::UnboundedSender<OutputLine>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(bytes) = std::fs::read(entry.path()) else { continue };
        let mut decoder = Decoder::default();
        match decoder.feed(&bytes) {
            Ok(records) => {
                if let Ok(mut r) = recorder.lock() {
                    let replayed = r.replay_tail(&records);
                    if replayed > 0 {
                        let _ = out.send(line("nodex", format!("recovered {replayed} trace events written at exit")));
                    }
                }
            }
            Err(e) => {
                let _ = out.send(line("nodex", format!("trace tail corrupt: {}", e.0)));
            }
        }
    }
}

fn send_frame(state: &AppState, recorder: &Arc<Mutex<Recorder>>) {
    let frame = match recorder.lock() {
        Ok(mut r) if r.has_changes() => r.take_frame(),
        _ => return,
    };
    let _ = state.events.send(ServerEvent::TraceFrame(Box::new(frame)));
}

/// Accepts trace connections (a program may spawn traced children) and feeds the recorder.
async fn accept_traces(
    listener: TcpListener,
    token: String,
    recorder: Arc<Mutex<Recorder>>,
    out: mpsc::UnboundedSender<OutputLine>,
    on_connect: Option<OnConnect>,
) {
    while let Ok((socket, _)) = listener.accept().await {
        let token = token.clone();
        let recorder = recorder.clone();
        let out = out.clone();
        let on_connect = on_connect.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(socket);
            let mut hello = String::new();
            let ok = tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut hello)).await;
            if !matches!(ok, Ok(Ok(_))) || hello.trim_end() != format!("NODEX1 {token}") {
                return; // not our program
            }
            if let Some(hook) = on_connect {
                let _ = tokio::task::spawn_blocking(move || hook()).await;
            }
            let mut decoder = Decoder::default();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = match reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                match decoder.feed(&buf[..n]) {
                    Ok(records) => {
                        if let Ok(mut r) = recorder.lock() {
                            for rec in &records {
                                r.record(rec);
                            }
                        }
                    }
                    Err(e) => {
                        let _ = out.send(line("nodex", format!("trace stream corrupt: {}", e.0)));
                        break;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_server_addresses_are_found() {
        assert_eq!(app_url("  ➜  Local:   http://localhost:5173/"), Some("http://localhost:5173/".into()));
        assert_eq!(app_url("** Angular Live Development Server is listening on localhost:4200, open your browser on http://localhost:4200/ **"), Some("http://localhost:4200/".into()));
        assert_eq!(app_url("ready in 300 ms"), None);
        assert_eq!(strip_ansi("\u{1b}[32m➜\u{1b}[39m  Local"), "➜  Local");
    }
}
