use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use tokio::{sync::mpsc::UnboundedReceiver, time::Instant};

const JOIN_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn connection(mut socket: WebSocket) {
    let first = match tokio::time::timeout(JOIN_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Binary(bytes)))) => bytes,
        _ => return,
    };
    drop(first);
}

pub async fn durable(receiver: &mut UnboundedReceiver<u32>, deadline: Instant) -> Option<u32> {
    match tokio::time::timeout_at(deadline, receiver.recv()).await {
        Ok(Some(message)) => Some(message),
        Ok(None) | Err(_) => None,
    }
}

pub async fn parenthesized(deadline: Instant) -> Result<Result<(), u32>, tokio::time::error::Elapsed> {
    use tokio::time::timeout_at as by;
    (/* preserve the direct await */ by(deadline, async { Err(5) })).await
}

pub async fn queued() {
    let pending = tokio::time::timeout(JOIN_TIMEOUT, async { 5 });
    drop(pending);
}

pub struct LocalSocket;

impl LocalSocket {
    async fn recv(&mut self) -> Option<Result<u32, ()>> { None }
}

pub async fn local_receive(mut socket: LocalSocket) {
    let _ = tokio::time::timeout(JOIN_TIMEOUT, socket.recv()).await;
}

pub async fn local_timeout(mut socket: WebSocket) {
    async fn timeout<F: Future>(_: Duration, future: F) -> F::Output { future.await }
    let _ = timeout(JOIN_TIMEOUT, socket.recv()).await;
}
