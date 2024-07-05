use crate::actor::Actor;
use crate::actor_bounds::{ActorBounds, Recv};
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::ActorCell;
use crate::actor_cell::Stop;
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::effect::recv::{RecvEffectIn, RecvEffectOut};
use crate::effect::spawn::{SpawnEffectIn, SpawnEffectOut};
use crate::resolve_when_one::ResolveWhenOne;
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
use futures::future::{Either, FusedFuture};
use futures::stream::FusedStream;
use futures::{future, StreamExt};
use std::future::Future;

pub struct TestBounds<M> {
    recv_effect_out_m_sender: Option<oneshot::Sender<RecvEffectOut<M>>>,
    spawn_effect_out_sender: Option<oneshot::Sender<SpawnEffectOut>>,
}

impl<M> TestBounds<M> {
    pub const fn new(
        recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
        spawn_effect_out_sender: oneshot::Sender<SpawnEffectOut>,
    ) -> Self {
        Self {
            recv_effect_out_m_sender: Some(recv_effect_out_m_sender),
            spawn_effect_out_sender: Some(spawn_effect_out_sender),
        }
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
        Fut: Future<Output= ActorCell<M2, TestBounds<M2>>> + Send + 'static;

    async fn recv(&mut self) -> Recv<M> {
        let select = future::select(&mut self.stop_receiver, self.m_receiver.next()).await;

        let recv = match select {
            Either::Left(_) => {
                self.m_receiver.close();
                // let m = self.m_receiver.try_next().expect("channel is closed");
                // Recv::Stopped(m)
                let m = self.m_receiver.try_next().expect("channel is closed");
                Recv::Stopped(m)
            }
            Either::Right((Some(m), _)) => Recv::Message(m),
            Either::Right((None, _)) => Recv::NoMoreSenders,
        };

        if let Some(recv_effect_out_m_sender) = self.bounds.recv_effect_out_m_sender.take() {
            let recv_effect_in_m_channel = oneshot::channel::<RecvEffectIn<M>>();
            let effect = RecvEffectOut::new(recv, recv_effect_in_m_channel.0);

            if let Err(RecvEffectOut { recv: m, .. }) = recv_effect_out_m_sender.send(effect) {
                // The testkit dropped.
                m
            } else {
                let RecvEffectIn {
                    recv,
                    recv_effect_out_m_sender,
                } = recv_effect_in_m_channel.1.await.expect("the effect should reply");

                self.bounds.recv_effect_out_m_sender = Some(recv_effect_out_m_sender);
                recv
            }
        } else {
            recv
        }
    }

    async fn spawn<M2, F, Fut>(&mut self, f: F) -> Option<Actor<M2, ActorTask<M2, F, Fut, TestBounds<M2>>>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ActorCell<M2, TestBounds<M2>>> + Send + 'static,
    {
        if self.stop_receiver.is_terminated() || self.m_receiver.is_terminated() {
            if let Some(spawn_effect_out_sender) = self.bounds.spawn_effect_out_sender.take() {
                let spawn_effect_in_channel = oneshot::channel::<SpawnEffectIn>();
                let effect = SpawnEffectOut::new::<M2>(None, spawn_effect_in_channel.0);

                if let Err(SpawnEffectOut { .. }) = spawn_effect_out_sender.send(effect) {
                    // The testkit dropped.
                } else {
                    let SpawnEffectIn {
                        spawn_effect_out_sender,
                    } = spawn_effect_in_channel.1.await.expect("the effect should reply");

                    self.bounds.spawn_effect_out_sender = Some(spawn_effect_out_sender);
                }
            }

            return None;
        }

        let stop_channel = oneshot::channel::<Stop>();
        let m2_channel = mpsc::channel::<M2>(100);

        let guard = ActorDropGuard::new(stop_channel.0);
        let recv_effect_out_m2_channel = oneshot::channel::<RecvEffectOut<M2>>();
        let spawn_effect_out_channel = oneshot::channel::<SpawnEffectOut>();
        let bounds = TestBounds::new(recv_effect_out_m2_channel.0, spawn_effect_out_channel.0);
        let cell = ActorCell::new(stop_channel.1, m2_channel.1, bounds);

        let m2_ref = ActorRef::new(m2_channel.0);
        let subtree = self.subtree.get_or_insert(ResolveWhenOne::new());
        let task = ActorTask::new(f, cell, m2_ref.clone(), Some(subtree.clone()));

        if let Some(spawn_effect_out_sender) = self.bounds.spawn_effect_out_sender.take() {
            let spawn_effect_in_channel = oneshot::channel::<SpawnEffectIn>();
            let testkit = Testkit::new(recv_effect_out_m2_channel.1, spawn_effect_out_channel.1);
            let effect = SpawnEffectOut::new(Some(testkit), spawn_effect_in_channel.0);

            if let Err(SpawnEffectOut { .. }) = spawn_effect_out_sender.send(effect) {
                // The testkit dropped.
            } else {
                let SpawnEffectIn {
                    spawn_effect_out_sender,
                } = spawn_effect_in_channel.1.await.expect("the effect should reply");

                self.bounds.spawn_effect_out_sender = Some(spawn_effect_out_sender);
            }
        }

        Some(Actor::new(task, guard, m2_ref))
    }
}
