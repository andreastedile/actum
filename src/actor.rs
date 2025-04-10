use crate::actor_ref::ExtendableMessageReceiver;
use crate::actor_ref::{create_actor_ref_and_message_receiver, ActorRef};
use crate::actor_task::{ExtensibleActorTask, RunTask};
use crate::actor_to_spawn::ActorToSpawn;
use crate::create_child::{ActorCell, CreateChild};
use std::future::Future;

impl CreateChild for ActorCell<()> {
    type MessageReceiverT<M2>
        = ExtendableMessageReceiver<M2, ()>
    where
        M2: Send + 'static;
    type HasRunTask<M2, F, Fut, Ret2>
        = ExtensibleActorTask<M2, F, Fut, Ret2, (), (), ()>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<()>, ExtendableMessageReceiver<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<()>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    async fn create_child<M2, F, Fut, Ret2>(&mut self, f: F) -> ActorToSpawn<M2, Self::HasRunTask<M2, F, Fut, Ret2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<()>, ExtendableMessageReceiver<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<()>, Ret2)> + Send + 'static,
        Ret2: Send + 'static,
    {
        let (actor_ref, receiver) = create_actor_ref_and_message_receiver::<M2>();

        let tracker = self.tracker.get_or_insert_default();
        let cell = ActorCell {
            tracker: None,
            dependency: (),
        };

        let task = ExtensibleActorTask::new(f, cell, receiver, actor_ref.clone(), (), Some(tracker.make_child()));

        ActorToSpawn::new(task, actor_ref)
    }
}

impl<M, F, Fut, Ret> RunTask<Ret> for ExtensibleActorTask<M, F, Fut, Ret, (), (), ()>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<()>, ExtendableMessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
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
