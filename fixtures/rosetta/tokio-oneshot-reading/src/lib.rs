use std::future::Future;
use tokio::sync::oneshot::{Receiver, Sender, channel as reply_channel};

pub fn durable(queue: impl FnOnce(Sender<bool>)) -> impl Future<Output = bool> {
    let (answer, answered) = reply_channel();
    queue(answer);
    async move { answered.await.unwrap_or(false) }
}

pub fn durable_blocking(answered: Receiver<bool>) -> bool {
    answered.blocking_recv().unwrap_or(false)
}

pub fn publish(answer: Sender<bool>) -> Result<(), bool> {
    Sender::send(answer, false)
}

pub async fn unrelated(receiver: std::future::Ready<bool>) -> bool {
    receiver.await
}

pub struct Local;

impl Local {
    fn send(self, value: bool) -> Result<(), bool> { Err(value) }
    fn blocking_recv(self) -> bool { false }
}

pub fn homonyms() {
    let _ = Local.send(false);
    let _ = Local.blocking_recv();
}

#[test]
fn false_is_a_payload_and_an_unsent_reply_is_an_error() {
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let (tx, rx) = reply_channel();
    assert_eq!(publish(tx), Ok(()));
    assert_eq!(runtime.block_on(rx), Ok(false));

    let (tx, rx) = reply_channel::<bool>();
    drop(tx);
    assert!(runtime.block_on(rx).is_err());

    let (tx, rx) = reply_channel();
    drop(rx);
    assert_eq!(tx.send(true), Err(true));

    let (tx, rx) = reply_channel();
    tx.send(true).unwrap();
    assert!(durable_blocking(rx));
}
