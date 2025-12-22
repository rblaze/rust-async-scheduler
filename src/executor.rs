use core::cell::Cell;
use core::sync::atomic::Ordering;
use core::task::{Context, Poll, Waker};

use futures::FutureExt;
use futures::task::LocalFutureObj;
use portable_atomic::AtomicBool;

use crate::time::Instant;
use crate::waker::WakerInfo;

pub trait Environment: core::fmt::Debug {
    /// Sleeps until `event` becomes true or `tick` is reached.
    /// Early return is okay but causes performance overhead.
    fn wait_for_event_with_deadline(&self, event: &AtomicBool, tick: Option<Instant>);
    /// Gets current tick count.
    fn ticks(&self) -> Instant;
}

pub(crate) trait Executor: core::fmt::Debug {
    /// Returns current time
    fn current_time(&self) -> Instant;
    /// Run task at specified time
    fn wakeup_task_at(&self, task_index: usize, time: Instant) -> Poll<()>;
    /// Mark task as ready to run
    fn set_task_runnable(&self, task_index: usize);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum TaskState {
    Runnable,
    Waiting(Option<Instant>),
}

impl TaskState {
    fn is_runnable(&self, tick: Instant) -> bool {
        match self {
            TaskState::Runnable => true,
            TaskState::Waiting(Some(expected_tick)) => *expected_tick <= tick,
            _ => false,
        }
    }
}

#[derive(Debug)]
struct TaskInfo {
    waker: WakerInfo,
    state: Cell<TaskState>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RunResult {
    /// Run next iteration immediately
    RunAgain,
    /// Wait for wakeup event or specified time
    WaitForTick(Instant),
    /// No active timeouts, wait for wakeup event
    WaitForEvent,
    /// No pending tasks left
    NoMoreTasks,
}

impl RunResult {
    fn from_task_state(state: Option<TaskState>) -> Self {
        match state {
            Some(TaskState::Runnable) => RunResult::RunAgain,
            Some(TaskState::Waiting(Some(tick))) => RunResult::WaitForTick(tick),
            Some(TaskState::Waiting(None)) => RunResult::WaitForEvent,
            None => RunResult::NoMoreTasks,
        }
    }
}

#[derive(Debug)]
pub struct LocalExecutor<'a, const N: usize> {
    env: &'a dyn Environment,
    // Space for tasks to run.
    tasks: [Option<TaskInfo>; N],
    wakeup_event: AtomicBool,
}

impl<'a, const N: usize> LocalExecutor<'a, N> {
    pub fn new(env: &'a dyn Environment) -> Self {
        Self {
            env,
            tasks: [const { None }; N],
            wakeup_event: AtomicBool::new(false),
        }
    }

    // Run all futures to completion.
    pub fn run(&'a mut self, mut futures: [LocalFutureObj<'_, ()>; N]) {
        for index in 0..N {
            self.tasks[index] = Some(TaskInfo {
                waker: WakerInfo::new(index, self as *const _ as *const _),
                state: Cell::new(TaskState::Runnable),
            });
        }

        loop {
            match self.run_once(&mut futures) {
                RunResult::RunAgain => continue,
                RunResult::WaitForTick(tick) => self
                    .env
                    .wait_for_event_with_deadline(&self.wakeup_event, Some(tick)),
                RunResult::WaitForEvent => self
                    .env
                    .wait_for_event_with_deadline(&self.wakeup_event, None),
                RunResult::NoMoreTasks => break,
            }
        }
    }

    // Polls all tasks once
    fn run_once(&mut self, futures: &mut [LocalFutureObj<'_, ()>; N]) -> RunResult {
        // Clear wakeup flag, it already activated this loop.
        self.wakeup_event.store(false, Ordering::Release);

        futures
            .iter_mut()
            .enumerate()
            .map(|(task_index, future)| self.run_task(task_index, future))
            .min()
            .unwrap_or(RunResult::NoMoreTasks)
    }

    fn run_task(&mut self, task_index: usize, future: &mut LocalFutureObj<'_, ()>) -> RunResult {
        let t = &mut self.tasks[task_index];

        if let Some(task) = t
            && task.state.get().is_runnable(self.env.ticks())
        {
            let waker = unsafe { Waker::from_raw(task.waker.to_raw_waker()) };
            let mut context = Context::from_waker(&waker);

            // Let sleep and yield futures update this field.
            task.state.set(TaskState::Waiting(None));

            if future.poll_unpin(&mut context).is_ready() {
                // Task finished
                *t = None;
            }
        }

        RunResult::from_task_state(t.as_ref().map(|task| task.state.get()))
    }
}

impl<'a, const N: usize> Executor for LocalExecutor<'a, N> {
    fn current_time(&self) -> Instant {
        self.env.ticks()
    }

    fn wakeup_task_at(&self, task_index: usize, time: Instant) -> Poll<()> {
        debug_assert!(task_index < N);

        if self.env.ticks() <= time {
            Poll::Ready(())
        } else {
            // This function is supposed to be called only for currently running task.
            self.tasks[task_index]
                .as_ref()
                .expect("wakeup_task_at() called for finished task")
                .state
                .update(|state| {
                    // Check if the task is already scheduled to wakeup at earlier time.
                    if state.is_runnable(time) {
                        state
                    } else {
                        TaskState::Waiting(Some(time))
                    }
                });

            Poll::Pending
        }
    }

    fn set_task_runnable(&self, task_index: usize) {
        debug_assert!(task_index < N);
        self.tasks[task_index]
            .as_ref()
            .expect("set_task_runnable() called for finished task")
            .state
            .set(TaskState::Runnable);

        self.wakeup_event.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use core::pin::pin;

    use crate::test_utils::TestEnvironment;

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
        let mut v = 0;
        {
            let env = TestEnvironment::new();
            let mut f = pin!(side_effect_add(1, 2, &mut v));
            let fo = LocalFutureObj::new(&mut f);

            LocalExecutor::new(&env).run([fo]);
        }
        // Check that task was actually run
        assert_eq!(v, 3);
    }
}
