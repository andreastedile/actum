use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::children_tracker::WakeParentOnDrop;
use crate::message_receiver::MessageReceiver;
use std::future::Future;
use std::marker::PhantomData;

pub trait RunTask<Ret>: Send + 'static {
    fn run_task(self) -> impl Future<Output = Ret> + Send + 'static;
}

pub struct ActorTask<M, F, Fut, Ret, D> {
    f: F,
    ret: PhantomData<Ret>,
    fut: PhantomData<Fut>,
    cell: ActorCell<D>,
    receiver: MessageReceiver<M>,
    actor_ref: ActorRef<M>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
}

impl<M, F, Fut, Ret, D> ActorTask<M, F, Fut, Ret, D> {
    pub const fn new(
        f: F,
        cell: ActorCell<D>,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
    ) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            cell,
            receiver,
            actor_ref,
            _waker: waker,
        }
    }
}

impl<M, F, Fut, Ret, D> RunTask<Ret> for ActorTask<M, F, Fut, Ret, D>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<D>, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<D>, Ret)> + Send + 'static,
    Ret: Send + 'static,
    D: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let fut = f(self.cell, self.receiver, self.actor_ref);
        let (mut cell, ret) = fut.await;

        if let Some(tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        ret
    }
}
