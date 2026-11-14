use tokio::sync::{broadcast, mpsc};

pub fn subscribe(sender: &broadcast::Sender<u32>) -> broadcast::Receiver<u32> {
    sender.subscribe()
}

pub fn publish(sender: &broadcast::Sender<String>, message: String) {
    let _ = broadcast::Sender::send(sender, message);
}

pub fn custom(sender: &mpsc::UnboundedSender<String>, message: String) {
    let _ = sender.send(message);
}

#[test]
fn subscribers_start_at_the_next_send_and_send_can_return_the_value() {
    let (sender, mut original) = broadcast::channel(4);
    assert_eq!(sender.send(10).unwrap(), 1);
    let mut new = subscribe(&sender);
    assert_eq!(new.try_recv(), Err(broadcast::error::TryRecvError::Empty));
    assert_eq!(sender.send(20).unwrap(), 2);
    assert_eq!(original.try_recv(), Ok(10));
    assert_eq!(original.try_recv(), Ok(20));
    assert_eq!(new.try_recv(), Ok(20));
    drop(original);
    drop(new);
    assert_eq!(sender.send(30).unwrap_err().0, 30);
}
