use core::cell::Cell;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::executor::Environment;

pub struct TestEnvironment {
    ticks: AtomicU32,
    executor: Cell<Option<&'static dyn crate::executor::Executor>>,
}

// Tests are single-threaded so Sync is okay.
unsafe impl Sync for TestEnvironment {}

impl TestEnvironment {
    pub const fn new() -> Self {
        Self {
            ticks: AtomicU32::new(0),
            executor: Cell::new(None),
        }
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment for TestEnvironment {
    fn sleep_if_zero(&self, mask: &core::sync::atomic::AtomicU32) {
        while mask.load(Ordering::Acquire) == 0 {
            // Spin
        }
    }

    fn ticks(&self) -> u32 {
        self.ticks.fetch_add(1, Ordering::AcqRel)
    }

    fn enter_executor(&self, executor: &dyn crate::executor::Executor) {
        if self.executor.get().is_some() {
            panic!("double-entering executor");
        }

        let r = unsafe {
            core::mem::transmute::<
                &dyn crate::executor::Executor,
                &'static dyn crate::executor::Executor,
            >(executor)
        };
        self.executor.set(Some(r));
    }

    fn leave_executor(&self) {
        self.executor
            .get()
            .expect("leaving executor without entering");
        self.executor.set(None);
    }

    fn current_executor(&self) -> Option<&dyn crate::executor::Executor> {
        self.executor.get()
    }
}
