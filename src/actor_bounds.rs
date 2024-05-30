use crate::actor::Actor;
use crate::actor_cell::actor_task::RunTask;
use crate::actor_ref::ActorRef;
use std::future::Future;

pub trait ActorBounds<M>: Send + 'static
where
    M: Send + 'static,
{
    type ChildActorBoundsType<M2>: Send + 'static
    where
        M2: Send + 'static;
    type ChildActorBounds<M2>: ActorBounds<M2>
    where
        M2: Send + 'static;
    type SpawnOut<M2, F, Fut>: RunTask
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActorBounds<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static;

    /// Asynchronously receive the next message.
    ///
    /// This method returns `None` when the actor has been stopped by its parent or when all its [ActorRef]s have been dropped.
    /// After that this method has returned `None`, subsequent calls to the method will continue to do so.
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + '_;

    /// Define a child actor.
    ///
    /// This method returns `None` when the actor has been stopped by its parent or when all its [ActorRef]s have been dropped.
    /// After that this method has returned `None`, subsequent calls to the method will continue to do so.
    fn spawn<M2, F, Fut>(
        &mut self,
        f: F,
    ) -> impl Future<Output = Option<Actor<M2, Self::SpawnOut<M2, F, Fut>>>> + Send + '_
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActorBounds<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static;
}

pub enum Recv<M> {
    /// The actor has received a message.
    Message(M),
    /// The actor has been stopped by its parent and should stop.
    Stopped(Option<M>),
    /// All [`ActorRef`]s to the actor have been dropped, and all messages sent to the actor
    /// have been received by the actor.
    ///
    /// The actor may wish to terminate unless it has other sources of input.
    NoMoreSenders,
}

impl<M> Recv<M> {
    pub fn message(self) -> Option<M> {
        if let Self::Message(m) = self {
            Some(m)
        } else {
            None
        }
    }

    pub const fn is_message(&self) -> bool {
        matches!(self, Self::Message(_))
    }

    pub fn is_message_and(self, f: impl FnOnce(M) -> bool) -> bool {
        if let Self::Message(m) = self {
            f(m)
        } else {
            false
        }
    }

    pub fn stopped(self) -> Option<Option<M>> {
        if let Self::Stopped(m) = self {
            Some(m)
        } else {
            None
        }
    }

    pub fn is_stopped_and(self, f: impl FnOnce(Option<M>) -> bool) -> bool {
        if let Self::Stopped(m) = self {
            f(m)
        } else {
            false
        }
    }

    pub const fn is_no_more_senders(&self) -> bool {
        matches!(self, Self::NoMoreSenders)
    }

    pub const fn as_ref(&self) -> Recv<&M> {
        match self {
            Self::Message(message) => Recv::Message(message),
            Self::Stopped(message) => Recv::Stopped(message.as_ref()),
            Self::NoMoreSenders => Recv::NoMoreSenders,
        }
    }
}
