use crate::actor::Actor;
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::test_actor::TestBounds;
use crate::actor_cell::{ActorCell, Stop};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::effect::recv::{RecvEffect, RecvEffectOut};
use crate::effect::spawn::{SpawnEffect, SpawnEffectOut};
use crate::effect::Effect;

use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, Stream};
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Receive and test actor [effects](Effect).
///
/// # Examples
///
/// Test whether the actor called [recv](crate::actor_bounds::ActorBounds::recv).
///
/// ```
/// use futures::StreamExt;
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
///     let (mut root, mut testkit) = testkit(root);
///     let handle = tokio::spawn(root.task.run_task());
///
///     root.m_ref.try_send(42).unwrap();
///
///     let effect = testkit.next().await.unwrap().recv().unwrap();
///     assert!(effect.as_ref().is_message_and(|m | *m == 42));
///     drop(effect);
///
///     let effect = testkit.next().await.unwrap().recv().unwrap();
///     assert!(effect.as_ref().is_message_and(|m | *m == 84));
///     drop(effect);
///
///     handle.await.unwrap();
/// }
/// ```
///
/// Test whether the actor called [spawn](crate::actor_bounds::ActorBounds::spawn).
///
/// # Example
/// ```
/// use futures::StreamExt;
/// use actum::prelude::*;
/// use actum::testkit::testkit;
///
/// async fn parent<AB>(mut cell: AB, me: ActorRef<u64>)
/// where
///     AB: ActorBounds<u64>,
/// {
///     let child = cell.spawn(child).await.unwrap();
///     let handle = tokio::spawn(child.task.run_task());
///     handle.await.unwrap();
/// }
///
/// async fn child<AB>(mut cell: AB, me: ActorRef<u32>)
/// where
///     AB: ActorBounds<u32>,
/// {
///     println!("child");
/// }
///
/// #[tokio::test]
/// async fn test() {
///     let (parent, mut parent_testkit) = testkit(parent);
///     let handle = tokio::spawn(parent.task.run_task());
///
///     let mut effect = parent_testkit.next().await.unwrap().spawn().unwrap();
///     let mut child_testkit = effect.testkit().unwrap().downcast::<u32>().unwrap();
///     drop(effect);
///     // Use the child testkit...
///
///     handle.await.unwrap();
/// }
/// ```
pub struct Testkit<M> {
    recv_effect_out_m_receiver: oneshot::Receiver<RecvEffectOut<M>>,
    spawn_effect_out_receiver: oneshot::Receiver<SpawnEffectOut>,
}

impl<M> Stream for Testkit<M> {
    type Item = Effect<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.recv_effect_out_m_receiver.poll_unpin(cx) {
            Poll::Ready(Ok(effect)) => {
                let recv_effect_out_m_channel = oneshot::channel::<RecvEffectOut<M>>();
                self.recv_effect_out_m_receiver = recv_effect_out_m_channel.1;
                let effect = RecvEffect::new(effect.recv, recv_effect_out_m_channel.0, effect.recv_effect_in_m_sender);
                return Poll::Ready(Some(Effect::Recv(effect)));
            }
            Poll::Ready(Err(_)) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        match self.spawn_effect_out_receiver.poll_unpin(cx) {
            Poll::Ready(Ok(mut effect)) => {
                let spawn_effect_out_channel = oneshot::channel::<SpawnEffectOut>();
                self.spawn_effect_out_receiver = spawn_effect_out_channel.1;
                let effect = SpawnEffect::new(
                    effect.testkit.take(),
                    spawn_effect_out_channel.0,
                    effect.spawn_effect_in_sender,
                );
                return Poll::Ready(Some(Effect::Spawn(effect)));
            }
            Poll::Ready(Err(_)) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        Poll::Pending
    }
}

impl<M> Testkit<M> {
    pub(crate) fn new(
        recv_effect_out_m_receiver: oneshot::Receiver<RecvEffectOut<M>>,
        spawn_effect_out_receiver: oneshot::Receiver<SpawnEffectOut>,
    ) -> Self {
        Self {
            recv_effect_out_m_receiver,
            spawn_effect_out_receiver,
        }
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
}

pub fn testkit<M, F, Fut>(f: F) -> (Actor<M, ActorTask<M, F, Fut, TestBounds<M>>>, Testkit<M>)
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, TestBounds<M>>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = ActorCell<M, TestBounds<M>>> + Send + 'static,
{
    let stop_channel = oneshot::channel::<Stop>();
    let m_channel = mpsc::channel::<M>(100);
    let recv_effect_out_m_channel = oneshot::channel::<RecvEffectOut<M>>();
    let spawn_effect_out_channel = oneshot::channel::<SpawnEffectOut>();

    let guard = ActorDropGuard::new(stop_channel.0);
    let bounds = TestBounds::new(recv_effect_out_m_channel.0, spawn_effect_out_channel.0);
    let cell = ActorCell::new(stop_channel.1, m_channel.1, bounds);

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), None);
    let testkit = Testkit::new(recv_effect_out_m_channel.1, spawn_effect_out_channel.1);

    (Actor::new(task, guard, m_ref), testkit)
}
