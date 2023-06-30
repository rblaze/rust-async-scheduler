use core::future::Future;
use core::task::{Context, Poll};

use crate::timer::get_ticks;

// Sleeping future. For now sleeps until manually woken up.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[must_use = "futures do nothing unless polled"]
pub struct Sleep {
    wake_at_tick: u32,
}

impl Sleep {
    pub fn new(wake_at_tick: u32) -> Self {
        Sleep { wake_at_tick }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if get_ticks() >= self.wake_at_tick {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use crate::executor::Executor;

    #[test]
    fn sleep_and_wake() {
        let result = Executor::block_on(async {
            for _ in 0..10 {
                Executor::sleep(10).await;
            }

            42
        });
        assert_eq!(result, 42);
    }
}
