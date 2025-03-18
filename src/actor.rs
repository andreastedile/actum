use crate::actor_ref::ActorRef;
use crate::actor_task::RunTask;
use crate::actor_to_spawn::ActorToSpawn;
use std::fmt::{Debug, Formatter};
use std::future::Future;

pub trait Actor<M>: Send + 'static
where
    M: Send + 'static,
{
    type ChildActorDependency<M2>: Send + 'static
    where
        M2: Send + 'static;
    type ChildActor<M2>: Actor<M2>
    where
        M2: Send + 'static;
    type HasRunTask<M2, F, Fut, Ret>: RunTask<Ret>
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActor<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self::ChildActor<M2>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    /// Asynchronously receive the next message.
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + '_;

    fn create_child<M2, F, Fut, Ret>(
        &mut self,
        f: F,
    ) -> impl Future<Output = ActorToSpawn<M2, Self::HasRunTask<M2, F, Fut, Ret>>> + Send + '_
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActor<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self::ChildActor<M2>, Ret)> + Send + 'static,
        Ret: Send + 'static;
}

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
    pub fn message(self) -> Option<M> {
        if let Self::Message(m) = self {
            Some(m)
        } else {
            None
        }
    }

    pub fn unwrap_message(self) -> M {
        match self {
            Self::Message(m) => m,
            other => panic!("called `Recv::unwrap_message()` on a `{:?}` value", other),
        }
    }

    pub const fn is_message(&self) -> bool {
        matches!(self, Self::Message(_))
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn is_message_and(self, f: impl FnOnce(M) -> bool) -> bool {
        if let Self::Message(m) = self {
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
            Self::NoMoreSenders => Recv::NoMoreSenders,
        }
    }
}
