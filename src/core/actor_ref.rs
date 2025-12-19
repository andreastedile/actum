use futures::channel::mpsc;
use std::fmt::{Debug, Formatter};

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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

    /// Attempts to send a message to the actor behind this reference, returning the message if there was an error.
    ///
    /// Errors if the [receiver](crate::core::receive_message::ReceiveMessage) of the intended actor has been dropped,
    /// either manually or automatically upon return.
    pub fn try_send(&mut self, message: M) -> Result<(), M> {
        self.m_sender.try_send(message).map_err(mpsc::TrySendError::into_inner)
    }
}
