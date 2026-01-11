use crate::actor::create_child::ActorCell;
use crate::actor::receive_message::MessageReceiver;
use crate::core::children_tracker::WakeParentOnDrop;
use crate::prelude::{ActorRef, RunTask};
use std::marker::PhantomData;

pub struct ActorTask<M, F, Fut, Output> {
    f: F,
    output: PhantomData<Output>,
    fut: PhantomData<Fut>,
    receiver: MessageReceiver<M>,
    cell: ActorCell,
    actor_ref: ActorRef<M>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
}

impl<M, F, Fut, Output> ActorTask<M, F, Fut, Output> {
    pub(crate) const fn new(
        f: F,
        cell: ActorCell,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
    ) -> Self {
        Self {
            f,
            output: PhantomData,
            fut: PhantomData,
            receiver,
            cell,
            actor_ref,
            _waker: waker,
        }
    }
}

impl<M, F, Fut, Output> RunTask<Output> for ActorTask<M, F, Fut, Output>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Output)> + Send + 'static,
    Output: Send + 'static,
{
    async fn run_task(self) -> Output {
        let f = self.f;
        let fut = f(self.cell, self.receiver, self.actor_ref);
        let (mut cell, output) = fut.await;

        if cell.tracker.has_children() {
            tracing::trace!("joining children");
            cell.tracker.join_all().await;
        }

        output
    }
}
