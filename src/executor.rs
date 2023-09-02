use core::future::Future;
use core::pin::{pin, Pin};
use core::task::{Context, Poll, Waker};
use futures::task::LocalFutureObj;

use crate::sleep::Sleep;
use crate::timer::get_ticks;
use crate::waker::SimpleWaker;

// TODO
// в статические переменные future не запихивается почему-то, надо вводиь lifetimes
// > как это сделано в embassy не понимаю. Попробовать написать пример этих их _spawn_async_fn медленно с нуля?
// >> попробовал, без nightly их код не собирается, примеров без макросов нет. В утиль :(

// >> сделать future -> LocalFutureObj -> Task
// >> изначально таск может быть просто обёрткой
// >> кажется он будет фиксированного размера, класть его в executor в буфер на N тасков
//
// хз что делать с динамическим spawn, может напихать указатели в task

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpawnError {
    AlreadySpawned,
    OutOfSpace,
}

pub struct Executor<'a, const N: usize = 1> {
    tasks: [Option<Task<'a>>; N],
}

impl<'a, const N: usize> Executor<'a, N> {
    pub fn new() -> Self {
        Self {
            tasks: core::array::from_fn(|_| None),
        }
    }

    pub fn spawn(&mut self, task: Task<'a>) -> Result<(), SpawnError> {
        let free_cell = self
            .tasks
            .iter_mut()
            .find(|v| v.is_none())
            .ok_or(SpawnError::OutOfSpace)?;
        *free_cell = Some(task);

        Ok(())
    }

    // Run all tasks in the pool to completion.
    pub fn run(&mut self) {
        while self.run_once() {}
    }

    // Polls all tasks once
    fn run_once(&mut self) -> bool {
        let mut any_tasks_left = false;

        for cell in &mut self.tasks {
            if let Some(task) = cell {
                let result = task.poll_future();
                if let Poll::Ready(()) = result {
                    *cell = None;
                } else {
                    any_tasks_left = true;
                }
            }
        }

        any_tasks_left
    }

    pub fn sleep(duration: u32) -> Sleep {
        Sleep::new(get_ticks() + duration)
    }

    pub fn block_on<F: Future>(f: F) -> F::Output {
        let raw_waker = SimpleWaker::new_raw_waker();
        let waker = unsafe { Waker::from_raw(raw_waker) };
        let mut context = Context::from_waker(&waker);
        let mut pinned_f = pin!(f);

        loop {
            match pinned_f.as_mut().poll(&mut context) {
                Poll::Ready(result) => return result,
                Poll::Pending => {}
            }
        }
    }
}

pub struct Task<'a> {
    future: LocalFutureObj<'a, ()>,
    waker: Waker,
}

impl<'a> Task<'a> {
    pub fn new(future: LocalFutureObj<'a, ()>) -> Self {
        Self {
            future,
            waker: unsafe { Waker::from_raw(SimpleWaker::new_raw_waker()) },
        }
    }

    pub fn poll_future(&mut self) -> Poll<()> {
        let mut context = Context::from_waker(&self.waker);
        Pin::new(&mut self.future).poll(&mut context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{join, task::LocalFutureObj};

    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    async fn add3(a: i32, b: i32, c: i32) -> i32 {
        let a_plus_b = add(a, b).await;
        a_plus_b + c
    }

    #[test]
    fn wait_for_single_coroutine() {
        let result = Executor::<1>::block_on(add3(1, 2, 3));
        assert_eq!(result, 6);
    }

    #[test]
    fn wait_for_join() {
        let f1 = add(2, 2);
        let f2 = add3(1, 2, 3);
        let result = Executor::<3>::block_on(async { join!(f1, f2) });
        assert_eq!(result, (4, 6));
    }

    async fn side_effect_add(a: i32, b: i32, ret: &mut i32) {
        *ret = add(a, b).await;
    }

    #[test]
    fn test_spawn() {
        let mut v = 0;
        {
            let mut f = pin!(side_effect_add(1, 2, &mut v));
            let fo = LocalFutureObj::new(&mut f);
            let t = Task::new(fo);

            let mut ex = Executor::<1>::new();
            assert_eq!(ex.spawn(t), Ok(()));
            ex.run();
        }
        // Check that task was actually run
        assert_eq!(v, 3);
    }

    #[test]
    fn test_spawn_overflow() {
        let mut v1 = 0;
        let mut v2 = 0;
        let mut v3 = 0;
        let mut v4 = 0;
        {
            let mut f1 = pin!(side_effect_add(1, 2, &mut v1));
            let fo1 = LocalFutureObj::new(&mut f1);
            let t1 = Task::new(fo1);

            let mut f2 = pin!(side_effect_add(3, 4, &mut v2));
            let fo2 = LocalFutureObj::new(&mut f2);
            let t2 = Task::new(fo2);

            let mut f3 = pin!(side_effect_add(5, 6, &mut v3));
            let fo3 = LocalFutureObj::new(&mut f3);
            let t3 = Task::new(fo3);

            let mut f4 = pin!(side_effect_add(7, 8, &mut v4));
            let fo4 = LocalFutureObj::new(&mut f4);
            let t4 = Task::new(fo4);

            let mut ex = Executor::<2>::new();
            assert_eq!(ex.spawn(t1), Ok(()));
            assert_eq!(ex.spawn(t2), Ok(()));
            // No more free cells
            assert_eq!(ex.spawn(t3), Err(SpawnError::OutOfSpace));
            ex.run();
            // Now we can spawn again
            assert_eq!(ex.spawn(t4), Ok(()));
            ex.run();
        }
        // Check that task was actually run
        assert_eq!(v1, 3);
        assert_eq!(v2, 7);
        // This task shouldn't be run
        assert_eq!(v3, 0);
        // And this should
        assert_eq!(v4, 15);
    }
}
