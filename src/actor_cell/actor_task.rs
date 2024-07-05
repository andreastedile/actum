use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::resolve_when_one::ResolveWhenOne;
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
    _parent: Option<ResolveWhenOne>,
}

impl<M, F, Fut, CABT> ActorTask<M, F, Fut, CABT> {
    pub const fn new(f: F, cell: ActorCell<M, CABT>, m_ref: ActorRef<M>, parent: Option<ResolveWhenOne>) -> Self {
        Self {
            f,
            fut: PhantomData,
            cell,
            m_ref,
            _parent: parent,
        }
    }
}

impl<M, F, Fut, CABT> RunTask for ActorTask<M, F, Fut, CABT>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, CABT>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = ActorCell<M, CABT>> + Send + 'static,
    CABT: Send + 'static,
{
    async fn run_task(mut self) {
        let f = self.f;
        let fut = f(self.cell, self.m_ref);
        self.cell = fut.await;

        if let Some(subtree) = self.cell.subtree.take() {
            tracing::trace!("join children");
            subtree.await;
        }
    }
}
