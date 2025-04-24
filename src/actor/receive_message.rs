use crate::core::message_receiver::MessageReceiver;
use crate::prelude::{ReceiveMessage, Recv};
use futures::StreamExt;
use std::future::poll_fn;
use std::task::Poll;

impl<M> ReceiveMessage<M> for MessageReceiver<M, ()>
where
    M: Send + 'static,
{
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + '_ {
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
