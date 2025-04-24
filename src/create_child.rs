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
    type ReceiveMessageT<M>: ReceiveMessage<M> + Send + 'static
    where
        M: Send + 'static;

    type RunTaskT<M, F, Fut, Ret>: RunTask<Ret>
    where
        M: Send + 'static,
        F: FnOnce(Self, Self::ReceiveMessageT<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;

    /// Creates a child actor.
    /// The actor should then be spawned onto the runtime of choice.
    ///
    /// See the [actum](crate::actum) function for passing a function pointer, passing a closure and
    /// for passing arguments to the actor.
    fn create_child<M, F, Fut, Ret>(
        &mut self,
        f: F,
    ) -> impl Future<Output = ActorToSpawn<M, Self::RunTaskT<M, F, Fut, Ret>>> + Send + '_
    where
        M: Send + 'static,
        F: FnOnce(Self, Self::ReceiveMessageT<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;
}
