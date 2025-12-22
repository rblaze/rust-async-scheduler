#![cfg_attr(not(test), no_std)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::unwrap_in_result)]

pub mod executor;
pub mod mailbox;
mod sleep;
pub mod sync;
pub mod time;
mod waker;
mod yield_now;

// Implementation constraints:
// * tasks are bound to the executor when run is called and can't migrate to other executors
// * no async functions in this crate are called from third-party executors (such as tokio)
// * number of tasks is small and running through the entire list on each iteration is okay

/// Reschedules current task for the next executor run.
pub async fn yield_executor() {
    yield_now::Yield::new().await
}

/// Returns awaitable that pauses execution for `duration`.
pub async fn sleep(duration: time::Duration) {
    sleep::Sleep::new(now().await + duration).await
}

/// Returns current time as provided by environment.
pub async fn now() -> time::Instant {
    time::CurrentTime::new().await
}

#[cfg(test)]
mod test_utils;
