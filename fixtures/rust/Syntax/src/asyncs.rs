use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// A tiny executor, so async code runs without dependencies.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

fn noop_waker() -> Waker {
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn noop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

pub async fn fetch(id: u32) -> u32 {
    id * 10
}

pub async fn total(ids: Vec<u32>) -> u32 {
    let mut sum = 0;
    for id in ids {
        sum += fetch(id).await;
    }
    Ready(0).await;
    let extra = async { 1 }.await;
    sum + extra
}

pub fn run() -> u32 {
    let future = async move {
        let a = fetch(1).await;
        a + 1
    };
    block_on(future) + block_on(total(vec![1, 2]))
}

pub struct Ready(pub u32);

impl Future for Ready {
    type Output = u32;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u32> {
        Poll::Ready(self.0)
    }
}
