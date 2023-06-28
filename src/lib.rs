#![cfg_attr(not(test), no_std)]

use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub struct Executor {}

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn block_on<F: Future>(f: F) -> F::Output {
        let raw_waker = RawWaker::new(core::ptr::null(), &DUMMY_VTABLE);
        let waker = unsafe { Waker::from_raw(raw_waker) };
        let mut context = Context::from_waker(&waker);
        let mut pinned_f = pin!(f);

        loop {
            match pinned_f.as_mut().poll(&mut context) {
                Poll::Ready(result) => return result,
                Poll::Pending => {}
            }
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

// Dummy waker that never expects to be called
const DUMMY_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);

#[cfg(test)]
mod tests {
    use super::*;

    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    async fn add3(a: i32, b: i32, c: i32) -> i32 {
        let a_plus_b = add(a, b).await;
        a_plus_b + c
    }

    #[test]
    fn wait_for_single_coroutine() {
        let result = Executor::block_on(add3(1, 2, 3));
        assert_eq!(result, 6);
    }
}
