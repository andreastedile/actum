use crate::actor_ref::ActorRef;
use crate::actor_task::RunTask;
use crate::actor_to_spawn::ActorToSpawn;
use crate::children_tracker::ChildrenTracker;
use crate::receive_message::ReceiveMessage;
use std::future::Future;

pub struct ActorCell<D> {
    pub(crate) tracker: Option<ChildrenTracker>,
    pub(crate) dependency: D,
}

impl<D> ActorCell<D> {
    pub const fn new(dependency: D) -> Self {
        Self {
            tracker: None,
            dependency,
        }
    }
}

pub trait CreateChild: Sized + Send + 'static {
    type MessageReceiverT<M2>: ReceiveMessage<M2> + Send + 'static
    where
        M2: Send + 'static;

    type HasRunTask<M2, F, Fut, Ret2>: RunTask<Ret2>
    where
        M2: Send + 'static,
        F: FnOnce(Self, Self::MessageReceiverT<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    /// Creates a child actor.
    /// The actor should then be spawned onto the runtime of choice.
    ///
    /// See the [actum](crate::actum) function for passing a function pointer, passing a closure and
    /// for passing arguments to the actor.
    fn create_child<M2, F, Fut, Ret2>(
        &mut self,
        f: F,
    ) -> impl Future<Output = ActorToSpawn<M2, Self::HasRunTask<M2, F, Fut, Ret2>>> + Send + '_
    where
        M2: Send + 'static,
        F: FnOnce(Self, Self::MessageReceiverT<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret2)> + Send + 'static,
        Ret2: Send + 'static;
}
