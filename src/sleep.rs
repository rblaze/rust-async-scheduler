#![deny(unsafe_code)]

use core::future::Future;
use core::task::{Context, Poll};

use crate::time::Instant;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[must_use = "futures do nothing unless polled"]
pub struct Sleep {
    wake_at_tick: Instant,
}

impl Sleep {
    pub(crate) fn new(wake_at_tick: Instant) -> Self {
        Self { wake_at_tick }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let info = crate::waker::from_waker(cx.waker());
        info.executor()
            .wakeup_task_at(info.task_index(), self.wake_at_tick)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::block_on;
    use crate::time::Duration;

    #[test]
    fn sleep_and_wake() {
        let v = block_on(async {
            for _ in 0..10 {
                println!("iter enter");
                crate::sleep(Duration::new(10)).await;
                println!("iter exit");
            }

            42
        });

        assert_eq!(v, 42);
    }
}
