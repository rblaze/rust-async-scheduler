use core::cell::Cell;
use core::future::Future;
use core::task::{Context, Poll};

// Sleeping future. For now sleeps until manually woken up.
#[derive(Debug)]
pub struct Event {
    signaled: Cell<bool>,
}

impl Event {
    pub fn new() -> Self {
        Event {
            signaled: Cell::new(false),
        }
    }

    pub fn signal(&self) {
        self.signaled.set(true);
    }

    pub fn is_ready(&self) -> bool {
        self.signaled.get()
    }

    pub fn wait(&self) -> EventFuture {
        EventFuture { event: self }
    }
}

impl Default for Event {
    fn default() -> Self {
        Event::new()
    }
}

#[must_use = "futures do nothing unless polled"]
pub struct EventFuture<'a> {
    event: &'a Event,
}

impl Future for EventFuture<'_> {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.event.is_ready() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    use futures::join;

    #[test]
    fn wait_for_event() {
        let result = Executor::block_on(async {
            let event = Event::new();

            join!(
                async {
                    Executor::sleep(200).await;
                    assert!(!event.is_ready());
                    event.signal();
                    10
                },
                async {
                    assert!(!event.is_ready());
                    event.wait().await;
                    20
                }
            )
        });
        assert_eq!(result, (10, 20));
    }

    #[test]
    fn event_already_signaled() {
        let result = Executor::block_on(async {
            let event = Event::new();

            event.signal();
            event.wait().await;

            42
        });
        assert_eq!(result, 42);
    }
}
