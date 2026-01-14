use crate::actor::receive_message::MessageReceiver;
use crate::actor::scoped::ScopedActorTask;
use crate::core::actor_ref::ActorRef;
use crate::core::actor_to_spawn::CreateActorResult;
use crate::core::children_tracker::ChildrenTracker;
use crate::core::create_child::CreateChild;
use futures::channel::mpsc;
use std::future::Future;

pub struct ActorCell {
    pub(crate) tracker: ChildrenTracker,
}

impl Default for ActorCell {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorCell {
    pub fn new() -> Self {
        Self {
            tracker: ChildrenTracker::new(),
        }
    }
}

impl CreateChild for ActorCell {
    type ReceiveMessageT<M>
        = MessageReceiver<M>
    where
        M: Send + 'static;

    type ScopedActorTaskT<M, F, Fut, Output>
        = ScopedActorTask<M, F, Fut, Output>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Output)> + Send + 'static,
        Output: Send + 'static;

    async fn create_child<M, F, Fut, Output>(
        &mut self,
        f: F,
    ) -> CreateActorResult<M, Self::ScopedActorTaskT<M, F, Fut, Output>>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Output)> + Send + 'static,
        Output: Send + 'static,
    {
        let m_channel = mpsc::channel::<M>(100);
        let actor_ref = ActorRef::new(m_channel.0);
        let receiver = MessageReceiver::new(m_channel.1);

        let cell = Self::new();

        let tracker = self.tracker.make_child();

        let task = ScopedActorTask::new(f, cell, receiver, actor_ref.clone(), Some(tracker));

        CreateActorResult::new(task, actor_ref)
    }
}
