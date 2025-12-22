use std::cell::Cell;
use std::default::Default;

use futures::Future;
use portable_atomic::AtomicBool;

use crate::executor::{Environment, LocalExecutor};
use crate::time::{Duration, Instant};

#[derive(Debug)]
pub struct TestEnvironment {
    tick: Cell<Instant>,
}

impl TestEnvironment {
    pub fn new() -> Self {
        Self {
            tick: Cell::new(Instant::new(0)),
        }
    }
}

impl Environment for TestEnvironment {
    fn wait_for_event_with_deadline(&self, _event: &AtomicBool, _tick: Option<Instant>) {
        // No-op to allow timer to tick
    }

    fn ticks(&self) -> Instant {
        let now = self.tick.get();
        self.tick.set(now + Duration::new(1));
        now
    }
}

pub fn block_on<T: Default>(future: impl Future<Output = T>) -> T {
    let mut ret: T = T::default();

    {
        let env = TestEnvironment::new();
        let f = core::pin::pin!(async {
            ret = future.await;
        });
        let fo = futures::task::LocalFutureObj::new(f);

        LocalExecutor::new(&env).run([fo]);
    }

    ret
}
