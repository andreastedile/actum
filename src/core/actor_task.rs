use std::future::Future;

pub trait RunTask<Output>: Send + 'static {
    fn run_task(self) -> impl Future<Output = Output> + Send + 'static;
}
