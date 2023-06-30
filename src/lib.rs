#![cfg_attr(not(test), no_std)]

pub mod event;
pub mod executor;
mod sleep;
mod timer;
mod waker;
