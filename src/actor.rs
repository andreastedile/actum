use crate::actor_ref::ActorRef;
use crate::actor_task::{ActorTask, RunTask};
use crate::actor_to_spawn::ActorToSpawn;
use crate::create_child::{ActorCell, CreateChild};
use crate::prelude::{ReceiveMessage, Recv};
use crate::receive_message::MessageReceiver;
use futures::StreamExt;
use futures::channel::mpsc;
use std::future::{Future, poll_fn};
use std::task::Poll;

impl CreateChild for ActorCell<()> {
    type ReceiveMessageT<M2>
        = MessageReceiver<M2, ()>
    where
        M2: Send + 'static;
    type RunTaskT<M2, F, Fut, Ret2>
        = ActorTask<M2, F, Fut, Ret2, (), (), ()>
    where
        M2: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    async fn create_child<M2, F, Fut, Ret2>(&mut self, f: F) -> ActorToSpawn<M2, Self::RunTaskT<M2, F, Fut, Ret2>>
    where
        M2: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret2)> + Send + 'static,
        Ret2: Send + 'static,
    {
        let m_channel = mpsc::channel::<M2>(100);
        let actor_ref = ActorRef::new(m_channel.0);
        let receiver = MessageReceiver::new(m_channel.1, ());

        let cell = Self::new(());

        let tracker = self.tracker.get_or_insert_default().make_child();

        let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), (), Some(tracker));

        ActorToSpawn::new(task, actor_ref)
    }
}

impl<M, F, Fut, Ret> RunTask<Ret> for ActorTask<M, F, Fut, Ret, (), (), ()>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<()>, MessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let fut = f(self.cell, self.receiver, self.actor_ref);
        let (mut cell, ret) = fut.await;

        if let Some(mut tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        ret
    }
}

impl<M> ReceiveMessage<M> for MessageReceiver<M, ()>
where
    M: Send + 'static,
{
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + '_ {
        poll_fn(|cx| match self.m_receiver.poll_next_unpin(cx) {
            Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
            Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
            Poll::Pending => Poll::Pending,
        })
    }
}
