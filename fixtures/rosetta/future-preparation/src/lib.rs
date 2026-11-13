use std::future::{Future, ready as resolved};
use std::pin::Pin;

type Reply = Pin<Box<dyn Future<Output = u8>>>;

pub fn now(replies: impl FnOnce() -> u8) -> Reply {
    Box::pin(resolved(replies()))
}

pub fn after_durable(durable: impl Future<Output = bool> + 'static, reply: u8) -> Reply {
    Box::pin(async move { if durable.await { reply } else { 0 } })
}

pub fn boxed_value(value: u8) -> Pin<Box<u8>> {
    Box::pin(value)
}

pub mod local {
    pub struct Box;

    impl Box {
        pub fn pin<T>(value: T) -> T {
            value
        }
    }

    pub fn ready(value: u8) -> u8 {
        value
    }
}

pub fn homonyms() -> u8 {
    local::Box::pin(local::ready(3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn evaluation_and_polling_happen_at_different_times() {
        let evaluated = Cell::new(false);
        let mut ready = now(|| {
            evaluated.set(true);
            8
        });
        assert!(evaluated.get());
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(ready.as_mut().poll(&mut cx), Poll::Ready(8));

        let mut pinned = after_durable(std::future::ready(true), 7);
        assert_eq!(pinned.as_mut().poll(&mut cx), Poll::Ready(7));
    }
}
