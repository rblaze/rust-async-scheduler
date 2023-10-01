#![deny(unsafe_code)]

use core::cell::Cell;
use core::fmt::Debug;
use core::future::Future;
use core::task::{Context, Poll, Waker};
use critical_section::Mutex;

pub use crate::mailbox::Error;

/// Mailbox that can be posted from other threads, storing a single value and allowing to wait for it.
pub struct Mailbox<T> {
    value: Mutex<Cell<Option<T>>>,
    waker: Mutex<Cell<Option<Waker>>>,
}

impl<T: Debug + Copy> Debug for Mailbox<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("sync::Mailbox")
            .field("value", &self.value)
            .finish()
    }
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Mailbox<T> {
    /// Creates empty mailbox.
    pub const fn new() -> Self {
        Self {
            value: Mutex::new(Cell::new(None)),
            waker: Mutex::new(Cell::new(None)),
        }
    }

    /// Puts value into mailbox, returning previous value.
    pub fn post(&self, value: T) -> Option<T> {
        let (old_value, waker) = critical_section::with(|cs| {
            (
                self.value.borrow(cs).replace(Some(value)),
                self.waker.borrow(cs).take(),
            )
        });

        if let Some(waker) = waker {
            waker.wake();
        }

        old_value
    }

    /// Waits for value to be posted and returns it.
    pub async fn read(&self) -> Result<T, Error> {
        critical_section::with(|cs| {
            let waker = self.waker.borrow(cs).take();
            if let Some(waker) = waker {
                // Mailbox busy.
                // Restore waker and return error.
                self.waker.borrow(cs).set(Some(waker));
                Err(Error::AlreadyWaiting)
            } else {
                Ok(())
            }
        })?;

        let fut = MailboxFuture { mailbox: self };

        Ok(fut.await)
    }
}

struct MailboxFuture<'a, T> {
    mailbox: &'a Mailbox<T>,
}

impl<T> Future for MailboxFuture<'_, T> {
    type Output = T;

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        critical_section::with(|cs| match self.mailbox.value.borrow(cs).take() {
            Some(value) => Poll::Ready(value),
            None => {
                self.mailbox.waker.borrow(cs).set(Some(cx.waker().clone()));
                Poll::Pending
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use core::pin::pin;

    use futures::join;
    use futures::task::LocalFutureObj;

    use super::*;
    use crate::executor::LocalExecutor;
    use crate::test_utils::{assign, block_on};

    async fn post_and_read<T, U>(mbox1: &Mailbox<T>, value: T, mbox2: &Mailbox<U>) -> U {
        mbox1.post(value);
        mbox2.read().await.unwrap()
    }

    async fn read_and_post<T, U>(mbox1: &Mailbox<T>, mbox2: &Mailbox<U>, value: U) -> T {
        let t = mbox1.read().await.unwrap();
        mbox2.post(value);

        t
    }

    #[test]
    fn post_from_coroutine() {
        let mbox1 = Mailbox::<i32>::new();
        let mbox2 = Mailbox::<&'static str>::new();

        let (t, u) = block_on(async {
            join!(
                post_and_read(&mbox1, 42, &mbox2),
                read_and_post(&mbox1, &mbox2, "hello")
            )
        });

        assert_eq!(t, "hello");
        assert_eq!(u, 42);
    }

    #[test]
    fn multi_post() {
        let mbox1 = Mailbox::<i32>::new();
        let mbox2 = Mailbox::<&'static str>::new();

        let t = block_on(async {
            let mut v = 0;
            for i in 0..10 {
                (_, v) = join!(
                    post_and_read(&mbox1, i, &mbox2),
                    read_and_post(&mbox1, &mbox2, "hello")
                );
            }
            v
        });

        assert_eq!(t, 9);
    }

    #[test]
    fn post_from_other_task() {
        let mut t = "";
        let mut u = 0;

        {
            let mbox1 = Mailbox::<i32>::new();
            let mbox2 = Mailbox::<&'static str>::new();

            let mut f1 = pin!(assign(&mut u, read_and_post(&mbox1, &mbox2, "hello")));
            let fo1 = LocalFutureObj::new(&mut f1);

            let mut f2 = pin!(assign(&mut t, post_and_read(&mbox1, 42, &mbox2)));
            let fo2 = LocalFutureObj::new(&mut f2);

            let mut ex = LocalExecutor::<2>::new();
            assert_eq!(ex.spawn(fo1), Ok(()));
            assert_eq!(ex.spawn(fo2), Ok(()));
            ex.run();
        }

        assert_eq!(t, "hello");
        assert_eq!(u, 42);
    }

    #[test]
    fn double_read_returns_error() {
        let mbox = Mailbox::<i32>::new();

        let (t, u, w) = block_on(async {
            join!(
                async { mbox.read().await },
                async { mbox.read().await },
                async { mbox.post(42) }
            )
        });

        assert_eq!(t, Ok(42));
        assert_eq!(u, Err(Error::AlreadyWaiting));
        assert_eq!(w, None);
    }
}
