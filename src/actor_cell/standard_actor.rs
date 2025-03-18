use crate::actor::Actor;
use crate::actor_bounds::{ActorBounds, Recv};
use crate::actor_task::ActorTask;
use crate::actor_cell::ActorCell;
use crate::actor_cell::Stop;
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::resolve_when_one::ResolveWhenOne;
use either::Either;
use futures::channel::{mpsc, oneshot};
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
    type SpawnOut<M2, F, Fut, Ret>
        = ActorTask<M2, F, Fut, Ret, StandardBounds>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, StandardBounds>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, StandardBounds>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    fn recv(&mut self) -> impl Future<Output = Recv<M>> + Send + '_ {
        poll_fn(|cx| {
            match self.stop_receiver.poll_unpin(cx) {
                Poll::Ready(Ok(Stop)) => {
                    self.m_receiver.close();
                    self.m_receiver.poll_next_unpin(cx).map(Recv::Stopped)
                }
                Poll::Ready(Err(oneshot::Canceled)) => {
                    //
                    self.m_receiver.poll_next_unpin(cx).map(Recv::Stopped)
                }
                Poll::Pending => {
                    //
                    match self.m_receiver.poll_next_unpin(cx) {
                        Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
                        Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
                        Poll::Pending => Poll::Pending,
                    }
                }
            }
        })
    }

    async fn spawn<M2, F, Fut, Ret>(
        &mut self,
        f: F,
    ) -> Either<Actor<M2, ActorTask<M2, F, Fut, Ret, StandardBounds>>, Option<M>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, StandardBounds>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, StandardBounds>, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        let stopped = match self.stop_receiver.try_recv() {
            Ok(None) => false,
            Ok(Some(Stop)) => {
                self.m_receiver.close();
                true
            }
            Err(oneshot::Canceled) => true,
        };

        if stopped {
            let m = self.m_receiver.try_next().expect("message channel was closed");
            return Either::Right(m);
        }

        let stop_channel = oneshot::channel::<Stop>();
        let m2_channel = mpsc::channel::<M2>(100);

        let guard = ActorDropGuard::new(stop_channel.0);
        let bounds = StandardBounds;
        let cell = ActorCell::new(stop_channel.1, m2_channel.1, bounds);

        let m2_ref = ActorRef::new(m2_channel.0);
        let subtree = self.subtree.get_or_insert(ResolveWhenOne::new());
        let task = ActorTask::new(f, cell, m2_ref.clone(), Some(subtree.clone()));

        Either::Left(Actor::new(task, guard, m2_ref))
    }
}
