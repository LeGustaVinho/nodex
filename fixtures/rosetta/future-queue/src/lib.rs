use std::future::Future;
use std::pin::Pin;

use futures_util::stream::{FuturesOrdered, StreamExt};

pub type Reply = Pin<Box<dyn Future<Output = u8>>>;

pub async fn exchange(first: Reply, second: Reply) -> Option<u8> {
    let mut replies = FuturesOrdered::new();
    replies.push_back(first);
    FuturesOrdered::push_back(&mut replies, second);
    replies.next().await
}

pub async fn gateway(next_message: impl Future<Output = Option<u8>>) -> Option<u8> {
    let mut replies = FuturesOrdered::<Reply>::new();
    tokio::select! {
        biased;
        Some(reply) = replies.next(), if !replies.is_empty() => return Some(reply),
        incoming = next_message => match incoming {
            Some(message) => {
                replies.push_back(if message > 2 {
                    Box::pin(std::future::ready(message))
                } else {
                    Box::pin(async { 0 })
                });
                let mut other = std::collections::VecDeque::new();
                other.push_back(4);
            }
            None => return None,
        },
    }
    replies.next().await
}

pub async fn nested(next_message: impl Future<Output = Option<u8>>) {
    let mut replies = FuturesOrdered::new();
    tokio::select! {
        incoming = next_message => {
            let work = || { replies.push_back(std::future::ready(incoming)); };
            let _ = std::hint::black_box(work);
        }
    }
}

pub struct Local;

impl Local {
    fn push_back(&mut self, _value: u8) {}
}

pub fn homonyms(mut queue: Local, mut values: std::collections::VecDeque<u8>) {
    queue.push_back(3);
    values.push_back(4);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::future::poll_fn;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn insertion_and_fifo_polling_happen_at_different_times() {
        let mut replies = FuturesOrdered::<Reply>::new();
        let first_ready = Rc::new(Cell::new(false));
        let first_waker = Rc::new(Cell::new(None::<Waker>));
        let ready = first_ready.clone();
        let waker = first_waker.clone();
        replies.push_back(Box::pin(poll_fn(move |cx| {
            if ready.get() {
                Poll::Ready(1)
            } else {
                waker.set(Some(cx.waker().clone()));
                Poll::Pending
            }
        })));

        let second_polled = Rc::new(Cell::new(false));
        let polled = second_polled.clone();
        replies.push_back(Box::pin(async move {
            polled.set(true);
            2
        }));
        assert!(!second_polled.get());
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(Pin::new(&mut replies.next()).poll(&mut cx), Poll::Pending);
        assert!(second_polled.get());

        first_ready.set(true);
        first_waker.take().unwrap().wake();
        assert_eq!(
            Pin::new(&mut replies.next()).poll(&mut cx),
            Poll::Ready(Some(1))
        );
        assert_eq!(
            Pin::new(&mut replies.next()).poll(&mut cx),
            Poll::Ready(Some(2))
        );
    }
}
