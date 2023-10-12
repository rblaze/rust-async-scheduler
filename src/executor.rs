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
    fn request_wakeup(&self, wake_at: u32);
}

pub trait Environment {
    /// Sleeps until `mask` is non-zero or `tick` is reached.
    /// Early return is okay but causes performance overhead.
    fn wait_for_event_with_timeout(&self, mask: &AtomicU32, tick: Option<u32>);
    /// Gets current tick count.
    fn ticks(&self) -> u32;
    /// Records `executor` as current one, to return from `current_executor()`
    fn enter_executor(&self, executor: &dyn Executor);
    /// Unregisters current executor.
    fn leave_executor(&self);
    /// Returns current executor, if any.
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

enum RunResult {
    /// Wait for wakeup event or specified time
    WaitForTick(u32),
    /// No active timeouts, wait for wakeup event
    WaitForEvent,
    /// No pending tasks left
    NoMoreTasks,
}

impl RunResult {
    /// Map enum to tuple for automatic Eq/Ord
    fn tuple(&self) -> (u32, u32) {
        match self {
            RunResult::WaitForTick(tick) => (0, *tick),
            RunResult::WaitForEvent => (1, 0),
            RunResult::NoMoreTasks => (2, 0),
        }
    }
}

pub struct LocalExecutor<const N: usize> {
    // Space for tasks to run.
    tasks: [Option<TaskInfo>; N],
    // Bitmask of tasks than need to be polled.
    ready_mask: AtomicU32,
    current_task: usize,
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
            current_task: usize::MAX,
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

        loop {
            match self.run_once(&mut futures) {
                RunResult::WaitForTick(tick) => {
                    environment().wait_for_event_with_timeout(&self.ready_mask, Some(tick))
                }
                RunResult::WaitForEvent => {
                    environment().wait_for_event_with_timeout(&self.ready_mask, None)
                }
                RunResult::NoMoreTasks => break,
            }
        }
    }

    // Polls all tasks once
    fn run_once(&mut self, futures: &mut [LocalFutureObj<'_, ()>; N]) -> RunResult {
        let _enter = Enter::new(self);
        futures
            .iter_mut()
            .zip(self.tasks.iter_mut())
            .enumerate()
            .map(|(index, (future, cell))| {
                self.current_task = index;
                Self::run_task(future, cell)
            })
            .min_by_key(RunResult::tuple)
            .unwrap_or(RunResult::NoMoreTasks)
    }

    fn run_task(future: &mut LocalFutureObj<'_, ()>, cell: &mut Option<TaskInfo>) -> RunResult {
        if let Some(task) = cell {
            if task.waker.is_task_runnable()
                || task
                    .sleep_until
                    .get()
                    .is_some_and(|ticks| ticks <= environment().ticks())
            {
                let waker = unsafe { Waker::from_raw(task.waker.to_raw_waker()) };
                let mut context = Context::from_waker(&waker);

                // Let sleep futures update this field.
                task.sleep_until.set(None);

                let result = Pin::new(future).poll(&mut context);
                if let Poll::Ready(()) = result {
                    *cell = None;
                }
            }
        }

        if let Some(task) = cell {
            if let Some(tick) = task.sleep_until.get() {
                RunResult::WaitForTick(tick)
            } else {
                RunResult::WaitForEvent
            }
        } else {
            RunResult::NoMoreTasks
        }
    }
}

impl<const N: usize> Executor for LocalExecutor<N> {
    fn sleep(&self, duration: u32) -> Sleep {
        Sleep::new(environment().ticks() + duration)
    }

    fn request_wakeup(&self, wake_at: u32) {
        let task = self.tasks[self.current_task]
            .as_ref()
            .expect("current_task points to finished task");

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

pub(crate) fn check_sleep(wake_at: u32) -> Poll<()> {
    if environment().ticks() >= wake_at {
        Poll::Ready(())
    } else {
        // If wake time is more recent than other sleeps in this task, save it.
        environment()
            .current_executor()
            .expect("check_sleep() called outside of a coroutine")
            .request_wakeup(wake_at);
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
