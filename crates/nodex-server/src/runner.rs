//! Runs the project instrumented and streams its execution to the browsers.
//!
//! 1. The language's `TraceInstrumenter` builds and instruments into `<project>/.nodex/run`.
//! 2. A loopback TCP listener with a fresh token receives the program's trace events.
//! 3. The program runs with `NODEX_TRACE`/`NODEX_TRACE_TOKEN`; its output is forwarded.
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
}

impl Default for RunManager {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: RunStatus { state: RunState::Idle, detail: String::new(), exit_code: None },
                output: VecDeque::new(),
                recorder: None,
                kill: None,
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

    pub fn stop(&self) -> bool {
        self.inner.lock().ok().and_then(|mut i| i.kill.take()).map(|k| k.send(()).is_ok()).unwrap_or(false)
    }
}

fn set_status(state: &AppState, status: RunStatus) {
    if let Ok(mut inner) = state.run.inner.lock() {
        inner.status = status.clone();
    }
    let _ = state.events.send(ServerEvent::RunStatus(status));
}

fn push_output(state: &AppState, lines: Vec<OutputLine>) {
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

fn line(stream: &str, text: impl Into<String>) -> OutputLine {
    OutputLine { stream: stream.into(), text: text.into() }
}

/// Starts a run. Returns an error if one is already in progress or nothing can run.
pub fn start(state: &Arc<AppState>) -> Result<(), String> {
    if state.run.is_busy() {
        return Err("a run is already in progress".into());
    }
    let (root, instrumenter) = {
        let project = state.project.lock().map_err(|_| "project lock poisoned")?;
        let instrumenter = project
            .active_frontends()
            .into_iter()
            .find_map(|f| f.instrumenter())
            .ok_or("no language in this project supports live traces yet")?;
        (project.root().to_path_buf(), instrumenter)
    };
    if let Ok(mut inner) = state.run.inner.lock() {
        inner.output.clear();
        inner.recorder = None;
    }
    set_status(state, RunStatus { state: RunState::Preparing, detail: "building and instrumenting…".into(), exit_code: None });
    let state = state.clone();
    tokio::spawn(async move {
        if let Err(reason) = run(&state, root, instrumenter).await {
            push_output(&state, vec![line("nodex", format!("✗ {reason}"))]);
            set_status(&state, RunStatus { state: RunState::Failed, detail: reason, exit_code: None });
        }
    });
    Ok(())
}

async fn run(
    state: &Arc<AppState>,
    root: std::path::PathBuf,
    instrumenter: Box<dyn nodex_lang_api::TraceInstrumenter>,
) -> Result<(), String> {
    let out = output_forwarder(state.clone());

    // 1. Build and instrument (blocking).
    let log_tx = out.clone();
    let out_dir = root.join(".nodex").join("run");
    let prep_root = root.clone();
    let command = tokio::task::spawn_blocking(move || {
        instrumenter.prepare(&prep_root, &out_dir, &mut |l| {
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
    if let Ok(mut inner) = state.run.inner.lock() {
        inner.recorder = Some(recorder.clone());
    }

    // 3. Trace intake.
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let endpoint = listener.local_addr().map_err(|e| e.to_string())?;
    let trace_token = new_token();
    let intake = tokio::spawn(accept_traces(listener, trace_token.clone(), recorder.clone(), out.clone()));

    // 4. Launch.
    let mut child = tokio::process::Command::new(&command.program)
        .args(&command.args)
        .current_dir(&command.cwd)
        .env("NODEX_TRACE", endpoint.to_string())
        .env("NODEX_TRACE_TOKEN", &trace_token)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", command.program))?;
    let _ = out.send(line("nodex", format!("▶ {}", command.description)));
    set_status(state, RunStatus { state: RunState::Running, detail: command.description.clone(), exit_code: None });
    for (stream, reader) in [
        ("out", child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)),
        ("err", child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>)),
    ] {
        if let Some(reader) = reader {
            let tx = out.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(reader).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let _ = tx.send(line(stream, l));
                }
            });
        }
    }
    let (kill_tx, kill_rx) = oneshot::channel();
    if let Ok(mut inner) = state.run.inner.lock() {
        inner.kill = Some(kill_tx);
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
    let status = tokio::select! {
        status = child.wait() => status.map_err(|e| e.to_string())?,
        _ = kill_rx => {
            let _ = child.kill().await;
            let _ = out.send(line("nodex", "■ stopped"));
            child.wait().await.map_err(|e| e.to_string())?
        }
    };
    // Let the last trace batches arrive before the final frame.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = stop_ticker.send(());
    let _ = ticker.await;
    intake.abort();
    send_frame(state, &recorder);
    let code = status.code();
    let _ = out.send(line("nodex", format!("■ exited with code {}", code.map_or("?".into(), |c| c.to_string()))));
    if let Ok(mut inner) = state.run.inner.lock() {
        inner.kill = None;
    }
    set_status(state, RunStatus { state: RunState::Exited, detail: command.description, exit_code: code });
    Ok(())
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
) {
    while let Ok((socket, _)) = listener.accept().await {
        let token = token.clone();
        let recorder = recorder.clone();
        let out = out.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(socket);
            let mut hello = String::new();
            let ok = tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut hello)).await;
            if !matches!(ok, Ok(Ok(_))) || hello.trim_end() != format!("NODEX1 {token}") {
                return; // not our program
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
