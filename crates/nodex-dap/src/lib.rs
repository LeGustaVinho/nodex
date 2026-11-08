//! # Nodex DAP client
//!
//! A small client for the [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/):
//! `Content-Length` framed JSON over a debug adapter's stdio or a TCP socket.
//!
//! - [`Client::request`] sends a request and waits for its response;
//! - events and the adapter's own requests (`runInTerminal`, `startDebugging`) arrive on the
//!   [`Incoming`] channel; answer the latter with [`Client::respond`].
//!
//! It is synchronous (a reader thread per connection): debug sessions are few and their
//! traffic small.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

/// Something the adapter sent that is not a response to our requests.
#[derive(Debug, Clone)]
pub enum Incoming {
    /// `{"type":"event","event":…,"body":…}`.
    Event { event: String, body: Value },
    /// A reverse request (`runInTerminal`, `startDebugging`); answer it with [`Client::respond`].
    Request { seq: i64, command: String, arguments: Value },
    /// The connection closed.
    Closed,
}

type Pending = Arc<Mutex<HashMap<i64, Sender<Result<Value, String>>>>>;

/// A connection to a debug adapter.
pub struct Client {
    writer: Mutex<Box<dyn Write + Send>>,
    seq: AtomicI64,
    pending: Pending,
    child: Mutex<Option<Child>>,
    timeout: Duration,
}

/// Writes one framed message.
fn write_message(w: &mut dyn Write, message: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(&body)?;
    w.flush()
}

/// Reads one framed message; `None` at the end of the stream.
fn read_message(r: &mut dyn BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            if length.is_some() {
                break;
            }
            continue;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            length = v.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0u8; length?];
    r.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

