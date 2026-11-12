use std::{path::Path, process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
};

fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let unc = path.strip_prefix("UNC/").or_else(|| path.strip_prefix("//"));
    let mut uri = if unc.is_some() { "file://".to_owned() } else { "file:///".to_owned() };
    for byte in unc.unwrap_or(path.trim_start_matches('/')).bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/' | b':') {
            uri.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

async fn send(stdin: &mut ChildStdin, message: Value) -> Option<()> {
    let body = serde_json::to_vec(&message).ok()?;
    stdin.write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes()).await.ok()?;
    stdin.write_all(&body).await.ok()
}

async fn response(stdout: &mut BufReader<ChildStdout>, stdin: &mut ChildStdin, id: u32) -> Option<Value> {
    loop {
        let mut length = None;
        loop {
            let mut line = String::new();
            if stdout.read_line(&mut line).await.ok()? == 0 {
                return None;
            }
            if line.trim().is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length = value.trim().parse::<usize>().ok();
            }
        }
        let mut bytes = vec![0; length.filter(|length| *length <= 8 * 1024 * 1024)?];
        stdout.read_exact(&mut bytes).await.ok()?;
        let message: Value = serde_json::from_slice(&bytes).ok()?;
        if message.get("method").is_some() && let Some(request) = message.get("id") {
            let result = if message["method"] == "workspace/configuration" {
                Value::Array(vec![Value::Null; message["params"]["items"].as_array().map_or(0, Vec::len)])
            } else {
                Value::Null
            };
            send(stdin, json!({ "jsonrpc": "2.0", "id": request, "result": result })).await?;
        } else if message["id"] == id {
            return Some(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}

pub async fn expand(root: &Path, file: &Path, source: &str, offset: usize) -> Option<Value> {
    let prefix = source.get(..offset)?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let column = prefix.rsplit('\n').next()?.encode_utf16().count();
    let (binary, _) = tokio::task::spawn_blocking(nodex_lang_rust::semantic::find_binary).await.ok()??;
    let uri = file_uri(file);
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut command = Command::from(nodex_lang_api::process::command(binary));
        let mut child = command.current_dir(root).stdin(Stdio::piped()).stdout(Stdio::piped())
            .stderr(Stdio::null()).kill_on_drop(true).spawn().ok()?;
        let mut stdin = child.stdin.take()?;
        let mut stdout = BufReader::new(child.stdout.take()?);
        send(&mut stdin, json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "rootUri": file_uri(root), "capabilities": {},
            "initializationOptions": { "checkOnSave": false }
        }})).await?;
        response(&mut stdout, &mut stdin, 1).await?;
        send(&mut stdin, json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} })).await?;
        send(&mut stdin, json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": { "uri": uri, "languageId": "rust", "version": 1, "text": source }
        }})).await?;
        for id in 2..42 {
            send(&mut stdin, json!({ "jsonrpc": "2.0", "id": id, "method": "rust-analyzer/expandMacro", "params": {
                "textDocument": { "uri": uri }, "position": { "line": line, "character": column }
            }})).await?;
            let result = response(&mut stdout, &mut stdin, id).await?;
            if result["expansion"].is_string() {
                return Some(result);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        None
    }).await.ok().flatten()
}
