use crate::actor::Recv;
use futures::channel::mpsc;
use futures::StreamExt;
use std::future::{poll_fn, Future};
use std::task::Poll;

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
