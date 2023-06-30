#![cfg_attr(not(test), no_std)]

pub mod executor;
mod sleep;
mod timer;
mod waker;

#[cfg(test)]
mod tests {
    use crate::executor::Executor;
    use futures::join;

    async fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    async fn add3(a: i32, b: i32, c: i32) -> i32 {
        let a_plus_b = add(a, b).await;
        a_plus_b + c
    }

    #[test]
    fn wait_for_single_coroutine() {
        let result = Executor::block_on(add3(1, 2, 3));
        assert_eq!(result, 6);
    }

    #[test]
    fn wait_for_join() {
        let f1 = add(2, 2);
        let f2 = add3(1, 2, 3);
        let result = Executor::block_on(async { join!(f1, f2) });
        assert_eq!(result, (4, 6));
    }

    async fn waiting_task() -> i32 {
        for _ in 0..10 {
            Executor::sleep(10).await;
        }

        42
    }

    #[test]
    fn sleep_and_wake() {
        let mut executor = Executor::default();
        let result = executor.run_until(waiting_task());
        assert_eq!(result, 42);
    }
}
