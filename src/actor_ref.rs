use crate::actor::Recv;
use futures::channel::mpsc;
use futures::StreamExt;
use std::future::{poll_fn, Future};
use std::task::Poll;

pub struct ActorRef<M>(mpsc::Sender<M>);

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> PartialEq<Self> for ActorRef<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_receiver(&other.0)
    }
}

impl<M> Eq for ActorRef<M> {}

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

pub struct MessageReceiver<M> {
    m_receiver: mpsc::Receiver<M>,
}

impl<M> MessageReceiver<M> {
    pub(crate) const fn new(m_receiver: mpsc::Receiver<M>) -> Self {
        Self { m_receiver }
    }

    pub(crate) fn recv(&mut self) -> impl Future<Output = Recv<M>> + '_ {
        poll_fn(|cx| match self.m_receiver.poll_next_unpin(cx) {
            Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
            Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
            Poll::Pending => return Poll::Pending,
        })
    }
}
