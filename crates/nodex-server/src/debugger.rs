//! Step-through debugging through a Debug Adapter Protocol adapter.
//!
//! One session at a time, never while a Run is going. A session:
//! 1. builds the project for debugging (the language's [`DebugLauncher`]);
//! 2. starts the adapter (stdio, or TCP on a free port) and sends `initialize` and `launch`;
//! 3. on `initialized`, sends the breakpoints and `configurationDone`;
//! 4. on `stopped`, reads the stack, the scopes and variables of the top frame, and places each
//!    frame on its Blueprint node (the line's first character → the smallest node, as traces);
//! 5. relays the program's output to the console.
//!
//! js-debug runs the program in a child session (`startDebugging`): Nodex opens a second
//! connection to the same adapter and follows that one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nodex_dap::{Client, Incoming, types};
use nodex_ir::Language;
use nodex_lang_api::{AdapterSpec, AdapterTransport, DebugLauncher};
use nodex_trace::Resolver;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::runner::{line, push_output};
use crate::{AppState, ServerEvent};

/// Where the debugger is.
#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DebugState {
    Idle,
    Starting,
    Running,
    Paused,
    Exited,
    Failed,
}

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct DebugStatus {
    pub state: DebugState,
    pub detail: String,
    /// Language whose adapter is missing (the UI offers to install it).
    pub missing_adapter: Option<Language>,
}

/// One frame of the paused thread, placed on the graphs.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct DebugFrame {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// Project-relative file, when the frame is in the project.
    pub file: Option<String>,
    #[ts(type = "number")]
    pub line: i64,
    pub flow: Option<String>,
    pub node: Option<String>,
}

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct DebugVariable {
    pub name: String,
    pub value: String,
    pub ty: Option<String>,
    /// Non-zero when it has children (`/api/debug/variables?ref=`).
    #[ts(type = "number")]
    pub reference: i64,
}

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct DebugScope {
    pub name: String,
    #[ts(type = "number")]
    pub reference: i64,
    pub variables: Vec<DebugVariable>,
}

/// The program stopped (breakpoint, step, pause, exception).
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct DebugStopped {
    pub reason: String,
    pub description: Option<String>,
    #[ts(type = "number")]
    pub thread: i64,
    pub frames: Vec<DebugFrame>,
    /// Scopes of the top frame, with their variables.
    pub scopes: Vec<DebugScope>,
}

/// A breakpoint, by file and line (1-based), as the UI and `.nodex/breakpoints.json` keep it.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[ts(export)]
pub struct Breakpoint {
    pub file: String,
    pub line: u32,
    #[serde(default)]
    pub verified: bool,
}

/// A command for the running session.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DebugCommand {
    Continue,
    Next,
    StepIn,
    StepOut,
    Pause,
    Stop,
}

enum Msg {
    /// From the adapter connection `which` (0 = the first, 1 = a js-debug child).
    Adapter(usize, Incoming),
    Command(DebugCommand),
    Breakpoints(String),
}

/// A session, registered from the moment Debug is pressed (so nothing else starts while it
/// builds).
struct Session {
    /// The connection that runs the program (a js-debug child, or the only one); `None` while
    /// building and starting the adapter.
    client: Option<Arc<Client>>,
    tx: Sender<Msg>,
    thread: Option<i64>,
    /// Stop pressed before the adapter was connected: the session ends instead of launching.
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct DebugManager {
    session: Mutex<Option<Session>>,
    status: Mutex<Option<DebugStatus>>,
    last_stop: Mutex<Option<DebugStopped>>,
}

impl DebugManager {
    pub fn is_busy(&self) -> bool {
        self.session.lock().map(|s| s.is_some()).unwrap_or(false)
    }

    pub fn status(&self) -> DebugStatus {
        self.status
            .lock()
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or(DebugStatus { state: DebugState::Idle, detail: String::new(), missing_adapter: None })
    }

