use crate::core::actor_cell::ActorCell;
use crate::core::actor_ref::ActorRef;
use crate::core::actor_task::ActorTask;
use crate::core::actor_to_spawn::ActorToSpawn;
use crate::core::create_child::CreateChild;
use crate::core::message_receiver::MessageReceiver;
use futures::channel::mpsc;
use std::future::Future;
impl CreateChild for ActorCell<()> {
    type ReceiveMessageT<M>
        = MessageReceiver<M, ()>
    where
        M: Send + 'static;
    type RunTaskT<M, F, Fut, Ret>
        = ActorTask<M, F, Fut, Ret, (), (), ()>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;

    async fn create_child<M, F, Fut, Ret>(&mut self, f: F) -> ActorToSpawn<M, Self::RunTaskT<M, F, Fut, Ret>>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        let m_channel = mpsc::channel::<M>(100);
        let actor_ref = ActorRef::new(m_channel.0);
        let receiver = MessageReceiver::new(m_channel.1, ());

        let cell = Self::new(());

        let tracker = self.tracker.get_or_insert_default().make_child();

        let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), (), Some(tracker));

        ActorToSpawn::new(task, actor_ref)
    }
}
