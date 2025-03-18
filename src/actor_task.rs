use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::resolve_when_one::ResolveWhenOne;
use std::future::Future;
use std::marker::PhantomData;

pub trait RunTask<Ret>: Send + 'static {
    fn run_task(self) -> impl Future<Output = Ret> + Send + 'static;
}

pub struct ActorTask<M, F, Fut, Ret, D> {
    f: F,
    ret: PhantomData<Ret>,
    fut: PhantomData<Fut>,
    cell: ActorCell<M, D>,
    m_ref: ActorRef<M>,
    _parent: Option<ResolveWhenOne>,
}

impl<M, F, Fut, Ret, D> ActorTask<M, F, Fut, Ret, D> {
    pub const fn new(f: F, cell: ActorCell<M, D>, m_ref: ActorRef<M>, parent: Option<ResolveWhenOne>) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            cell,
            m_ref,
            _parent: parent,
        }
    }
}

impl<M, F, Fut, Ret, D> RunTask<Ret> for ActorTask<M, F, Fut, Ret, D>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, D>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<M, D>, Ret)> + Send + 'static,
    Ret: Send + 'static,
    D: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let fut = f(self.cell, self.m_ref);
        let (mut cell, ret) = fut.await;

        if let Some(subtree) = cell.subtree.take() {
            tracing::trace!("join children");
            subtree.await;
        }

        ret
    }
}