    pub fn last_stop(&self) -> Option<DebugStopped> {
        self.last_stop.lock().ok().and_then(|s| s.clone())
    }

    fn client(&self) -> Option<Arc<Client>> {
        self.session.lock().ok()?.as_ref().and_then(|s| s.client.clone())
    }

    /// Sends a command to the session. While it is still building, only Stop is accepted: the
    /// session then ends without launching the program.
    pub fn command(&self, cmd: DebugCommand) -> Result<(), String> {
        let s = self.session.lock().map_err(|_| "poisoned")?;
        let s = s.as_ref().ok_or("no debug session")?;
        if s.client.is_none() {
            if cmd != DebugCommand::Stop {
                return Err("the program is still starting".into());
            }
            s.cancelled.store(true, Ordering::SeqCst);
            return Ok(());
        }
        s.tx.send(Msg::Command(cmd)).map_err(|_| "the debug session ended".to_owned())
    }

    /// Children of a variable (or the variables of a scope).
    pub fn variables(&self, reference: i64) -> Result<Vec<DebugVariable>, String> {
        let client = self.client().ok_or("no debug session")?;
        variables(&client, reference)
    }

    /// The scopes and variables of another frame of the paused thread.
    pub fn scopes(&self, frame: i64) -> Result<Vec<DebugScope>, String> {
        let client = self.client().ok_or("no debug session")?;
        scopes(&client, frame)
    }

    /// Evaluates an expression in a frame (watch expressions).
    pub fn evaluate(&self, expression: &str, frame: Option<i64>) -> Result<DebugVariable, String> {
        let client = self.client().ok_or("no debug session")?;
        let mut args = json!({ "expression": expression, "context": "watch" });
        if let Some(f) = frame {
            args["frameId"] = json!(f);
        }
        let r = client.request("evaluate", args)?;
        Ok(DebugVariable {
            name: expression.to_owned(),
            value: r.get("result").and_then(Value::as_str).unwrap_or("").to_owned(),
            ty: r.get("type").and_then(Value::as_str).map(str::to_owned),
            reference: r.get("variablesReference").and_then(Value::as_i64).unwrap_or(0),
        })
    }

