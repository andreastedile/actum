use crate::actor_cell::{ActorCell, Stopped};
use crate::actor_ref::ActorRef;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::panic;

pub trait RunTask: Send + 'static {
    fn run_task(self) -> impl Future<Output = Option<Box<dyn Any + Send>>> + Send + 'static;
}

pub struct ActorTask<M, F, Fut, CABT> {
    f: F,
    fut: PhantomData<Fut>,
    cell: ActorCell<M, CABT>,
    m_ref: ActorRef<M>,
    stopped_receiver: mpsc::UnboundedReceiver<Stopped>,
    stopped_sender: Option<mpsc::UnboundedSender<Stopped>>,
}

impl<M, F, Fut, CABT> ActorTask<M, F, Fut, CABT> {
    pub const fn new(
        f: F,
        cell: ActorCell<M, CABT>,
        m_ref: ActorRef<M>,
        stopped_receiver: mpsc::UnboundedReceiver<Stopped>,
        stopped_sender: Option<mpsc::UnboundedSender<Stopped>>,
    ) -> Self {
        Self {
            f,
            fut: PhantomData,
            cell,
            m_ref,
            stopped_receiver,
            stopped_sender,
        }
    }
}

impl<M, F, Fut, CABT> RunTask for ActorTask<M, F, Fut, CABT>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, CABT>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
    CABT: Send + 'static,
{
    async fn run_task(mut self) -> Option<Box<dyn Any + Send>> {
        tracing::trace!("start");

        let f = self.f;
        let fut = match panic::catch_unwind(panic::AssertUnwindSafe(|| f(self.cell, self.m_ref))) {
            Ok(fut) => fut,
            Err(error) => {
                tracing::error!("panic");

                tracing::trace!("wait children");
                while self.stopped_receiver.next().await.is_some() {}

                if let Some(sender) = self.stopped_sender {
                    sender
                        .unbounded_send(Stopped)
                        .expect("failed to send Stopped to parent");
                };

                return Some(error);
            }
        };

        match panic::AssertUnwindSafe(fut).catch_unwind().await {
            Ok(_) => {
                tracing::trace!("wait children");
                while self.stopped_receiver.next().await.is_some() {}
            }
            Err(panic) => {
                tracing::debug!("panic");

                tracing::trace!("wait children");
                while self.stopped_receiver.next().await.is_some() {}

                return Some(panic);
            }
        };

        if let Some(sender) = self.stopped_sender {
            sender
                .unbounded_send(Stopped)
                .expect("failed to send Stopped to parent");
        }
        None
    }
}
