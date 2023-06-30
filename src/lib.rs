#![cfg_attr(not(test), no_std)]

use core::cell::RefCell;
use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use critical_section::Mutex;

struct Ticker {
    ticks: u32,
}

impl Ticker {
    // Return next tick every time it is checked
    fn get_ticks(&mut self) -> u32 {
        let ticks = self.ticks;
        self.ticks += 1;
        ticks
    }
}

struct GlobalTimer {
    ticker: Mutex<RefCell<Ticker>>,
}

impl GlobalTimer {
    pub const fn new() -> Self {
        Self {
            ticker: Mutex::new(RefCell::new(Ticker { ticks: 0 })),
        }
    }

    pub fn get_ticks(&self) -> u32 {
        critical_section::with(|cs| self.ticker.borrow_ref_mut(cs).get_ticks())
    }
}

static GLOBAL_TIMER: GlobalTimer = GlobalTimer::new();

pub struct Executor {}

impl Executor {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn sleep(duration: u32) -> Sleep {
        Sleep {
            wake_at_tick: GLOBAL_TIMER.get_ticks() + duration,
        }
    }

    pub fn run_until<F: Future>(&mut self, f: F) -> F::Output {
        let raw_waker = RawWaker::new(core::ptr::null(), &SIMPLE_WAKER_VTABLE);
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

// Sleeping future. For now sleeps until manually woken up.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Sleep {
    wake_at_tick: u32,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if GLOBAL_TIMER.get_ticks() >= self.wake_at_tick {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

// struct SimpleWaker {}

const SIMPLE_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);

// static WAKER: SimpleWaker = SimpleWaker {};

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

    async fn waiting_task() -> i32 {
        for _ in 0..10 {
            Executor::sleep(10).await;
        }

        42
    }

    #[test]
    fn sleep_and_wake() {
        let mut executor = Executor::default();
        let result = executor.run_until(waiting_task());
        assert_eq!(result, 42);
    }
}
