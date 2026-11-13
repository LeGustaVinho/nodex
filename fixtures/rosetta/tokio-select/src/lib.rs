use std::future::Future;
use tokio::time::{Instant, sleep_until};

pub async fn pilot<F, G>(mut socket_next: impl FnMut() -> F, mut send_tick: impl FnMut() -> G, deadline: Instant)
where
    F: Future<Output = Option<u32>>,
    G: Future<Output = ()>,
{
    loop {
        let next = tokio::select! {
            biased;
            next = socket_next() => next,
            _ = send_tick() => continue,
            _ = sleep_until(deadline) => break,
        };
        if next.is_none() {
            break;
        }
    }
}

pub async fn complex_handlers() {
    'outer: loop {
        tokio::select! {
            _ = std::future::ready(()) => { loop { break; } }
            _ = std::future::ready(()) => break 'outer,
            else => break
        }
    }
}

macro_rules! select {
    ($($tokens:tt)*) => { () };
}

pub fn local_select() {
    loop {
        select! { _ = wait() => continue, };
        break;
    }
}

pub async fn durable(mut receiver: tokio::sync::mpsc::Receiver<u32>) {
    while let Some(first) = receiver.recv().await {
        std::hint::black_box(first);
    }
}
