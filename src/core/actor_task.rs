use std::future::Future;

pub trait RunTask<Ret>: Send + 'static {
    fn run_task(self) -> impl Future<Output = Ret> + Send + 'static;
}
