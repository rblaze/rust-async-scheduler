use core::task::{RawWaker, RawWakerVTable};

pub struct SimpleWaker {}

impl SimpleWaker {
    pub const fn new_raw_waker() -> RawWaker {
        RawWaker::new(core::ptr::null(), &SIMPLE_WAKER_VTABLE)
    }
}

const SIMPLE_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);
