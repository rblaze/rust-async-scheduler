#![deny(unsafe_code)]

use core::future::Future;
use core::task::{Context, Poll};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[must_use = "futures do nothing unless polled"]
pub struct Sleep {
    wake_at_tick: u32,
}

impl Sleep {
    pub(crate) fn new(wake_at_tick: u32) -> Self {
        Self { wake_at_tick }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        crate::executor::check_sleep(self.wake_at_tick)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::block_on;

    #[test]
    fn sleep_and_wake() {
        let v = block_on(async {
            for _ in 0..10 {
                println!("iter enter");
                crate::executor::sleep(10).await;
                println!("iter exit");
            }

            42
        });

        assert_eq!(v, 42);
    }
}
