use core::sync::atomic::{AtomicU32, Ordering};
use core::task::{RawWaker, RawWakerVTable};

pub enum WakerError {
    AlreadyBoundToExecutor,
    TaskIndexOutOfRange,
}

// Problem here: Waker can be cloned and outlive the task after it is
// deleted from the executor list.
// TODO: refcount Wakers and only delete the task when count drops to zero.
pub struct WakerInfo {
    task_mask: u32,
    executor_ready_mask: *const AtomicU32,
}

impl WakerInfo {
    pub const fn new() -> Self {
        Self {
            task_mask: 0,
            executor_ready_mask: core::ptr::null(),
        }
    }

    pub const fn to_raw_waker(&self) -> RawWaker {
        let ptr = self as *const Self as *const ();
        RawWaker::new(ptr, &WAKER_VTABLE)
    }

    pub fn bind_to_executor(
        &mut self,
        task_idx: usize,
        executor_ready_mask: &AtomicU32,
    ) -> Result<(), WakerError> {
        if self.task_mask != 0 || !self.executor_ready_mask.is_null() {
            return Err(WakerError::AlreadyBoundToExecutor);
        }
        if task_idx >= 32 {
            return Err(WakerError::TaskIndexOutOfRange);
        }
        self.task_mask = 1 << task_idx;
        self.executor_ready_mask = executor_ready_mask as *const AtomicU32;
        Ok(())
    }

    pub fn is_task_ready_to_run(&self, executor_ready_mask: &AtomicU32) -> bool {
        debug_assert!(self.task_mask != 0);
        debug_assert!(self.executor_ready_mask == executor_ready_mask as *const AtomicU32);

        let prev_mask = executor_ready_mask.fetch_and(!self.task_mask, Ordering::AcqRel);
        prev_mask & self.task_mask != 0
    }

    pub unsafe fn wake_task(&self) {
        debug_assert!(self.task_mask != 0);
        debug_assert!(!self.executor_ready_mask.is_null());

        let executor_ready_mask = &*self.executor_ready_mask;
        executor_ready_mask.fetch_or(self.task_mask, Ordering::Release);
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

        let mut waker = WakerInfo::new();
        assert!(waker.bind_to_executor(task_idx, &mask).is_ok());
        assert_eq!(mask.load(Ordering::Acquire), default_mask);

        // Task isn't automatically ready after binding
        assert!(!waker.is_task_ready_to_run(&mask));
        assert_eq!(mask.load(Ordering::Acquire), default_mask);

        // Task is ready to run after waking it
        unsafe { waker.wake_task() };
        assert_eq!(mask.load(Ordering::Acquire), default_mask | (1 << task_idx));

        assert!(waker.is_task_ready_to_run(&mask));
        // Task is removed from the mask after check.
        assert_eq!(mask.load(Ordering::Acquire), default_mask);
    }
}
