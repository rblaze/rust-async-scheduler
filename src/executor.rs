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
