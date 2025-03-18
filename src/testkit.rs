use crate::actor::Actor;
use crate::actor_cell::test_actor::TestBounds;
use crate::actor_cell::{ActorCell, Stop};
use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::drop_guard::ActorDropGuard;

use crate::effect::{
    Effect, EffectFromActorToTestkit, RecvEffect, RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor,
    SpawnEffect, SpawnEffectFromActorToTestkit, SpawnEffectFromTestkitToActor,
};
use futures::channel::{mpsc, oneshot};
use futures::{future, StreamExt};
use std::any::Any;
use std::future::Future;

/// Receive and test actor [effects](EffectFromActorToTestkit).
///
/// # Examples
///
/// Test whether the actor called [recv](crate::actor_bounds::ActorBounds::recv).
///
/// ```
/// use actum::prelude::*;
/// use actum::testkit::testkit;
///
/// async fn root<AB>(mut cell: AB, mut me: ActorRef<u64>) -> (AB, ())
/// where
///     AB: ActorBounds<u64>,
/// {
///     let m1 = cell.recv().await.message().unwrap();
///     me.try_send(m1 * 2).unwrap();
///
///     let m2 = cell.recv().await.message().unwrap();
///     debug_assert_eq!(m2, m1 * 2);
///
///     (cell, ())
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let (mut root, mut testkit) = testkit(root);
///     let handle = tokio::spawn(root.task.run_task());
///
///     root.m_ref.try_send(42).unwrap();
///
///     let _ = testkit
///         .test_next_effect(async |effect| {
///             let effect = effect.unwrap();
///             let m = effect.unwrap_recv().recv.unwrap_message();
///             assert_eq!(*m, 42);
///         })
///         .await;
///
///     let _ = testkit
///         .test_next_effect(async |effect| {
///             let effect = effect.unwrap();
///             let m = effect.unwrap_recv().recv.unwrap_message();
///             assert_eq!(*m, 84);
///         })
///         .await;
///
///     handle.await.unwrap();
/// }
/// ```
///
/// Test whether the actor called [spawn](crate::actor_bounds::ActorBounds::spawn).
///
/// # Example
/// ```
/// use actum::prelude::*;
/// use actum::testkit::testkit;
///
/// async fn parent<AB>(mut cell: AB, _me: ActorRef<u64>) -> (AB, ())
/// where
///     AB: ActorBounds<u64>,
/// {
///     let child = cell.spawn(child).await.unwrap_left();
///     let handle = tokio::spawn(child.task.run_task());
///     handle.await.unwrap();
///     (cell, ())
/// }
///
/// async fn child<AB>(mut cell: AB, _me: ActorRef<u32>) -> (AB, ())
/// where
///     AB: ActorBounds<u32>,
/// {
///     println!("child");
///     (cell, ())
/// }
///
/// #[tokio::test]
/// async fn test() {
///     let (parent, mut parent_testkit) = testkit(parent);
///     let handle = tokio::spawn(parent.task.run_task());
///
///     let (_parent_testkit, _child_testkit) = parent_testkit.test_next_effect(|effect| {
///         effect.unwrap_spawn().testkit_or_message.downcast_unwrap::<u32>()
///     });
///
///     // Use the child testkit...
///
///     handle.await.unwrap();
/// }
/// ```
pub struct Testkit<M> {
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
    recv_effect_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
    spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromActorToTestkit<M>>,
    spawn_effect_sender: mpsc::Sender<SpawnEffectFromTestkitToActor<M>>,
}

impl<M> Testkit<M> {
    pub const fn new(
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
        recv_effect_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
        spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromActorToTestkit<M>>,
        spawn_effect_sender: mpsc::Sender<SpawnEffectFromTestkitToActor<M>>,
    ) -> Self {
        Self {
            recv_effect_receiver,
            recv_effect_sender,
            spawn_effect_receiver,
            spawn_effect_sender,
        }
    }

