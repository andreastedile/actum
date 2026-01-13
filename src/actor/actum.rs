use crate::actor::create_child::ActorCell;
use crate::actor::receive_message::MessageReceiver;
use crate::actor::run_task::ActorTask;
use crate::core::actor_ref::ActorRef;
use crate::core::actor_to_spawn::CreateActorResult;
use futures::channel::mpsc;

pub fn actum<M, F, Fut, Output>(f: F) -> CreateActorResult<M, ScopedActorTask<M, F, Fut, Output>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Output)> + Send + 'static,
    Output: Send + 'static,
{
    let m_channel = mpsc::channel::<M>(100);
    let actor_ref = ActorRef::new(m_channel.0);
    let receiver = MessageReceiver::new(m_channel.1);

    let cell = ActorCell::new();

    let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), None);

    CreateActorResult::new(task, actor_ref)
}
