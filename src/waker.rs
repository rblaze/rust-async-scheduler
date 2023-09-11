use core::num::NonZeroU32;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};
use core::task::{RawWaker, RawWakerVTable};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WakerError {
    TaskIndexOutOfRange,
}

// Problem here: Waker can be cloned and outlive the task after it is
// deleted from the executor list.
// TODO: refcount Wakers and only delete the task when count drops to zero.
pub struct WakerInfo {
    task_mask: NonZeroU32,
    executor_ready_mask: NonNull<AtomicU32>,
}

impl WakerInfo {
    /// Creates new waker and marks task as ready to run.
    pub fn new(task_idx: usize, executor_ready_mask: &AtomicU32) -> Result<Self, WakerError> {
        let task_mask = NonZeroU32::new(1 << task_idx).ok_or(WakerError::TaskIndexOutOfRange)?;

        executor_ready_mask.fetch_or(task_mask.get(), Ordering::Release);

        Ok(Self {
            task_mask,
            executor_ready_mask: NonNull::from(executor_ready_mask),
        })
    }

    pub const fn to_raw_waker(&self) -> RawWaker {
        let ptr = self as *const Self as *const ();
        RawWaker::new(ptr, &WAKER_VTABLE)
    }

    /// Returns true if task is ready to run.
    /// "Ready" bit is cleared after this call.
    pub fn is_task_runnable(&self) -> bool {
        // TODO check that current executor is one bound to task

        let executor_ready_mask = unsafe { self.executor_ready_mask.as_ref() };
        let prev_mask = executor_ready_mask.fetch_and(!self.task_mask.get(), Ordering::AcqRel);
        prev_mask & self.task_mask.get() != 0
    }

    pub unsafe fn wake_task(&self) {
        let executor_ready_mask = self.executor_ready_mask.as_ref();
        executor_ready_mask.fetch_or(self.task_mask.get(), Ordering::Release);
    }
}

const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |ptr| RawWaker::new(ptr, &WAKER_VTABLE),
    |ptr| unsafe {
        debug_assert!(!ptr.is_null());
        let waker = &*(ptr as *const WakerInfo);

        waker.wake_task();
    },
    |ptr| unsafe {
        debug_assert!(!ptr.is_null());
        let waker = &*(ptr as *const WakerInfo);

        waker.wake_task();
    },
    |_| {},
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waker() {
        let default_mask = 1 | (1 << 2) | (1 << 7);
        let mask = AtomicU32::new(default_mask);
        let task_idx = 5;

        let waker = WakerInfo::new(task_idx, &mask).expect("Waker creation failed");
        assert_eq!(
            mask.load(Ordering::Acquire),
            default_mask | (1 << task_idx),
            "Runnable bit for task must be set after new()"
        );

        assert!(
            waker.is_task_runnable(),
            "Task must be runnable when created"
        );
        assert_eq!(
            mask.load(Ordering::Acquire),
            default_mask,
            "Runnable bit must be cleared after is_task_runnable()"
        );
        assert!(
            !waker.is_task_runnable(),
            "Task must not be runnable until wake_task()"
        );

        unsafe { waker.wake_task() };
        assert_eq!(
            mask.load(Ordering::Acquire),
            default_mask | (1 << task_idx),
            "Runnable bit must be set after wake_task()"
        );

        assert!(
            waker.is_task_runnable(),
            "Task must be runnable after wake_task()"
        );
    }
}
