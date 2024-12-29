use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[derive(Debug, Copy, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Yield {
    ready: bool,
}

impl Yield {
    pub(crate) fn new() -> Self {
        Self { ready: false }
    }
}

impl Future for Yield {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.ready {
            Poll::Ready(())
        } else {
            Pin::into_inner(self).ready = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::block_on;

    #[test]
    fn yield_and_wake() {
        let v = block_on(async {
            for _ in 0..10 {
                println!("iter enter");
                crate::executor::yield_now().await;
                println!("iter exit");
            }

            42
        });

        assert_eq!(v, 42);
    }
}
