use crate::actor::Actor;
use crate::actor_bounds::{ActorBounds, Recv};
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::Stop;
use crate::actor_cell::{ActorCell, Stopped};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use futures::channel::{mpsc, oneshot};
use futures::future::FusedFuture;
use futures::stream::FusedStream;
use futures::{FutureExt, StreamExt};
use std::future::{poll_fn, Future};
use std::task::Poll;

pub struct StandardBounds;

impl<M> ActorBounds<M> for ActorCell<M, StandardBounds>
where
    M: Send + 'static,
{
    type ChildActorBoundsType<M2: Send + 'static> = StandardBounds;
    type ChildActorBounds<M2: Send + 'static> = ActorCell<M2, StandardBounds>;
    type SpawnOut<M2, F, Fut> = ActorTask<M2, F, Fut, StandardBounds> where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, StandardBounds>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static;

    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + '_ {
        poll_fn(|cx| {
            if self.stop_receiver.poll_unpin(cx).is_ready() {
                // Poll::Ready(Ok(Stop)) | Poll::Ready(Err(oneshot::Canceled))

                self.m_receiver.close();
                self.m_receiver.poll_next_unpin(cx).map(Recv::Stopped)
            } else {
                match self.m_receiver.poll_next_unpin(cx) {
                    Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
                    Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
                    Poll::Pending => Poll::Pending,
                }
            }
        })
    }

    async fn spawn<M2, F, Fut>(&mut self, f: F) -> Option<Actor<M2, ActorTask<M2, F, Fut, StandardBounds>>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, StandardBounds>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if self.stop_receiver.is_terminated() || self.m_receiver.is_terminated() {
            return None;
        }

        let stop_channel = oneshot::channel::<Stop>();
        let stopped_channel = mpsc::unbounded::<Stopped>();
        let m2_channel = mpsc::channel::<M2>(100);

        let guard = ActorDropGuard::new(stop_channel.0);
        let bounds = StandardBounds;
        let cell = ActorCell::new(stop_channel.1, stopped_channel.0, m2_channel.1, bounds);

        let m2_ref = ActorRef::new(m2_channel.0);
        let task = ActorTask::new(
            f,
            cell,
            m2_ref.clone(),
            stopped_channel.1,
            Some(self.stopped_sender.clone()),
        );

        Some(Actor::new(task, guard, m2_ref))
    }
}
