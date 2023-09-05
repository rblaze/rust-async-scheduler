use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{RawWaker, RawWakerVTable};

// Problem here: Waker can be cloned and outlive the task after it is
// deleted from the executor list.
// TODO: refcount Wakers and only delete the task when count drops to zero.
pub struct WakerInfo {
    task_ready_to_run: AtomicBool,
    executor_wakeup_event: *const AtomicBool,
}

impl WakerInfo {
    pub const fn new() -> Self {
        Self {
            task_ready_to_run: AtomicBool::new(true),
            executor_wakeup_event: core::ptr::null(),
        }
    }

    pub const fn into_raw_waker(&self) -> RawWaker {
        let ptr = self as *const Self as *const ();
        RawWaker::new(ptr, &WAKER_VTABLE)
    }

    pub fn bind_to_executor(&mut self, executor_wakeup_event: &AtomicBool) {
        debug_assert!(self.executor_wakeup_event.is_null());
        self.executor_wakeup_event = executor_wakeup_event as *const _;
    }

    pub fn is_task_ready_to_run(&mut self) -> bool {
        self.task_ready_to_run.swap(false, Ordering::Acquire)
    }

    pub unsafe fn wake_task(&self) {
        debug_assert!(!self.executor_wakeup_event.is_null());

        // Must first set task flag then executor flag.
        // Otherwise, the executor may wake up and go through all the tasks
        // before task is marked as ready to run.
        self.task_ready_to_run.store(true, Ordering::Release);
        let executor_wakeup_event = &*self.executor_wakeup_event;
        executor_wakeup_event.store(true, Ordering::Release);
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
