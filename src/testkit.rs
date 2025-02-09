use crate::actor::Actor;
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::test_actor::TestBounds;
use crate::actor_cell::{ActorCell, Stop};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;

use crate::effect::{
    Effect, EffectFromActorToTestkit, RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor,
    SpawnEffectFromActorToTestkit, SpawnEffectFromTestkitToActor,
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
/// async fn root<AB>(mut cell: AB, mut me: ActorRef<u64>)
/// where
///     AB: ActorBounds<u64>,
/// {
///     let m1 = cell.recv().await.message().unwrap();
///     me.try_send(m1 * 2).unwrap();
///
///     let m2 = cell.recv().await.message().unwrap();
///     debug_assert_eq!(m2, m1 * 2);
/// }
///
/// #[tokio::test]
/// async fn test() {
///     let (mut root, testkit) = testkit(root);
///     let handle = tokio::spawn(root.task.run_task());
///
///     root.m_ref.try_send(42).unwrap();
///
///     let (testkit, effect) = testkit
///         .test_next_effect(|effect| {
///             let m = effect.unwrap_message().m;
///             assert_eq!(*m, 42);
///         })
///         .await
///         .unwrap();
///
///     let (_testkit, _) = testkit
///         .test_next_effect(|effect| {
///             let m = effect.unwrap_message();
///             assert_eq!(*m, 84);
///         })
///         .await
///         .unwrap();
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
///     let child = cell.spawn(child).await.unwrap();
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
///     let (_parent_testkit, _child_testkit) =
///         parent_testkit.test_next_effect(|effect| effect.unwrap_spawn().testkit.unwrap().downcast_unwrap::<u32>());
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
    pub fn new(
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
    pub async fn test_next_effect<T>(mut self, handler: impl FnOnce(Effect<M>) -> T) -> Option<(Self, T)>
    where
        M: Send + 'static,
    {
        let select = future::select(self.recv_effect_receiver.next(), self.spawn_effect_receiver.next()).await;

        let mut effect_from_actor: EffectFromActorToTestkit<M> = match select {
            future::Either::Left((Some(inner), _)) => EffectFromActorToTestkit::Recv(inner),
            future::Either::Right((Some(inner), _)) => EffectFromActorToTestkit::Spawn(inner),
            future::Either::Left((None, _)) => {
                // The actor returns -> TestBounds is dropped at the end of the ActorTask::run_task scope
                // -> the channel is closed.
                assert!(self.recv_effect_sender.is_closed());
                return None;
            }
            future::Either::Right((None, _)) => {
                // Same reasoning.
                assert!(self.spawn_effect_sender.is_closed());
                return None;
            }
        };

        let effect = match &mut effect_from_actor {
            EffectFromActorToTestkit::Recv(inner) => Effect::Recv(inner.0.as_ref()),
            EffectFromActorToTestkit::Spawn(inner) => {
                //
                match &mut inner.0 {
                    either::Either::Left(testkit) => Effect::Spawn(either::Either::Left(testkit.take().unwrap())),
                    either::Either::Right(recv) => Effect::Spawn(either::Either::Right(recv.as_ref())),
                }
            }
        };

        let t = handler(effect);

        match effect_from_actor {
            EffectFromActorToTestkit::Recv(inner) => {
                let effect_to_actor = RecvEffectFromTestkitToActor(inner.0);
                self.recv_effect_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            EffectFromActorToTestkit::Spawn(inner) => match inner.0 {
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

        Some((self, t))
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
        self.downcast().expect(&format!(
            "testkit is not downcastable to {}",
            std::any::type_name::<M>()
        ))
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
