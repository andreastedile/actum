use enum_as_inner::EnumAsInner;
use futures::channel::mpsc;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};

/// Reference to an actor that can be used to send messages to it, enabling communication.
///
/// It is first obtained by creating an actor either at the top level of the actor tree hierarchy or as a child of an existing actor.
/// In both cases, both the newly created actor and the caller that created the actor obtain a copy of the reference.
///
/// It can be cloned and shared between actors in messages.
///
/// **Reference count**: If all references to an actor are dropped (including the actor's own copy — that is, no more senders exist), any subsequent call by the actor to the [recv](crate::core::receive_message::ReceiveMessage::recv) method of its receiver will return the [NoMoreSenders](crate::core::receive_message::Recv::NoMoreSenders) variant.
pub struct ActorRef<M> {
    m_sender: mpsc::Sender<M>,
}

impl<M> Debug for ActorRef<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorRef")
            .field("closed", &self.m_sender.is_closed())
            .finish()
    }
}

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        Self {
            m_sender: self.m_sender.clone(),
        }
    }
}

impl<M> PartialEq<Self> for ActorRef<M> {
    fn eq(&self, other: &Self) -> bool {
        self.m_sender.same_receiver(&other.m_sender)
    }
}

impl<M> Eq for ActorRef<M> {}

impl<M> ActorRef<M> {
    pub(crate) const fn new(m_sender: mpsc::Sender<M>) -> Self {
        Self { m_sender }
    }

    /// Attempts to send a message to the referenced actor, returning the message if an error occurs.
    ///
    /// Errors if the actor's receiver has been dropped or if the underlying channel is full.
    /// The receiver may be dropped in two cases:
    /// 1. The actor's future has completed, in which case the receiver is automatically dropped at the end of its scope.
    /// 2. The receiver is explicitly dropped by the user (for example, to decrease the [reference count](ActorRef)).
    ///
    /// Therefore, a send error does not necessarily indicate that the actor's future has completed.
    pub fn try_send(&mut self, message: M) -> Result<(), TrySendError<M>> {
        self.m_sender.try_send(message).map_err(|err| {
            if err.is_full() {
                TrySendError::Full(err.into_inner())
            } else {
                TrySendError::ReceiverDropped(err.into_inner())
            }
        })
    }
}

/// Error returned from [`try_send`](ActorRef::try_send).
#[derive(EnumAsInner)]
pub enum TrySendError<M> {
    /// The error is a result of the receiver being dropped.
    Full(M),
    /// The error is a result of the receiver being dropped.
    ReceiverDropped(M),
}

impl<M> TrySendError<M> {
    pub fn into_message(self) -> M {
        match self {
            TrySendError::Full(m) => m,
            TrySendError::ReceiverDropped(m) => m,
        }
    }
}

impl<M> Error for TrySendError<M> {}

impl<M> Debug for TrySendError<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("TrySendError::Full"),
            TrySendError::ReceiverDropped(_) => f.write_str("TrySendError::ReceiverDropped"),
        }
    }
}

impl<M> Display for TrySendError<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("the channel is full"),
            TrySendError::ReceiverDropped(_) => f.write_str("the receiver dropped"),
        }
    }
}
