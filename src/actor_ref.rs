use futures::channel::mpsc;

pub struct ActorRef<M> {
    m_sender: mpsc::Sender<M>,
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

    /// Attempt to send a message to this actor, returning the message if there was an error.
    pub fn try_send(&mut self, message: M) -> Result<(), M> {
        self.m_sender.try_send(message).map_err(mpsc::TrySendError::into_inner)
    }
}