    fn breakpoints_changed(&self, file: &str) {
        if let Ok(s) = self.session.lock()
            && let Some(s) = s.as_ref()
        {
            let _ = s.tx.send(Msg::Breakpoints(file.to_owned()));
        }
    }
}

fn set_status(state: &AppState, status: DebugStatus) {
    if let Ok(mut s) = state.debug.status.lock() {
        *s = Some(status.clone());
    }
    let _ = state.events.send(ServerEvent::DebugStatus(status));
}

fn status(state: DebugState, detail: impl Into<String>) -> DebugStatus {
    DebugStatus { state, detail: detail.into(), missing_adapter: None }
}

// ---------------------------------------------------------------------------------------------
// Breakpoints (`.nodex/breakpoints.json`)

fn breakpoints_file(state: &AppState) -> PathBuf {
    state.project.lock().map(|p| p.root().join(".nodex").join("breakpoints.json")).unwrap_or_default()
}

pub fn breakpoints(state: &AppState) -> Vec<Breakpoint> {
    std::fs::read_to_string(breakpoints_file(state)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Replaces the breakpoints of one file; a running session gets them at once.
pub fn set_breakpoints(state: &AppState, file: &str, lines: Vec<u32>) -> Result<Vec<Breakpoint>, String> {
    let mut all: Vec<Breakpoint> = breakpoints(state).into_iter().filter(|b| b.file != file).collect();
    for l in lines {
        all.push(Breakpoint { file: file.to_owned(), line: l, verified: false });
    }
    all.sort();
    all.dedup_by(|a, b| a.file == b.file && a.line == b.line);
    let path = breakpoints_file(state);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    state.debug.breakpoints_changed(file);
    Ok(all)
}

// ---------------------------------------------------------------------------------------------
// Starting a session

/// Starts debugging the project. Returns an error if something is already running or the
/// language has no debugger.
pub fn start(state: &Arc<AppState>) -> Result<(), String> {
    if state.debug.is_busy() || state.run.is_busy() {
        return Err("a run or a debug session is already in progress".into());
    }
    let (root, language, launcher) = {
        let project = state.project.lock().map_err(|_| "project lock poisoned")?;
        let found = project
            .active_frontends()
            .into_iter()
            .find_map(|f| f.debugger(project.root()).map(|d| (f.id(), d)))
            .ok_or("no language in this project has a debugger yet")?;
        (project.root().to_path_buf(), found.0, found.1)
    };
    let spec = launcher.adapter();
    let Some((program, _)) = crate::adapters::locate(&spec) else {
        let s = DebugStatus {
            state: DebugState::Failed,
            detail: format!("the {} debug adapter is not installed", spec.name),
            missing_adapter: Some(language),
        };
        set_status(state, s);
        return Err(format!("the {} debug adapter is not installed", spec.name));
    };
    // Registered at once: Run and another Debug are refused, and Stop works during the build.
    let (tx, msgs) = mpsc::channel::<Msg>();
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut session = state.debug.session.lock().map_err(|_| "debug lock poisoned")?;
        if session.is_some() {
            return Err("a run or a debug session is already in progress".into());
        }
        *session = Some(Session { client: None, tx: tx.clone(), thread: None, cancelled: cancelled.clone() });
    }
    if let Ok(mut l) = state.debug.last_stop.lock() {
        *l = None;
    }
    set_status(state, status(DebugState::Starting, "building for debugging…"));
    let registered = state.clone();
    let state = state.clone();
    let channel = (tx, msgs, cancelled);
    std::thread::Builder::new()
        .name("nodex-debug".into())
        .spawn(move || {
            let result = session(&state, &root, launcher, spec, program, channel);
            if let Ok(mut s) = state.debug.session.lock() {
                *s = None;
            }
            match result {
                Ok(()) => set_status(&state, status(DebugState::Exited, "the program ended")),
                Err(e) => {
                    push_output(&state, vec![line("nodex", format!("✗ {e}"))]);
                    set_status(&state, status(DebugState::Failed, e));
                }
            }
        })
        .map_err(|e| {
            if let Ok(mut s) = registered.debug.session.lock() {
                *s = None;
            }
            e.to_string()
        })?;
    Ok(())
}

fn free_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

/// A started adapter: its first connection, the address of a TCP adapter (js-debug child sessions
/// connect to it again) and the adapter process when Nodex spawned it as a server.
type StartedAdapter = (Client, Receiver<Incoming>, Option<String>, Option<std::process::Child>);

/// Starts the adapter.
fn start_adapter(spec: &AdapterSpec, program: &Path) -> Result<StartedAdapter, String> {
    let port = free_port()?;
    let args: Vec<String> = spec.args.iter().map(|a| a.replace("{port}", &port.to_string())).collect();
    let mut cmd = match spec.interpreter {
        Some(i) => {
            // `node` honours NODEX_NODE, as the TypeScript sidecar and Run do.
            let interpreter = if i == "node" { nodex_lang_typescript::semantic::node() } else { i.into() };
            let mut c = nodex_lang_api::process::command(interpreter);
            c.arg(program);
            c
        }
        None => nodex_lang_api::process::command(program),
    };
    cmd.args(&args);
    if let Some(dir) = program.parent() {
        cmd.current_dir(dir);
    }
    match spec.transport {
        AdapterTransport::Stdio => {
            let (client, rx) = Client::spawn(cmd)?;
            Ok((client, rx, None, None))
        }
        AdapterTransport::Tcp => {
            let child = cmd
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("cannot start the {} debug adapter: {e}", spec.name))?;
            let addr = format!("127.0.0.1:{port}");
            let (client, rx) = Client::connect(&addr, Duration::from_secs(20))?;
            Ok((client, rx, Some(addr), Some(child)))
        }
    }
}

/// Forwards one connection's messages into the session loop.
fn forward(which: usize, rx: Receiver<Incoming>, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        while let Ok(m) = rx.recv() {
            let closed = matches!(m, Incoming::Closed);
            if tx.send(Msg::Adapter(which, m)).is_err() || closed {
                break;
            }
        }
    });
}

