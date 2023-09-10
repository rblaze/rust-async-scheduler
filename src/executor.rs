use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicU32;
use core::task::{Context, Poll, Waker};
use futures::task::FutureObj;

use crate::sleep::Sleep;
use crate::timer::get_ticks;
use crate::waker::{WakerError, WakerInfo};

// TODO implement sleep
// Таймер так или иначе будет будить процессор. Можно не морочиться с прерываниями,
// а обрабатывать очередь таймера в основном цикле.
// Тривиальное решение: добавить в каждый таск поле "ready at", будить по нему.
// Проблема: что делать если в таске несколько sleep, склеенных через join?
// Решение: они могут обновлять поле ready at, ставить своё время туда если оно меньше,
// чем у нынешнего кандидата. Future должно будет хранить собственное время и ссылку
// на таск.
// Недостаток: ссылку скорее всего не воткнёшь, потому что нет никакой гарантии, что
// она не переживёт Task. Хотя можно попробовать станцевать с 'a, может и прокатит...
// Можно воткнуть указатель, но это unsafe опять :( И так уже многовато в waker.

extern "Rust" {
    // If mask is zero, pause the CPU until interrupt or some other wakeup event.
    // Doing nothing is okay as long as busy-looping is acceptable.
    // Waiting until flag is set is okay too.
    fn _sleep_if_zero(mask: &AtomicU32);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpawnError {
    OutOfSpace,
    Waker(WakerError),
}

struct TaskInfo<'a> {
    future: FutureObj<'a, ()>,
    waker: WakerInfo,
}

pub struct Executor<'a, const N: usize = 1> {
    // Space for tasks to run.
    tasks: [Option<TaskInfo<'a>>; N],
    // Bitmask of the tasks than need to be polled.
    ready_mask: AtomicU32,
}

impl<'a, const N: usize> Executor<'a, N> {
    pub fn new() -> Self {
        Self {
            tasks: core::array::from_fn(|_| None),
            ready_mask: AtomicU32::new(0),
        }
    }

    pub fn spawn(&mut self, task: FutureObj<'a, ()>) -> Result<(), SpawnError> {
        let (idx, free_cell) = self
            .tasks
            .iter_mut()
            .enumerate()
            .find(|(_, v)| v.is_none())
            .ok_or(SpawnError::OutOfSpace)?;

        *free_cell = Some(TaskInfo {
            future: task,
            waker: WakerInfo::new(idx, &self.ready_mask).map_err(SpawnError::Waker)?,
        });

        Ok(())
    }

    // Run all tasks in the pool to completion.
    pub fn run(&mut self) {
        while self.run_once() {
            unsafe {
                _sleep_if_zero(&self.ready_mask);
            }
        }
    }

    // Polls all tasks once
    fn run_once(&mut self) -> bool {
        let mut any_tasks_left = false;

        for cell in &mut self.tasks {
            if let Some(task) = cell {
                if task.waker.is_task_runnable() {
                    let waker = unsafe { Waker::from_raw(task.waker.to_raw_waker()) };
                    let mut context = Context::from_waker(&waker);

                    let result = Pin::new(&mut task.future).poll(&mut context);
                    if let Poll::Ready(()) = result {
                        *cell = None;
                    } else {
                        any_tasks_left = true;
                    }
                }
            }
        }

        any_tasks_left
    }

    pub fn sleep(duration: u32) -> Sleep {
        Sleep::new(get_ticks() + duration)
    }
}

impl<'a, const N: usize> Default for Executor<'a, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
const SIMPLE_VTABLE: core::task::RawWakerVTable = core::task::RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);

#[cfg(test)]
pub fn block_on<F: Future>(f: F) -> F::Output {
    use core::task::RawWaker;

    let raw_waker = RawWaker::new(core::ptr::null(), &SIMPLE_VTABLE);
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut context = Context::from_waker(&waker);
    let mut pinned_f = core::pin::pin!(f);

    loop {
        match pinned_f.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use core::pin::pin;
    use core::sync::atomic::Ordering;

    use super::*;
    use futures::{join, task::FutureObj};

    #[no_mangle]
    fn _sleep_if_zero(mask: &AtomicU32) {
        while mask.load(Ordering::Acquire) == 0 {
            // Spin
        }
    }

    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    async fn add3(a: i32, b: i32, c: i32) -> i32 {
        let a_plus_b = add(a, b).await;
        a_plus_b + c
    }

    #[test]
    fn wait_for_single_coroutine() {
        let result = block_on(add3(1, 2, 3));
        assert_eq!(result, 6);
    }

    #[test]
    fn wait_for_join() {
        let f1 = add(2, 2);
        let f2 = add3(1, 2, 3);
        let result = block_on(async { join!(f1, f2) });
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
            let fo = FutureObj::new(&mut f);

            let mut ex: Executor = Executor::new();
            assert_eq!(ex.spawn(fo), Ok(()));
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
            let fo1 = FutureObj::new(&mut f1);

            let mut f2 = pin!(side_effect_add(3, 4, &mut v2));
            let fo2 = FutureObj::new(&mut f2);

            let mut f3 = pin!(side_effect_add(5, 6, &mut v3));
            let fo3 = FutureObj::new(&mut f3);

            let mut f4 = pin!(side_effect_add(7, 8, &mut v4));
            let fo4 = FutureObj::new(&mut f4);

            let mut ex = Executor::<2>::new();
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
