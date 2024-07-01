use crate::actor_cell::{ActorCell, Stopped};
use crate::actor_ref::ActorRef;
use futures::channel::mpsc;
use futures::StreamExt;
use std::future::Future;
use std::marker::PhantomData;

pub trait RunTask: Send + 'static {
    fn run_task(self) -> impl Future<Output = ()> + Send + 'static;
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
    async fn run_task(mut self) {
        let f = self.f;
        let fut = f(self.cell, self.m_ref);
        fut.await;

        tracing::trace!("join children");
        while self.stopped_receiver.next().await.is_some() {}

        if let Some(stopped_sender) = self.stopped_sender {
            stopped_sender
                .unbounded_send(Stopped)
                .expect("failed to send Stopped to parent");
        };
    }
}