/// A path without the Windows verbatim prefix (`\\?\D:\x` becomes `D:\x`): debug adapters
/// compare source paths as text.
fn plain(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

type Channel = (Sender<Msg>, Receiver<Msg>, Arc<AtomicBool>);

fn session(state: &Arc<AppState>, root: &Path, launcher: Box<dyn DebugLauncher>, spec: AdapterSpec, program: PathBuf, channel: Channel) -> Result<(), String> {
    let (tx, msgs, cancelled) = channel;
    let stopped_early = || {
        let yes = cancelled.load(Ordering::SeqCst);
        if yes {
            push_output(state, vec![line("nodex", "■ stopped")]);
        }
        yes
    };
    let root = &plain(root);
    // 1. Debug build.
    let out_dir = root.join(".nodex").join("run");
    let st = state.clone();
    let launch = launcher.prepare(root, &out_dir, &mut |l| push_output(&st, vec![line("build", l)]))?;
    if stopped_early() {
        return Ok(());
    }
    push_output(state, vec![line("nodex", format!("🐞 {} with {} {}", launch.description, spec.name, spec.version))]);

    // 2. Adapter.
    let (client, rx, addr, mut adapter_process) = start_adapter(&spec, &program)?;
    let _cleanup = Cleanup(adapter_process.take());
    let client = Arc::new(client);
    forward(0, rx, tx.clone());
    // From here on, commands (Stop too) go through the session loop.
    if let Ok(mut s) = state.debug.session.lock()
        && let Some(s) = s.as_mut()
    {
        s.client = Some(client.clone());
    }
    if stopped_early() {
        client.shutdown(!launch.attach);
        return Ok(());
    }
    // Whether each connection takes `configurationDone` (unity-dap does not).
    let takes_done = |caps: &Value| caps.get("supportsConfigurationDoneRequest").and_then(Value::as_bool).unwrap_or(false);
    let mut done_supported = vec![takes_done(&client.initialize(spec.adapter_id)?)];
    // An attached program (the Unity Editor) is detached from on Stop, never ended.
    let terminate = !launch.attach;
    let launch_reply = client.send_request(if launch.attach { "attach" } else { "launch" }, launch.launch.clone())?;
    let mut clients: Vec<Arc<Client>> = vec![client.clone()];
    let mut active = 0usize;
    set_status(state, status(DebugState::Running, launch.description.clone()));

    // 3. Events and commands.
    let resolver = || state.project.lock().ok().map(|p| Resolver::new(p.flows()));
    let mut launch_reply = Some(launch_reply);
    loop {
        if let Some(r) = &launch_reply
            && let Ok(result) = r.try_recv()
        {
            launch_reply = None;
            if let Err(e) = result {
                let what = if launch.attach { "attach to the program" } else { "start the program" };
                return Err(format!("the adapter could not {what}: {e}"));
            }
        }
        let msg = match msgs.recv_timeout(Duration::from_millis(200)) {
            Ok(m) => m,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        match msg {
            Msg::Adapter(which, Incoming::Event { event, body }) => match event.as_str() {
                "initialized" => {
                    let c = clients[which].clone();
                    configure(state, &c, root, done_supported[which])?;
                }
                "stopped" if which == active => {
                    let thread = body.get("threadId").and_then(Value::as_i64).unwrap_or(1);
                    let reason = body.get("reason").and_then(Value::as_str).unwrap_or("pause").to_owned();
                    let description = body.get("description").and_then(Value::as_str).map(str::to_owned).or_else(|| body.get("text").and_then(Value::as_str).map(str::to_owned));
                    if let Ok(mut s) = state.debug.session.lock()
                        && let Some(s) = s.as_mut()
                    {
                        s.thread = Some(thread);
                    }
                    let stopped = describe_stop(&clients[active], root, thread, reason, description, resolver().as_ref());
                    if let Ok(mut l) = state.debug.last_stop.lock() {
                        *l = Some(stopped.clone());
                    }
                    set_status(state, status(DebugState::Paused, stopped.reason.clone()));
                    let _ = state.events.send(ServerEvent::DebugStopped(Box::new(stopped)));
                }
                "continued" if which == active => {
                    set_status(state, status(DebugState::Running, launch.description.clone()));
                    let _ = state.events.send(ServerEvent::DebugContinued);
                }
                "output" => {
                    let category = body.get("category").and_then(Value::as_str).unwrap_or("console");
                    if category == "telemetry" {
                        continue;
                    }
                    let text = body.get("output").and_then(Value::as_str).unwrap_or("");
                    let stream = if category == "stderr" { "err" } else if category == "console" || category == "important" { "nodex" } else { "out" };
                    let lines: Vec<_> = text.lines().filter(|l| !l.is_empty()).map(|l| line(stream, l)).collect();
                    if !lines.is_empty() {
                        push_output(state, lines);
                    }
                }
                "terminated" if which == active => break,
                "exited" if which == active => {
                    let code = body.get("exitCode").and_then(Value::as_i64).unwrap_or(0);
                    push_output(state, vec![line("nodex", format!("■ exited with code {code}"))]);
                }
                _ => {}
            },
            Msg::Adapter(which, Incoming::Request { seq, command, arguments }) => {
                let c = clients[which].clone();
                match command.as_str() {
                    // js-debug: run the program in a child session on a new connection.
                    "startDebugging" => {
                        c.respond(seq, &command, true, json!({}))?;
                        let addr = addr.as_deref().ok_or("startDebugging needs a TCP adapter")?;
                        let (child, child_rx) = Client::connect(addr, Duration::from_secs(10))?;
                        let child = Arc::new(child);
                        forward(clients.len(), child_rx, tx.clone());
                        done_supported.push(takes_done(&child.initialize(spec.adapter_id)?));
                        let config = arguments.get("configuration").cloned().unwrap_or(Value::Null);
                        let _ = child.send_request("launch", config)?;
                        clients.push(child.clone());
                        active = clients.len() - 1;
                        if let Ok(mut s) = state.debug.session.lock()
                            && let Some(s) = s.as_mut()
                        {
                            s.client = Some(child);
                        }
                    }
                    _ => {
                        c.respond(seq, &command, false, json!({ "error": { "id": 1, "format": "not supported by Nodex" } }))?;
                    }
                }
            }
            Msg::Adapter(which, Incoming::Closed) => {
                if which == active || which == 0 {
                    break;
                }
            }
            Msg::Command(cmd) => {
                let c = clients[active].clone();
                let thread = state.debug.session.lock().ok().and_then(|s| s.as_ref().and_then(|s| s.thread)).unwrap_or(1);
                let args = json!({ "threadId": thread });
                let result = match cmd {
                    DebugCommand::Continue => c.request("continue", args),
                    DebugCommand::Next => c.request("next", args),
                    DebugCommand::StepIn => c.request("stepIn", args),
                    DebugCommand::StepOut => c.request("stepOut", args),
                    DebugCommand::Pause => c.request("pause", args),
                    DebugCommand::Stop => {
                        for c in clients.iter().rev() {
                            c.shutdown(terminate);
                        }
                        push_output(state, vec![line("nodex", "■ stopped")]);
                        break;
                    }
                };
                match result {
                    Ok(_) if cmd != DebugCommand::Pause => {
                        // Some adapters do not send `continued` after a step.
                        set_status(state, status(DebugState::Running, launch.description.clone()));
                        let _ = state.events.send(ServerEvent::DebugContinued);
                    }
                    Ok(_) => {}
                    Err(e) => push_output(state, vec![line("nodex", format!("✗ {e}"))]),
                }
            }
            Msg::Breakpoints(file) => {
                let c = clients[active].clone();
                let _ = send_file_breakpoints(state, &c, root, &file);
            }
        }
    }
    for c in clients.iter().rev() {
        c.shutdown(terminate);
    }
    Ok(())
}

/// Kills the adapter process (TCP adapters) when the session ends.
struct Cleanup(Option<std::process::Child>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            #[cfg(windows)]
            let _ = nodex_lang_api::process::command("taskkill")
                .args(["/PID", &c.id().to_string(), "/T", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Breakpoints of every file, exception breakpoints, `configurationDone`.
/// `configuration_done`: the adapter announced `supportsConfigurationDoneRequest` (DAP clients
/// must not send it otherwise).
fn configure(state: &AppState, client: &Client, root: &Path, configuration_done: bool) -> Result<(), String> {
    let mut by_file: BTreeMap<String, ()> = BTreeMap::new();
    for b in breakpoints(state) {
        by_file.insert(b.file, ());
    }
    for file in by_file.keys() {
        // One file the adapter rejects (renamed, deleted) must not end the session.
        if let Err(e) = send_file_breakpoints(state, client, root, file) {
            push_output(state, vec![line("nodex", format!("breakpoints in {file} not set: {e}"))]);
        }
    }
    let _ = client.request("setExceptionBreakpoints", json!({ "filters": [] }));
    if configuration_done {
        client.request("configurationDone", json!({}))?;
    }
    Ok(())
}

fn send_file_breakpoints(state: &AppState, client: &Client, root: &Path, file: &str) -> Result<(), String> {
    let lines: Vec<u32> = breakpoints(state).into_iter().filter(|b| b.file == file).map(|b| b.line).collect();
    // Native separators: adapters match the debug info's paths as text.
    let native = if cfg!(windows) { file.replace('/', "\\") } else { file.to_owned() };
    let path = plain(&root.join(native));
    let body = client.request(
        "setBreakpoints",
        json!({
            "source": { "path": path, "name": Path::new(file).file_name().map(|n| n.to_string_lossy().into_owned()) },
            "breakpoints": lines.iter().map(|l| json!({ "line": l })).collect::<Vec<_>>(),
            "lines": lines,
        }),
    )?;
    let verified = body
        .get("breakpoints")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|b| b.get("verified").and_then(Value::as_bool) == Some(true)).count())
        .unwrap_or(0);
    // Adapters bind most breakpoints later (when the module or its source map loads), so only a
    // reason that is not "pending" is worth showing.
    let why = body.pointer("/breakpoints/0/message").and_then(Value::as_str).unwrap_or("");
    let pending = ["provisional", "pending", "not yet", "will be"].iter().any(|w| why.to_ascii_lowercase().contains(w));
    if verified < lines.len() && !why.is_empty() && !pending {
        push_output(state, vec![line("nodex", format!("breakpoints in {file}: {verified} of {} bound ({why})", lines.len()))]);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Reading a stop

fn variables(client: &Client, reference: i64) -> Result<Vec<DebugVariable>, String> {
    let body = client.request("variables", json!({ "variablesReference": reference }))?;
    let vars: Vec<types::Variable> = serde_json::from_value(body.get("variables").cloned().unwrap_or(Value::Array(Vec::new()))).map_err(|e| e.to_string())?;
    Ok(vars
        .into_iter()
        .filter(|v| !v.name.starts_with("__nx"))
        .map(|v| DebugVariable { name: v.name, value: v.value, ty: v.ty, reference: v.variables_reference })
        .collect())
}

fn scopes(client: &Client, frame: i64) -> Result<Vec<DebugScope>, String> {
    let body = client.request("scopes", json!({ "frameId": frame }))?;
    let scopes: Vec<types::Scope> = serde_json::from_value(body.get("scopes").cloned().unwrap_or(Value::Array(Vec::new()))).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (i, s) in scopes.into_iter().enumerate() {
        // Cheap scopes near the code (block, locals, arguments) are read at once; globals,
        // registers and the rest when opened.
        let far = s.expensive || ["global", "register", "module", "static"].iter().any(|w| s.name.to_lowercase().contains(w));
        let vars = if i < 4 && !far { variables(client, s.variables_reference).unwrap_or_default() } else { Vec::new() };
        out.push(DebugScope { name: s.name, reference: s.variables_reference, variables: vars });
    }
    Ok(out)
}

/// The project-relative path of an adapter's absolute path, if the file is in the project.
fn relative(root: &Path, path: &str) -> Option<String> {
    let norm = |p: &str| {
        let p = p.replace('\\', "/");
        let p = p.strip_prefix("//?/").unwrap_or(&p).to_owned();
        if cfg!(windows) { p.to_lowercase() } else { p }
    };
    let root_s = root.to_string_lossy();
    let r = norm(&root_s);
    let r = r.trim_end_matches('/');
    let full = path.replace('\\', "/");
    let full = full.strip_prefix("//?/").unwrap_or(&full);
    let lower = norm(full);
    lower.strip_prefix(r).filter(|rest| rest.starts_with('/')).map(|_| full[r.len() + 1..].to_owned())
}

/// The byte offset of the first non-blank character of a 1-based line.
fn line_offset(text: &str, line: i64) -> Option<u32> {
    let mut start = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i as i64 + 1 == line {
            let indent = l.len() - l.trim_start().len();
            return Some((start + indent) as u32);
        }
        start += l.len();
    }
    None
}

fn describe_stop(client: &Client, root: &Path, thread: i64, reason: String, description: Option<String>, resolver: Option<&Resolver>) -> DebugStopped {
    let frames: Vec<types::StackFrame> = client
        .request("stackTrace", json!({ "threadId": thread, "startFrame": 0, "levels": 40 }))
        .ok()
        .and_then(|b| serde_json::from_value(b.get("stackFrames").cloned().unwrap_or(Value::Null)).ok())
        .unwrap_or_default();
    let mut texts: BTreeMap<String, Option<String>> = BTreeMap::new();
    let frames: Vec<DebugFrame> = frames
        .into_iter()
        .map(|f| {
            let file = f.source.as_ref().and_then(|s| s.path.as_deref()).and_then(|p| relative(root, p));
            let (flow, node) = match (&file, resolver) {
                (Some(rel), Some(resolver)) => {
                    let text = texts.entry(rel.clone()).or_insert_with(|| std::fs::read_to_string(root.join(rel)).ok());
                    match text.as_deref().and_then(|t| line_offset(t, f.line)) {
                        Some(offset) => match resolver.locate(rel, offset) {
                            Some(loc) => (Some(loc.flow), loc.node),
                            None => (None, None),
                        },
                        None => (None, None),
                    }
                }
                _ => (None, None),
            };
            DebugFrame { id: f.id, name: f.name, file, line: f.line, flow, node }
        })
        .collect();
    let top = frames.iter().find(|f| f.file.is_some()).or(frames.first()).map(|f| f.id);
    let scopes = top.and_then(|id| scopes(client, id).ok()).unwrap_or_default();
    DebugStopped { reason, description, thread, frames, scopes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_lines() {
        if cfg!(windows) {
            // Adapters may report another drive-letter case and backslashes.
            assert_eq!(relative(Path::new(r"D:\Proj\App"), r"d:\proj\app\src\main.rs").as_deref(), Some("src/main.rs"));
        }
        assert_eq!(relative(Path::new("/p/app"), "/p/app/src/x.ts").as_deref(), Some("src/x.ts"));
        assert_eq!(relative(Path::new("/p/app"), "/p/other/x.ts"), None);
        assert_eq!(line_offset("a\n    b\nc", 2), Some(6));
        assert_eq!(line_offset("a", 3), None);
    }
}