    /// Receive an effect from the actor and test it.
    /// If the actor has returned, then there will be no more effects to test and None is returned.
    /// Otherwise, the testkit is returned.
    #[must_use]
    pub async fn test_next_effect<T>(&mut self, handler: impl AsyncFnOnce(Option<Effect<M>>) -> T) -> T
    where
        M: Send + 'static,
    {
        let select = future::select(self.recv_effect_receiver.next(), self.spawn_effect_receiver.next()).await;

        let mut effect_from_actor = match select {
            future::Either::Left((Some(inner), _)) => Some(EffectFromActorToTestkit::Recv(inner)),
            future::Either::Right((Some(inner), _)) => Some(EffectFromActorToTestkit::Spawn(inner)),
            future::Either::Left((None, _)) => {
                // The actor has returned -> TestBounds has been dropped at the end of the ActorTask::run_task scope
                // -> the channel is closed.
                assert!(self.recv_effect_sender.is_closed());
                None
            }
            future::Either::Right((None, _)) => {
                // Same reasoning.
                assert!(self.spawn_effect_sender.is_closed());
                None
            }
        };

        let mut discarded = false;
        let effect = match &mut effect_from_actor {
            None => None,
            Some(EffectFromActorToTestkit::Recv(inner)) => {
                let effect = RecvEffect {
                    recv: inner.0.as_ref(),
                    discarded: &mut discarded,
                };
                Some(Effect::Recv(effect))
            }
            Some(EffectFromActorToTestkit::Spawn(inner)) => {
                //
                match &mut inner.0 {
                    either::Either::Left(testkit) => {
                        let effect = SpawnEffect {
                            testkit_or_message: either::Either::Left(testkit.take().unwrap()),
                        };
                        Some(Effect::Spawn(effect))
                    }
                    either::Either::Right(recv) => {
                        let effect = SpawnEffect {
                            testkit_or_message: either::Either::Right(recv.as_ref()),
                        };
                        Some(Effect::Spawn(effect))
                    }
                }
            }
        };

        let t = handler(effect).await;

        match effect_from_actor {
            None => {}
            Some(EffectFromActorToTestkit::Recv(inner)) => {
                let effect_to_actor = RecvEffectFromTestkitToActor {
                    recv: inner.0,
                    discarded,
                };
                self.recv_effect_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            Some(EffectFromActorToTestkit::Spawn(inner)) => match inner.0 {
                either::Either::Left(testkit) => {
                    assert!(testkit.is_none(), "testkit is previously unwrapped");
                    let effect_to_actor = SpawnEffectFromTestkitToActor(either::Either::Left(()));
                    self.spawn_effect_sender
                        .try_send(effect_to_actor)
                        .expect("could not send effect back to actor");
                }
                either::Either::Right(recv) => {
                    let effect_to_actor = SpawnEffectFromTestkitToActor(either::Either::Right(recv));
                    self.spawn_effect_sender
                        .try_send(effect_to_actor)
                        .expect("could not send effect back to actor");
                }
            },
        };

        t
    }
}

/// A boxed [Testkit] which can be [downcast](AnyTestkit::downcast).
pub struct AnyTestkit(Option<Box<dyn Any + Send>>);

impl<M> From<Testkit<M>> for AnyTestkit
where
    M: Send + 'static,
{
    fn from(testkit: Testkit<M>) -> Self {
        Self(Some(Box::new(testkit)))
    }
}

impl AnyTestkit {
    /// Attempt to downcast to a concrete M-typed [Testkit].
    pub fn downcast<M: 'static>(&mut self) -> Option<Testkit<M>> {
        let any_testkit = self.0.take()?;

        match any_testkit.downcast::<Testkit<M>>() {
            Ok(m_testkit) => Some(*m_testkit),
            Err(testkit) => {
                self.0 = Some(testkit);
                None
            }
        }
    }

    pub fn downcast_unwrap<M: 'static>(&mut self) -> Testkit<M> {
        self.downcast()
            .unwrap_or_else(|| panic!("testkit is not downcastable to {}", std::any::type_name::<M>()))
    }
}

