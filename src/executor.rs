use core::cell::{Cell, OnceCell};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use critical_section::Mutex;
use futures::task::LocalFutureObj;
use portable_atomic::AtomicU32;

use crate::sleep::Sleep;
use crate::waker::{WakerError, WakerInfo};

pub trait Executor {
    fn sleep(&self, duration: u32) -> Sleep;
    fn mark_sleep(&self, wake_at: u32, waker: &Waker);
}

pub trait Environment {
    fn sleep_if_zero(&self, mask: &AtomicU32);
    fn ticks(&self) -> u32;
    fn enter_executor(&self, executor: &dyn Executor);
    fn leave_executor(&self);
    fn current_executor(&self) -> Option<&dyn Executor>;
}

static ENV: Mutex<OnceCell<&'static (dyn Environment + Sync)>> = Mutex::new(OnceCell::new());

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EnvError {
    AlreadyInitialized,
    Uninitialized,
}

/// Sets executor environment.
/// Must be called before calling run() for any executor
pub fn set_environment(env: &'static (dyn Environment + Sync)) -> Result<(), EnvError> {
    critical_section::with(|cs| {
        ENV.borrow(cs)
            .set(env)
            .or(Err(EnvError::AlreadyInitialized))
    })
}

fn environment() -> &'static (dyn Environment + Sync) {
    critical_section::with(|cs| ENV.borrow(cs).get().ok_or(EnvError::Uninitialized).copied())
        .unwrap()
}

/// enter_executor/leave_executor guard
struct Enter {}

impl Enter {
    /// Registers executor with environment and returns guard struct to unregister it.
    pub fn new(executor: &dyn Executor) -> Self {
        environment().enter_executor(executor);
        Self {}
    }
}

impl Drop for Enter {
    fn drop(&mut self) {
        environment().leave_executor();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpawnError {
    Waker(WakerError),
}

struct TaskInfo {
    waker: WakerInfo,
    sleep_until: Cell<Option<u32>>,
}

pub struct LocalExecutor<const N: usize> {
    // Space for tasks to run.
    tasks: [Option<TaskInfo>; N],
    // Bitmask of the tasks than need to be polled.
    ready_mask: AtomicU32,
}

impl<const N: usize> Default for LocalExecutor<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> LocalExecutor<N> {
    pub fn new() -> Self {
        Self {
            tasks: core::array::from_fn(|_| None),
            ready_mask: AtomicU32::new(0),
        }
    }

    // Run all futures to completion.
    pub fn run(&mut self, mut futures: [LocalFutureObj<'_, ()>; N]) {
        for index in 0..N {
            self.tasks[index] = Some(TaskInfo {
                waker: WakerInfo::new(index, &self.ready_mask).unwrap(),
                sleep_until: Cell::new(None),
            });
        }

        while self.run_once(&mut futures) {
            environment().sleep_if_zero(&self.ready_mask);
        }
    }

    // Polls all tasks once
    fn run_once(&mut self, futures: &mut [LocalFutureObj<'_, ()>; N]) -> bool {
        let mut any_tasks_left = false;

        let _enter = Enter::new(self);
        for (future, cell) in futures.iter_mut().zip(self.tasks.iter_mut()) {
            if let Some(task) = cell {
                if task.waker.is_task_runnable()
                    || task
                        .sleep_until
                        .get()
                        .is_some_and(|ticks| ticks <= environment().ticks())
                {
                    let waker = unsafe { Waker::from_raw(task.waker.to_raw_waker()) };
                    let mut context = Context::from_waker(&waker);

                    let result = Pin::new(future).poll(&mut context);
                    if let Poll::Ready(()) = result {
                        *cell = None;
                    }
                }

                if cell.is_some() {
                    any_tasks_left = true;
                }
            }
        }

        any_tasks_left
    }
}

impl<const N: usize> Executor for LocalExecutor<N> {
    fn sleep(&self, duration: u32) -> Sleep {
        Sleep::new(environment().ticks() + duration)
    }

    fn mark_sleep(&self, wake_at: u32, waker: &Waker) {
        let task = self
            .tasks
            .iter()
            .flatten()
            .find(|task| (unsafe { Waker::from_raw(task.waker.to_raw_waker()) }).will_wake(waker))
            .expect("can't find task from waker");

        if task.sleep_until.get().unwrap_or(u32::MAX) > wake_at {
            task.sleep_until.set(Some(wake_at));
        }
    }
}

pub fn sleep(duration: u32) -> Sleep {
    environment()
        .current_executor()
        .expect("sleep() called outside of a coroutine")
        .sleep(duration)
}

pub(crate) fn check_sleep(wake_at: u32, waker: &Waker) -> Poll<()> {
    if environment().ticks() >= wake_at {
        Poll::Ready(())
    } else {
        // If wake time is more recent than other sleeps in this task, save it.
        environment()
            .current_executor()
            .expect("check_sleep() called outside of a coroutine")
            .mark_sleep(wake_at, waker);
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use core::pin::pin;

    use crate::test_utils::setup;

    use super::*;
    use futures::task::LocalFutureObj;

    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    async fn side_effect_add(a: i32, b: i32, ret: &mut i32) {
        *ret = add(a, b).await;
    }

    #[test]
    fn test_spawn() {
        setup();
        let mut v = 0;
        {
            let mut f = pin!(side_effect_add(1, 2, &mut v));
            let fo = LocalFutureObj::new(&mut f);

            LocalExecutor::new().run([fo]);
        }
        // Check that task was actually run
        assert_eq!(v, 3);
    }
}
