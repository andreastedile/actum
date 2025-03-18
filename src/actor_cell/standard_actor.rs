use crate::actor::{Actor, Recv};
use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use futures::channel::mpsc;
use futures::StreamExt;
use std::future::{poll_fn, Future};
use std::task::Poll;

impl<M> Actor<M> for ActorCell<M, ()>
where
    M: Send + 'static,
{
    type ChildActorDependency<M2: Send + 'static> = ();
    type ChildActor<M2: Send + 'static> = ActorCell<M2, ()>;
    type HasRunTask<M2, F, Fut, Ret>
        = ActorTask<M2, F, Fut, Ret, ()>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, ()>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + '_ {
        poll_fn(|cx| {
            //
            match self.m_receiver.poll_next_unpin(cx) {
                Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
                Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
                Poll::Pending => Poll::Pending,
            }
        })
    }

    async fn create_child<M2, F, Fut, Ret>(&mut self, f: F) -> ActorToSpawn<M2, ActorTask<M2, F, Fut, Ret, ()>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, ()>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, ()>, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        let m2_channel = mpsc::channel::<M2>(100);

        let cell = ActorCell::new(m2_channel.1, ());

        let actor_ref = ActorRef::new(m2_channel.0);
        let tracker = self.tracker.get_or_insert_default();
        let task = ActorTask::new(f, cell, actor_ref.clone(), Some(tracker.make_child()));

        ActorToSpawn::new(task, actor_ref)
    }
}
