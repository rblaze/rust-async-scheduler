use core::future::Future;
use core::task::{Context, Poll};

use crate::timer::get_ticks;

// Sleeping future. For now sleeps until manually woken up.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Sleep {
    wake_at_tick: u32,
}

impl Sleep {
    pub fn new(wake_at_tick: u32) -> Self {
        Sleep { wake_at_tick }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if get_ticks() >= self.wake_at_tick {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}
