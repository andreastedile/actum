use crate::prelude::{ReceiveMessage, Recv};
use futures::StreamExt;
use futures::channel::mpsc;
use std::future::poll_fn;
use std::task::Poll;

pub struct MessageReceiver<M> {
    m_receiver: mpsc::Receiver<M>,
}

impl<M> MessageReceiver<M> {
    pub fn new(m_receiver: mpsc::Receiver<M>) -> Self {
        Self { m_receiver }
    }
}

impl<M> ReceiveMessage<M> for MessageReceiver<M>
where
    M: Send + 'static,
{
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + Unpin + '_ {
        poll_fn(|cx| {
            //
            match self.m_receiver.poll_next_unpin(cx) {
                Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
                Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
                Poll::Pending => Poll::Pending,
            }
        })
    }
}
