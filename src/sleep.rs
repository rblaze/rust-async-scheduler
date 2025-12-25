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
    use std::pin::pin;

    use futures::task::LocalFutureObj;

    use crate::executor::LocalExecutor;
    use crate::test_utils::TestEnvironment;
    use crate::time::{Duration, Instant};

    #[test]
    fn sleep_and_wake() {
        let env = TestEnvironment::new();
        let mut f = pin!(async {
            let mut start_tick = Instant::new(0);

            for _ in 0..10 {
                crate::sleep(Duration::new(10)).await;

                let now = env.current_tick();
                println!("sleep enter {}, exit {}", start_tick, now);
                assert!(now - start_tick >= Duration::new(10));
                start_tick = now;
            }
        });

        let fo = LocalFutureObj::new(&mut f);

        LocalExecutor::new(&env).run([fo]);
    }
}
