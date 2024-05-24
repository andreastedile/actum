use crate::actor::Actor;
use crate::actor_bounds::ActorBounds;
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::ActorCell;
use crate::actor_cell::{Stop, Stopped};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::effect::recv::RecvEffect;
use crate::effect::spawn::SpawnEffect;
use crate::effect::Effect;
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
use futures::future::{Either, FusedFuture};
use futures::stream::FusedStream;
use futures::{future, StreamExt};
use std::future::Future;

pub struct TestBounds<M> {
    effect_m_sender: mpsc::UnboundedSender<Effect<M>>,
}

impl<M> TestBounds<M> {
    pub const fn new(effect_m_sender: mpsc::UnboundedSender<Effect<M>>) -> Self {
        Self { effect_m_sender }
    }
}

impl<M> ActorBounds<M> for ActorCell<M, TestBounds<M>>
where
    M: Send + 'static,
{
    type ChildActorBoundsType<M2: Send + 'static> = TestBounds<M2>;
    type ChildActorBounds<M2: Send + 'static> = ActorCell<M2, TestBounds<M2>>;
    type SpawnOut<M2, F, Fut> = ActorTask<M2, F, Fut, TestBounds<M2>> where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output= ()> + Send + 'static;

    async fn recv(&mut self) -> Option<M> {
        let select = future::select(&mut self.stop_receiver, self.m_receiver.next()).await;

        let m = if let Either::Right((m, _)) = select { m } else { None };

        let m_channel = oneshot::channel::<Option<M>>();
        let effect = RecvEffect::new(m, m_channel.0);

        if let Err(error) = self.bounds.effect_m_sender.unbounded_send(effect.into()) {
            // The testkit dropped.

            return error.into_inner().recv().unwrap().into_inner();
        };

        m_channel.1.await.expect("the effect sends m on drop")
    }

    async fn spawn<M2, F, Fut>(&mut self, f: F) -> Result<Actor<M2, ActorTask<M2, F, Fut, TestBounds<M2>>>, Stop>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let channel = oneshot::channel::<()>();

        if self.stop_receiver.is_terminated() || self.m_receiver.is_terminated() {
            let effect = SpawnEffect::new::<M>(None, channel.0);

            if let Err(error) = self.bounds.effect_m_sender.unbounded_send(effect.into()) {
                // The testkit dropped.

                let _ = error.into_inner().spawn().unwrap().into_inner();
            } else {
                channel.1.await.expect("the effect sends unit on drop");
            }

            return Err(Stop);
        }

        let stop_channel = oneshot::channel::<Stop>();
        let stopped_channel = mpsc::unbounded::<Stopped>();
        let m2_channel = mpsc::channel::<M2>(100);
        let effect_m2_channel = mpsc::unbounded::<Effect<M2>>();

        let guard = ActorDropGuard::new(stop_channel.0);
        let bounds = TestBounds::new(effect_m2_channel.0);
        let cell = ActorCell::new(stop_channel.1, stopped_channel.0, m2_channel.1, bounds);

        let m_ref = ActorRef::new(m2_channel.0);
        let task = ActorTask::new(
            f,
            cell,
            m_ref.clone(),
            stopped_channel.1,
            Some(self.stopped_sender.clone()),
        );

        let testkit = Testkit::new(effect_m2_channel.1);
        let effect = SpawnEffect::new(Some(testkit), channel.0);

        if let Err(error) = self.bounds.effect_m_sender.unbounded_send(effect.into()) {
            // The testkit dropped.

            let _ = error.into_inner().recv().unwrap().into_inner();
        } else {
            channel.1.await.expect("the effect did not send unit on drop");
        }

        Ok(Actor::new(task, guard, m_ref))
    }
}
