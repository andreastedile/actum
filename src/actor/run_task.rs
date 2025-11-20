use crate::actor::create_child::ActorCell;
use crate::actor::receive_message::MessageReceiver;
use crate::core::children_tracker::WakeParentOnDrop;
use crate::prelude::{ActorRef, RunTask};
use std::marker::PhantomData;

pub struct ActorTask<M, F, Fut, Ret> {
    f: F,
    ret: PhantomData<Ret>,
    fut: PhantomData<Fut>,
    receiver: MessageReceiver<M>,
    cell: ActorCell,
    actor_ref: ActorRef<M>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
}

impl<M, F, Fut, Ret> ActorTask<M, F, Fut, Ret> {
    pub(crate) const fn new(
        f: F,
        cell: ActorCell,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
    ) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            receiver,
            cell,
            actor_ref,
            _waker: waker,
        }
    }
}

impl<M, F, Fut, Ret> RunTask<Ret> for ActorTask<M, F, Fut, Ret>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let fut = f(self.cell, self.receiver, self.actor_ref);
        let (mut cell, ret) = fut.await;

        if cell.tracker.has_children() {
            tracing::trace!("joining children");
            cell.tracker.join_all().await;
        }

        ret
    }
}
