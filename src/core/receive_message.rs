use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};
use std::future::Future;

pub trait ReceiveMessage<M>: Send + 'static
where
    M: Send + 'static,
{
    /// Asynchronously receive the next message.
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + '_;
}

/// Value returned by the [recv](ReceiveMessage::recv) method.
#[derive(EnumAsInner)]
pub enum Recv<M> {
    /// The actor has received a message.
    Message(M),
    /// All [ActorRef](crate::core::actor_ref::ActorRef)s to the actor have been dropped, and all messages sent to the actor
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
