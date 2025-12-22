use core::task::{RawWaker, RawWakerVTable, Waker};

use crate::executor::Executor;

#[derive(Debug)]
pub(crate) struct WakerInfo {
    task_index: usize,
    executor: *const dyn Executor,
}

impl WakerInfo {
    pub fn new(task_index: usize, executor: *const dyn Executor) -> Self {
        Self {
            task_index,
            executor,
        }
    }

    pub const fn to_raw_waker(&self) -> RawWaker {
        let ptr = self as *const Self as *const ();
        RawWaker::new(ptr, &WAKER_VTABLE)
    }

    pub fn executor(&self) -> &dyn Executor {
        unsafe { self.executor.as_ref().expect("executor is null") }
    }

    pub fn task_index(&self) -> usize {
        self.task_index
    }

    fn wake_task(&self) {
        self.executor().set_task_runnable(self.task_index);
    }
}

pub(crate) fn from_waker(waker: &Waker) -> &WakerInfo {
    unsafe {
        (waker.data() as *const WakerInfo)
            .as_ref()
            .expect("waker has null data pointer")
    }
}

const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    // clone()
    |ptr| RawWaker::new(ptr, &WAKER_VTABLE),
    // wake()
    |ptr| unsafe {
        debug_assert!(!ptr.is_null());
        let waker = &*(ptr as *const WakerInfo);

        waker.wake_task();
    },
    // wake_by_ref()
    |ptr| unsafe {
        debug_assert!(!ptr.is_null());
        let waker = &*(ptr as *const WakerInfo);

        waker.wake_task();
    },
    // drop()
    |_| {},
);
