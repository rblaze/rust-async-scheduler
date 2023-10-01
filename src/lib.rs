#![cfg_attr(not(test), no_std)]

pub mod executor;
pub mod mailbox;
pub mod sync;
mod sleep;
mod waker;

#[cfg(test)]
mod test_utils;
