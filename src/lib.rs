#![cfg_attr(not(test), no_std)]

pub mod executor;
mod sleep;
mod waker;

#[cfg(test)]
mod test_utils;
