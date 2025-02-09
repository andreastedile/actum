use crate::actor::Actor;
use crate::actor_bounds::{ActorBounds, Recv};
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::ActorCell;
use crate::actor_cell::Stop;
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::effect::{
    RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor, SpawnEffectFromActorToTestkit,
    SpawnEffectFromTestkitToActor,
};
use crate::resolve_when_one::ResolveWhenOne;
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
use futures::{future, StreamExt};
use std::future::Future;

pub struct TestBounds<M> {
    recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
    spawn_effect_sender: mpsc::Sender<SpawnEffectFromActorToTestkit<M>>,
    spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor<M>>,
    awaiting_testkit: bool,
}

impl<M> TestBounds<M> {
    pub const fn new(
        recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
        spawn_effect_sender: mpsc::Sender<SpawnEffectFromActorToTestkit<M>>,
        spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor<M>>,
    ) -> Self {
        Self {
            recv_effect_sender,
            recv_effect_receiver,
            spawn_effect_sender,
            spawn_effect_receiver,
            awaiting_testkit: false,
        }
    }
}

impl<M> ActorBounds<M> for ActorCell<M, TestBounds<M>>
where
    M: Send + 'static,
{
    type ChildActorBoundsType<M2: Send + 'static> = TestBounds<M2>;
    type ChildActorBounds<M2: Send + 'static> = ActorCell<M2, TestBounds<M2>>;
    type SpawnOut<M2, F, Fut, Ret>
        = ActorTask<M2, F, Fut, Ret, TestBounds<M2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, TestBounds<M2>>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    async fn recv(&mut self) -> Recv<M> {
        assert!(!self.bounds.recv_effect_sender.is_closed());

        if !self.bounds.awaiting_testkit {
            let select = future::select(&mut self.stop_receiver, self.m_receiver.next()).await;

            let recv = match select {
                future::Either::Left((Ok(Stop), _)) => {
                    self.m_receiver.close();
                    let m = self.m_receiver.try_next().expect("message channel was closed");
                    Recv::Stopped(m)
                }
                future::Either::Left((Err(oneshot::Canceled), _)) => {
                    let m = self.m_receiver.try_next().expect("message channel was closed");
                    Recv::Stopped(m)
                }
                future::Either::Right((Some(m), _)) => Recv::Message(m),
                future::Either::Right((None, _)) => Recv::NoMoreSenders,
            };

            let recv_effect_to_testkit = RecvEffectFromActorToTestkit(recv);

            self.bounds
                .recv_effect_sender
                .try_send(recv_effect_to_testkit)
                .expect("could not send the effect to the testkit");

            // there is an await next
            self.bounds.awaiting_testkit = true; // now the future is cancel safe
        }

        let recv_effect_from_testkit = self
            .bounds
            .recv_effect_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        self.bounds.awaiting_testkit = false;

        recv_effect_from_testkit.0
    }

    async fn spawn<M2, F, Fut, Ret>(
        &mut self,
        f: F,
    ) -> either::Either<Actor<M2, ActorTask<M2, F, Fut, Ret, TestBounds<M2>>>, Option<M>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, TestBounds<M2>>, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        assert!(!self.bounds.spawn_effect_sender.is_closed());

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
            let spawn_effect_actor_to_testkit = SpawnEffectFromActorToTestkit(either::Either::Right(m));

            self.bounds
                .spawn_effect_sender
                .try_send(spawn_effect_actor_to_testkit)
                .expect("could not send the effect to the testkit");

            let effect_from_testkit = self
                .bounds
                .spawn_effect_receiver
                .next()
                .await
                .expect("could not receive the effect back from the testkit");

            let m = effect_from_testkit.0.unwrap_right();
            return either::Either::Right(m);
        }

        let stop_channel = oneshot::channel::<Stop>();
        let m2_channel = mpsc::channel::<M2>(100);

        let guard = ActorDropGuard::new(stop_channel.0);
        let recv_effect_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M2>>(1);
        let recv_effect_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M2>>(1);
        let spawn_effect_actor_to_testkit_channel = mpsc::channel::<SpawnEffectFromActorToTestkit<M2>>(1);
        let spawn_effect_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor<M2>>(1);
        let bounds = TestBounds::new(
            recv_effect_actor_to_testkit_channel.0,
            recv_effect_testkit_to_actor_channel.1,
            spawn_effect_actor_to_testkit_channel.0,
            spawn_effect_testkit_to_actor_channel.1,
        );
        let cell = ActorCell::new(stop_channel.1, m2_channel.1, bounds);

        let m2_ref = ActorRef::new(m2_channel.0);
        let subtree = self.subtree.get_or_insert(ResolveWhenOne::new());
        let task = ActorTask::new(f, cell, m2_ref.clone(), Some(subtree.clone()));

        let testkit = Testkit::new(
            recv_effect_actor_to_testkit_channel.1,
            recv_effect_testkit_to_actor_channel.0,
            spawn_effect_actor_to_testkit_channel.1,
            spawn_effect_testkit_to_actor_channel.0,
        );
        let spawn_effect_to_testkit = SpawnEffectFromActorToTestkit(either::Either::Left(Some(testkit.into())));

        self.bounds
            .spawn_effect_sender
            .try_send(spawn_effect_to_testkit)
            .expect("could not send the effect to the testkit");

        let spawn_effect_from_testkit = self
            .bounds
            .spawn_effect_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        assert!(spawn_effect_from_testkit.0.is_left());

        either::Either::Left(Actor::new(task, guard, m2_ref))
    }
}
