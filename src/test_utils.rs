use core::{cell::Cell, mem::MaybeUninit};
use std::sync::Once;

use futures::Future;

use crate::executor::{Environment, LocalExecutor};
use crate::time::Ticks;

pub struct TestEnvironment {}

thread_local! {
    static TICKS: Cell<Ticks> = Cell::new(Ticks::new(0));
    static EXECUTOR: Cell<Option<&'static dyn crate::executor::Executor>> = Cell::new(None);
}

impl Environment for TestEnvironment {
    fn wait_for_event_with_timeout(
        &self,
        _mask: &portable_atomic::AtomicU32,
        _tick: Option<Ticks>,
    ) {
        // No-op to allow timer to tick
    }

    fn ticks(&self) -> Ticks {
        TICKS.with(|t| {
            let ticks = t.get();
            t.set(ticks + Ticks::new(1));
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

pub async fn assign<T, F: Future<Output = T>>(dest: &mut T, src: F) {
    *dest = src.await;
}

async fn write<T, F: Future<Output = T>>(dest: &mut MaybeUninit<T>, src: F) {
    dest.write(src.await);
}

pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let _ = setup();
    let mut ret: MaybeUninit<T> = MaybeUninit::uninit();
    {
        let f = core::pin::pin!(write(&mut ret, future));
        let fo = futures::task::LocalFutureObj::new(f);

        LocalExecutor::new().run([fo]);
    }
    unsafe { ret.assume_init() }
}