impl Client {
    /// Starts an adapter that speaks DAP on its stdio (netcoredbg `--interpreter=vscode`).
    pub fn spawn(mut command: Command) -> Result<(Self, Receiver<Incoming>), String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start the debug adapter: {e}"))?;
        let stdin: ChildStdin = child.stdin.take().ok_or("no adapter stdin")?;
        let stdout = child.stdout.take().ok_or("no adapter stdout")?;
        let (client, rx) = Self::over(Box::new(stdin), Box::new(stdout));
        *client.child.lock().map_err(|_| "poisoned")? = Some(child);
        Ok((client, rx))
    }

    /// Connects to an adapter listening on TCP (js-debug's `dapDebugServer`, CodeLLDB `--port`).
    /// Retries for a while: adapters take a moment to open their port.
    pub fn connect(addr: &str, wait: Duration) -> Result<(Self, Receiver<Incoming>), String> {
        let deadline = std::time::Instant::now() + wait;
        let stream = loop {
            match TcpStream::connect(addr) {
                Ok(s) => break s,
                Err(e) if std::time::Instant::now() >= deadline => return Err(format!("cannot reach the debug adapter at {addr}: {e}")),
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        };
        let _ = stream.set_nodelay(true);
        let reader = stream.try_clone().map_err(|e| e.to_string())?;
        Ok(Self::over(Box::new(stream), Box::new(reader)))
    }

    /// A client over any byte streams (tests use in-memory pipes).
    pub fn over(writer: Box<dyn Write + Send>, reader: Box<dyn Read + Send>) -> (Self, Receiver<Incoming>) {
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = mpsc::channel();
        let p = pending.clone();
        std::thread::Builder::new()
            .name("nodex-dap-read".into())
            .spawn(move || {
                let mut reader = BufReader::new(reader);
                while let Some(message) = read_message(&mut reader) {
                    match message.get("type").and_then(Value::as_str) {
                        Some("response") => {
                            let seq = message.get("request_seq").and_then(Value::as_i64).unwrap_or(-1);
                            let ok = message.get("success").and_then(Value::as_bool).unwrap_or(false);
                            let result = if ok {
                                Ok(message.get("body").cloned().unwrap_or(Value::Null))
                            } else {
                                Err(message
                                    .pointer("/body/error/format")
                                    .and_then(Value::as_str)
                                    .or_else(|| message.get("message").and_then(Value::as_str))
                                    .unwrap_or("request failed")
                                    .to_owned())
                            };
                            if let Some(waiter) = p.lock().ok().and_then(|mut m| m.remove(&seq)) {
                                let _ = waiter.send(result);
                            }
                        }
                        Some("event") => {
                            let event = message.get("event").and_then(Value::as_str).unwrap_or("").to_owned();
                            let body = message.get("body").cloned().unwrap_or(Value::Null);
                            if tx.send(Incoming::Event { event, body }).is_err() {
                                break;
                            }
                        }
                        Some("request") => {
                            let _ = tx.send(Incoming::Request {
                                seq: message.get("seq").and_then(Value::as_i64).unwrap_or(0),
                                command: message.get("command").and_then(Value::as_str).unwrap_or("").to_owned(),
                                arguments: message.get("arguments").cloned().unwrap_or(Value::Null),
                            });
                        }
                        _ => {}
                    }
                }
                // Fail every request still waiting.
                if let Ok(mut m) = p.lock() {
                    for (_, w) in m.drain() {
                        let _ = w.send(Err("the debug adapter closed the connection".into()));
                    }
                }
                let _ = tx.send(Incoming::Closed);
            })
            .expect("can start a thread");
        let client = Self { writer: Mutex::new(writer), seq: AtomicI64::new(1), pending, child: Mutex::new(None), timeout: Duration::from_secs(30) };
        (client, rx)
    }

    /// Sends a request and waits for its response body.
    pub fn request(&self, command: &str, arguments: Value) -> Result<Value, String> {
        let rx = self.send_request(command, arguments)?;
        rx.recv_timeout(self.timeout).map_err(|_| format!("the debug adapter did not answer {command}"))?
    }

    /// Sends a request without waiting (`launch` answers only after `configurationDone` with some
    /// adapters); the response arrives on the returned channel.
    pub fn send_request(&self, command: &str, arguments: Value) -> Result<Receiver<Result<Value, String>>, String> {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().map_err(|_| "poisoned")?.insert(seq, tx);
        let message = json!({ "seq": seq, "type": "request", "command": command, "arguments": arguments });
        let mut w = self.writer.lock().map_err(|_| "poisoned")?;
        write_message(&mut **w, &message).map_err(|e| format!("cannot write to the debug adapter: {e}"))?;
        Ok(rx)
    }

    /// Answers a reverse request from the adapter.
    pub fn respond(&self, request_seq: i64, command: &str, success: bool, body: Value) -> Result<(), String> {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let message = json!({ "seq": seq, "type": "response", "request_seq": request_seq, "command": command, "success": success, "body": body });
        let mut w = self.writer.lock().map_err(|_| "poisoned")?;
        write_message(&mut **w, &message).map_err(|e| e.to_string())
    }

    /// The `initialize` request with the capabilities Nodex supports.
    pub fn initialize(&self, adapter_id: &str) -> Result<Value, String> {
        self.request(
            "initialize",
            json!({
                "clientID": "nodex",
                "clientName": "Nodex",
                "adapterID": adapter_id,
                "locale": "en",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "pathFormat": "path",
                "supportsVariableType": true,
                "supportsVariablePaging": false,
                "supportsRunInTerminalRequest": true,
                "supportsStartDebuggingRequest": true,
                "supportsProgressReporting": false,
                "supportsInvalidatedEvent": false,
            }),
        )
    }

    /// Ends the session (and the adapter process when we started it). `terminate` also ends the
    /// debugged program; an attached one (the Unity Editor) is only detached from.
    pub fn shutdown(&self, terminate: bool) {
        let _ = self.send_request("disconnect", json!({ "terminateDebuggee": terminate }));
        std::thread::sleep(Duration::from_millis(200));
        if let Ok(mut c) = self.child.lock()
            && let Some(mut child) = c.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock()
            && let Some(mut child) = c.take()
        {
            let _ = child.kill();
        }
    }
}

/// Typed views of the bodies Nodex reads.
pub mod types {
    use serde::Deserialize;

