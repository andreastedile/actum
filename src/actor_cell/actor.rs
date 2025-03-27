use crate::actor::{Actor, Recv};
use crate::actor_cell::ActorCell;
use crate::actor_ref::MessageReceiver;
use crate::actor_ref::{create_actor_ref_and_message_receiver, ActorRef};
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use std::future::Future;

impl<M> Actor<M> for ActorCell<()>
where
    M: Send + 'static,
{
    type ChildActorDependency<M2: Send + 'static> = ();
    type ChildActor<M2: Send + 'static> = ActorCell<()>;
    type HasRunTask<M2, F, Fut, Ret>
        = ActorTask<M2, F, Fut, Ret, ()>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<()>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    fn recv<'a>(&'a mut self, receiver: &'a mut MessageReceiver<M>) -> impl Future<Output = Recv<M>> + Send + 'a {
        receiver.recv()
    }

    async fn create_child<M2, F, Fut, Ret>(&mut self, f: F) -> ActorToSpawn<M2, ActorTask<M2, F, Fut, Ret, ()>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<()>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        let (actor_ref, receiver) = create_actor_ref_and_message_receiver::<M2>();

        let cell = Self::new(());

        let tracker = self.tracker.get_or_insert_default();
        let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), Some(tracker.make_child()));

        ActorToSpawn::new(task, actor_ref)
    }
}
