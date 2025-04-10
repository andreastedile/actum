use crate::actor::Actor;
use crate::actor_cell::ActorCell;
use crate::actor_ref::ExtendableMessageReceiver;
use crate::actor_ref::{create_actor_ref_and_message_receiver, ActorRef};
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use std::future::Future;

impl<M, Ret> Actor<M, Ret> for ActorCell<(), Ret>
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    type ChildActor<M2: Send + 'static, Ret2: Send + 'static> = ActorCell<(), Ret2>;
    type HasRunTask<M2, F, Fut, Ret2>
        = ActorTask<M2, F, Fut, Ret2, (), ExtendableMessageReceiver<M2, ()>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<(), Ret2>, ExtendableMessageReceiver<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<(), Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    type MessageReceiverT<M2>
        = ExtendableMessageReceiver<M2, ()>
    where
        M2: Send + 'static;

    async fn create_child<M2, F, Fut, Ret2>(
        &mut self,
        f: F,
    ) -> ActorToSpawn<M2, ActorTask<M2, F, Fut, Ret2, (), ExtendableMessageReceiver<M2, ()>>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<(), Ret2>, ExtendableMessageReceiver<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<(), Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static,
    {
        let (actor_ref, receiver) = create_actor_ref_and_message_receiver::<M2>();

        let cell = ActorCell::new(());

        let tracker = self.tracker.get_or_insert_default();
        let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), Some(tracker.make_child()));

        ActorToSpawn::new(task, actor_ref)
    }
}
