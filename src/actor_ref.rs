use crate::actor::Recv;
use futures::channel::mpsc;
use futures::StreamExt;
use std::future::{poll_fn, Future};
use std::task::Poll;

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
            Poll::Pending => Poll::Pending,
        })
    }
}

pub(crate) fn create_actor_ref_and_message_receiver<M>() -> (ActorRef<M>, MessageReceiver<M>) {
    let (m_sender, m_receiver) = mpsc::channel::<M>(100);
    let actor_ref = ActorRef::<M>::new(m_sender);
    let receiver = MessageReceiver::<M>::new(m_receiver);
    (actor_ref, receiver)
}
