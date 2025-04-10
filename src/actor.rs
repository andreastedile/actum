use crate::actor_ref::ActorRef;
use crate::actor_ref::MessageReceiver;
use crate::actor_task::RunTask;
use crate::actor_to_spawn::ActorToSpawn;
use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};
use std::future::Future;

pub trait Actor<M, Ret>: Send + 'static
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    type ChildActor<M2, Ret2>: Actor<M2, Ret2>
    where
        M2: Send + 'static,
        Ret2: Send + 'static;
    type HasRunTask<M2, F, Fut, Ret2>: RunTask<Ret2>
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActor<M2, Ret2>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self::ChildActor<M2, Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    /// Asynchronously receive the next message.
    fn recv<'a>(&'a mut self, receiver: &'a mut MessageReceiver<M>) -> impl Future<Output = Recv<M>> + Send + 'a;

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
        F: FnOnce(Self::ChildActor<M2, Ret2>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self::ChildActor<M2, Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;
}

/// Value returned by the [recv](Actor::recv) method.
#[derive(EnumAsInner)]
pub enum Recv<M> {
    /// The actor has received a message.
    Message(M),
    /// All [`ActorRef`]s to the actor have been dropped, and all messages sent to the actor
    /// have been received by the actor.
    ///
    /// The actor may wish to terminate unless it has other sources of input.
    NoMoreSenders,
}

impl<M> Debug for Recv<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(_) => f.write_str("Message"),
            Self::NoMoreSenders => f.write_str("NoMoreSenders"),
        }
    }
}

impl<M> Recv<M> {
    pub const fn as_ref(&self) -> Recv<&M> {
        match self {
            Self::Message(message) => Recv::Message(message),
            Self::NoMoreSenders => Recv::NoMoreSenders,
        }
    }
}
