use core::cell::Cell;
use std::sync::Once;

use crate::executor::{Environment, LocalExecutor};

pub struct TestEnvironment {}

thread_local! {
    static TICKS: Cell<u32> = Cell::new(0);
    static EXECUTOR: Cell<Option<&'static dyn crate::executor::Executor>> = Cell::new(None);
}

impl Environment for TestEnvironment {
    fn sleep_if_zero(&self, _mask: &core::sync::atomic::AtomicU32) {
        // No-op to allow timer to tick
    }

    fn ticks(&self) -> u32 {
        TICKS.with(|t| {
            let ticks = t.get();
            t.set(ticks + 1);
            ticks
        })
    }

    fn enter_executor(&self, executor: &dyn crate::executor::Executor) {
        EXECUTOR.with(|cell| {
            if cell.get().is_some() {
                panic!("double-entering executor");
            }

            let r = unsafe {
                core::mem::transmute::<
                    &dyn crate::executor::Executor,
                    &'static dyn crate::executor::Executor,
                >(executor)
            };
            cell.set(Some(r));
        });
    }

    fn leave_executor(&self) {
        EXECUTOR.with(|cell| {
            cell.get().expect("leaving executor without entering");
            cell.set(None);
        });
    }

    fn current_executor(&self) -> Option<&dyn crate::executor::Executor> {
        EXECUTOR.with(|cell| cell.get())
    }
}

static TESTENV: TestEnvironment = TestEnvironment {};
static SETUP: Once = Once::new();

pub fn setup() {
    SETUP.call_once(|| {
        let _ = crate::executor::set_environment(&TESTENV);
    });
}

async fn run_and_set<R: Send>(result: &mut R, future: impl futures::Future<Output = R> + Send) {
    *result = future.await;
}

pub fn block_on<R: Send + Default>(future: impl futures::Future<Output = R> + Send) -> R {
    let _ = setup();
    let mut r: R = Default::default();
    {
        let f = core::pin::pin!(run_and_set(&mut r, future));
        let fo = futures::task::FutureObj::new(f);

        let mut ex: LocalExecutor<'_, 1> = LocalExecutor::new();
        assert_eq!(ex.spawn(fo), Ok(()));
        ex.run();
    }
    r
}
