use crate::core::actor_cell::ActorCell;
use crate::core::actor_task::ActorTask;
use crate::core::message_receiver::MessageReceiver;
use crate::prelude::{ActorRef, RunTask};

impl<M, F, Fut, Ret> RunTask<Ret> for ActorTask<M, F, Fut, Ret, (), (), ()>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<()>, MessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let fut = f(self.cell, self.receiver, self.actor_ref);
        let (mut cell, ret) = fut.await;

        if let Some(mut tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        ret
    }
}