pub fn testkit<M, F, Fut, Ret>(f: F) -> (Actor<M, ActorTask<M, F, Fut, Ret, TestBounds<M>>>, Testkit<M>)
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, TestBounds<M>>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<M, TestBounds<M>>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let stop_channel = oneshot::channel::<Stop>();
    let m_channel = mpsc::channel::<M>(100);

    let guard = ActorDropGuard::new(stop_channel.0);
    let recv_effect_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M>>(1);
    let recv_effect_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M>>(1);
    let spawn_effect_actor_to_testkit_channel = mpsc::channel::<SpawnEffectFromActorToTestkit<M>>(1);
    let spawn_effect_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor<M>>(1);
    let bounds = TestBounds::new(
        recv_effect_actor_to_testkit_channel.0,
        recv_effect_testkit_to_actor_channel.1,
        spawn_effect_actor_to_testkit_channel.0,
        spawn_effect_testkit_to_actor_channel.1,
    );
    let cell = ActorCell::new(stop_channel.1, m_channel.1, bounds);

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), None);
    let testkit = Testkit::new(
        recv_effect_actor_to_testkit_channel.1,
        recv_effect_testkit_to_actor_channel.0,
        spawn_effect_actor_to_testkit_channel.1,
        spawn_effect_testkit_to_actor_channel.0,
    );

    (Actor::new(task, guard, m_ref), testkit)
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use crate::testkit::testkit;
    use std::time::Duration;
    use tokio::runtime::Handle;
    use tokio::time::timeout;
    use tracing::{info_span, Instrument};

    struct NonClone;

    #[tokio::test]
    async fn test_complex_scenario_with_slow_testkit() {
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_line_number(true)
            .with_max_level(tracing::Level::TRACE)
            .try_init();

        let (mut actor, mut tk) = testkit::<NonClone, _, _, ()>(|mut cell, _| async move {
            tracing::info!("calling recv with timeout");
            let result = timeout(Duration::from_millis(500), cell.recv()).await;
            assert!(result.is_err());

            tracing::info!("timeout expired; calling recv");
            let recv = cell.recv().await;
            recv.unwrap_message();
            tracing::info!("message received!");

            (cell, ())
        });

        let actor_handle = tokio::spawn(actor.task.run_task().instrument(info_span!("actor")));

        tracing::info!("sending message to actor");
        assert!(actor.m_ref.try_send(NonClone).is_ok());

        let _ = tk
            .test_next_effect(async |effect| {
                let effect = effect.unwrap();
                tracing::info!("first effect received; sleeping");
                tokio::time::sleep(Duration::from_millis(1000)).await;

                tracing::info!("testing the effect");
                effect.unwrap_recv().recv.unwrap_message();
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = tk
            .test_next_effect(async |effect| {
                let effect = effect.unwrap();
                tracing::info!("effect received; testing it");
                effect.unwrap_recv().recv.unwrap_message();
            })
            .instrument(info_span!("testkit"))
            .await;

        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_effect_discard() {
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_line_number(true)
            .with_max_level(tracing::Level::TRACE)
            .try_init();

        let (mut actor, mut tk) = testkit::<u32, _, _, ()>(|mut cell, _| async move {
            tracing::info!("calling recv");
            let m = cell.recv().await.unwrap_message();
            assert_eq!(m, 2);
            tracing::info!("message received = 2");

            (cell, ())
        });

        let actor_handle = tokio::task::spawn_blocking(|| {
            Handle::current().block_on(actor.task.run_task().instrument(info_span!("actor")));
        });

        tracing::info!("sending 1 to actor");
        let _ = actor.m_ref.try_send(1);

        tracing::info!("sending 2 to actor");
        let _ = actor.m_ref.try_send(2);

        let _ = tk
            .test_next_effect(async |effect| {
                let effect = effect.unwrap();
                let mut effect = effect.unwrap_recv();
                let m = effect.recv.as_ref().unwrap_message();
                assert_eq!(**m, 1);
                tracing::info!("the first effect contains 1; discarding the effect");
                effect.discard();
            })
            .instrument(info_span!("testkit"))
            .await;
        let _ = tk
            .test_next_effect(async |effect| {
                let effect = effect.unwrap();
                let effect = effect.unwrap_recv();
                let m = effect.recv.as_ref().unwrap_message();
                tracing::info!("the second effect contains 2");
                assert_eq!(**m, 2);
            })
            .instrument(info_span!("testkit"))
            .await;

        actor_handle.await.unwrap();
    }
}
