use core::cell::{Cell, OnceCell};
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicU32;
use core::task::{Context, Poll, Waker};
use critical_section::Mutex;
use futures::task::LocalFutureObj;

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
    OutOfSpace,
    Waker(WakerError),
}

struct TaskInfo<'a> {
    future: LocalFutureObj<'a, ()>,
    waker: WakerInfo,
    sleep_until: Cell<Option<u32>>,
}

pub struct LocalExecutor<'a, const N: usize = 1> {
    // Space for tasks to run.
    tasks: [Option<TaskInfo<'a>>; N],
    // Bitmask of the tasks than need to be polled.
    ready_mask: AtomicU32,
}

impl<'a, const N: usize> Default for LocalExecutor<'a, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, const N: usize> LocalExecutor<'a, N> {
    pub fn new() -> Self {
        Self {
            tasks: core::array::from_fn(|_| None),
            ready_mask: AtomicU32::new(0),
        }
    }

    pub fn spawn(&mut self, task: LocalFutureObj<'a, ()>) -> Result<(), SpawnError> {
        let (idx, free_cell) = self
            .tasks
            .iter_mut()
            .enumerate()
            .find(|(_, v)| v.is_none())
            .ok_or(SpawnError::OutOfSpace)?;

        *free_cell = Some(TaskInfo {
            future: task,
            waker: WakerInfo::new(idx, &self.ready_mask).map_err(SpawnError::Waker)?,
            sleep_until: Cell::new(None),
        });

        Ok(())
    }

    // Run all tasks in the pool to completion.
    pub fn run(&mut self) {
        while self.run_once() {
            environment().sleep_if_zero(&self.ready_mask);
        }
    }

    // Polls all tasks once
    fn run_once(&mut self) -> bool {
        let mut any_tasks_left = false;

        let _enter = Enter::new(self);
        for cell in &mut self.tasks {
            if let Some(task) = cell {
                if task.waker.is_task_runnable()
                    || task
                        .sleep_until
                        .get()
                        .is_some_and(|ticks| ticks <= environment().ticks())
                {
                    let waker = unsafe { Waker::from_raw(task.waker.to_raw_waker()) };
                    let mut context = Context::from_waker(&waker);

                    let result = Pin::new(&mut task.future).poll(&mut context);
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

impl<'a, const N: usize> Executor for LocalExecutor<'a, N> {
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

            let mut ex: LocalExecutor = LocalExecutor::new();
            assert_eq!(ex.spawn(fo), Ok(()));
            ex.run();
        }
        // Check that task was actually run
        assert_eq!(v, 3);
    }

    #[test]
    fn test_spawn_overflow() {
        setup();
        let mut v1 = 0;
        let mut v2 = 0;
        let mut v3 = 0;
        let mut v4 = 0;
        {
            let mut f1 = pin!(side_effect_add(1, 2, &mut v1));
            let fo1 = LocalFutureObj::new(&mut f1);

            let mut f2 = pin!(side_effect_add(3, 4, &mut v2));
            let fo2 = LocalFutureObj::new(&mut f2);

            let mut f3 = pin!(side_effect_add(5, 6, &mut v3));
            let fo3 = LocalFutureObj::new(&mut f3);

            let mut f4 = pin!(side_effect_add(7, 8, &mut v4));
            let fo4 = LocalFutureObj::new(&mut f4);

            let mut ex = LocalExecutor::<2>::new();
            assert_eq!(ex.spawn(fo1), Ok(()));
            assert_eq!(ex.spawn(fo2), Ok(()));
            // No more free cells
            assert_eq!(ex.spawn(fo3), Err(SpawnError::OutOfSpace));
            ex.run();
            // Now we can spawn again
            assert_eq!(ex.spawn(fo4), Ok(()));
            ex.run();
        }
        // Check that task was actually run
        assert_eq!(v1, 3);
        assert_eq!(v2, 7);
        // This task shouldn't be run
        assert_eq!(v3, 0);
        // And this task should
        assert_eq!(v4, 15);
    }
}
