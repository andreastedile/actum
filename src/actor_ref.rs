use futures::channel::mpsc;

pub struct ActorRef<M>(mpsc::Sender<M>);

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> ActorRef<M>
where
    M: Send + 'static,
{
    pub const fn new(m_sender: mpsc::Sender<M>) -> Self {
        Self(m_sender)
    }

    /// Attempt to send a message to this actor, returning the message if there was an error.
    pub fn try_send(&mut self, message: M) -> Result<(), M> {
        self.0.try_send(message).map_err(mpsc::TrySendError::into_inner)
    }
}
