use axum::extract::ws::WebSocket;
use tokio::sync::{broadcast, mpsc};

pub async fn gateway(
    mut results: broadcast::Receiver<u8>,
    mut publications: broadcast::Receiver<u16>,
    mut other: mpsc::Receiver<u8>,
    mut socket: WebSocket,
    enabled: bool,
) {
    use tokio::select as choose;
    choose! {
        biased;
        result = results.recv() => match result {
            Ok(value) => { std::hint::black_box(value); }
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return,
        },
        publication = /* conexão */ (publications.recv()), if enabled => {
            std::hint::black_box(publication).ok();
        }
        _ = other.recv() => {}
        _ = socket.recv() => {}
    }
}

pub async fn awaited(rx: &mut broadcast::Receiver<u8>) -> Result<u8, broadcast::error::RecvError> {
    (broadcast::Receiver::recv(rx)).await
}

pub async fn pending(rx: &mut broadcast::Receiver<u8>) {
    let future = rx.recv();
    drop(future);
}

pub async fn local_macro(rx: &mut broadcast::Receiver<u8>) {
    macro_rules! select { ($value:expr) => { let _ = $value; }; }
    select!(rx.recv());
}

#[cfg(test)]
mod tests {
    use super::*;
    use broadcast::error::{RecvError, TryRecvError};

    #[tokio::test]
    async fn receive_reports_lag_then_drains_before_closed() {
        let (sender, mut rx) = broadcast::channel(2);
        for value in [10, 20, 30] {
            sender.send(value).unwrap();
        }
        drop(sender);
        assert_eq!(rx.recv().await, Err(RecvError::Lagged(1)));
        assert_eq!(rx.recv().await, Ok(20));
        assert_eq!(rx.recv().await, Ok(30));
        assert_eq!(rx.recv().await, Err(RecvError::Closed));
    }

    #[tokio::test]
    async fn dropping_a_pending_select_receive_consumes_no_message() {
        let (sender, mut rx) = broadcast::channel(2);
        tokio::select! {
            biased;
            _ = rx.recv() => panic!("receive must be pending"),
            _ = std::future::ready(()) => {}
        }
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        sender.send(42).unwrap();
        assert_eq!(rx.recv().await, Ok(42));
    }

    #[tokio::test]
    async fn a_ready_receive_can_consume_before_its_pattern_fails() {
        let (sender, mut rx) = broadcast::channel(2);
        sender.send(1).unwrap();
        tokio::select! {
            biased;
            Ok(2) = rx.recv() => panic!("pattern must fail"),
            _ = std::future::ready(()) => {}
        }
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }
}