    #[derive(Deserialize, Debug, Clone)]
    pub struct StackFrame {
        pub id: i64,
        pub name: String,
        #[serde(default)]
        pub source: Option<Source>,
        pub line: i64,
        #[serde(default)]
        pub column: i64,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct Source {
        #[serde(default)]
        pub path: Option<String>,
        #[serde(default)]
        pub name: Option<String>,
    }

    #[derive(Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Scope {
        pub name: String,
        pub variables_reference: i64,
        #[serde(default)]
        pub expensive: bool,
    }

    #[derive(Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Variable {
        pub name: String,
        pub value: String,
        #[serde(default, rename = "type")]
        pub ty: Option<String>,
        #[serde(default)]
        pub variables_reference: i64,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct Thread {
        pub id: i64,
        pub name: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A scripted adapter: answers initialize/launch/stackTrace and sends events.
    fn fake_adapter() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);
            let mut seq = 100;
            let send = |w: &mut TcpStream, v: Value| {
                write_message(w, &v).unwrap();
            };
            while let Some(m) = read_message(&mut reader) {
                if m["type"] == "response" {
                    // Our reverse request was answered: report it as an event.
                    seq += 1;
                    send(&mut writer, json!({"seq": seq, "type": "event", "event": "output", "body": {"output": format!("answered {}", m["command"])}}));
                    continue;
                }
                let command = m["command"].as_str().unwrap().to_owned();
                let req = m["seq"].as_i64().unwrap();
                seq += 1;
                let body = match command.as_str() {
                    "initialize" => json!({"supportsConfigurationDoneRequest": true}),
                    "stackTrace" => json!({"stackFrames": [{"id": 1, "name": "main", "line": 7, "column": 1, "source": {"path": "/p/main.rs"}}]}),
                    "fail" => {
                        send(&mut writer, json!({"seq": seq, "type": "response", "request_seq": req, "command": command, "success": false, "message": "nope"}));
                        continue;
                    }
                    _ => json!({}),
                };
                send(&mut writer, json!({"seq": seq, "type": "response", "request_seq": req, "command": command, "success": true, "body": body}));
                if command == "initialize" {
                    seq += 1;
                    send(&mut writer, json!({"seq": seq, "type": "event", "event": "initialized"}));
                    seq += 1;
                    send(&mut writer, json!({"seq": seq, "type": "request", "command": "runInTerminal", "arguments": {"args": ["prog"]}}));
                }
            }
        });
        addr
    }

    #[test]
    fn requests_events_and_reverse_requests() {
        let addr = fake_adapter();
        let (client, rx) = Client::connect(&addr, Duration::from_secs(2)).unwrap();
        let caps = client.initialize("lldb").unwrap();
        assert_eq!(caps["supportsConfigurationDoneRequest"], true);
        match rx.recv_timeout(Duration::from_secs(2)).unwrap() {
            Incoming::Event { event, .. } => assert_eq!(event, "initialized"),
            other => panic!("{other:?}"),
        }
        let Incoming::Request { seq, command, arguments } = rx.recv_timeout(Duration::from_secs(2)).unwrap() else { panic!() };
        assert_eq!(command, "runInTerminal");
        assert_eq!(arguments["args"][0], "prog");
        client.respond(seq, &command, true, json!({"processId": 1})).unwrap();
        match rx.recv_timeout(Duration::from_secs(2)).unwrap() {
            Incoming::Event { event, body } => {
                assert_eq!(event, "output");
                assert_eq!(body["output"], "answered \"runInTerminal\"");
            }
            other => panic!("{other:?}"),
        }
        let stack = client.request("stackTrace", json!({"threadId": 1})).unwrap();
        let frames: Vec<types::StackFrame> = serde_json::from_value(stack["stackFrames"].clone()).unwrap();
        assert_eq!(frames[0].line, 7);
        assert_eq!(frames[0].source.as_ref().unwrap().path.as_deref(), Some("/p/main.rs"));
        assert_eq!(client.request("fail", json!({})).unwrap_err(), "nope");
    }
}
