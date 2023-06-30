use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

use crate::sleep::Sleep;
use crate::timer::get_ticks;
use crate::waker::SimpleWaker;

pub struct Executor {}

impl Executor {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn sleep(duration: u32) -> Sleep {
        Sleep::new(get_ticks() + duration)
    }

    pub fn run_until<F: Future>(&mut self, f: F) -> F::Output {
        let raw_waker = SimpleWaker::new_raw_waker();
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

    pub fn block_on<F: Future>(f: F) -> F::Output {
        Self::default().run_until(f)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::join;

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

    #[test]
    fn wait_for_join() {
        let f1 = add(2, 2);
        let f2 = add3(1, 2, 3);
        let result = Executor::block_on(async { join!(f1, f2) });
        assert_eq!(result, (4, 6));
    }
}
