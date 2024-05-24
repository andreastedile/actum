use crate::actor::Actor;
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::test_actor::TestBounds;
use crate::actor_cell::{ActorCell, Stop, Stopped};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::effect::Effect;

use futures::channel::{mpsc, oneshot};
use futures::{Stream, StreamExt};
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
///     let m1 = cell.recv().await.unwrap();
///     me.try_send(m1 * 2).unwrap();
///
///     let m2 = cell.recv().await.unwrap();
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
///     assert_eq!(*effect.message().unwrap(), 42);
///     drop(effect);
///
///     let effect = testkit.next().await.unwrap().recv().unwrap();
///     assert_eq!(*effect.message().unwrap(), 84);
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
pub struct Testkit<M>(mpsc::UnboundedReceiver<Effect<M>>);

impl<M> Stream for Testkit<M> {
    type Item = Effect<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

impl<M> Testkit<M> {
    pub(crate) fn new(effect_m_receiver: mpsc::UnboundedReceiver<Effect<M>>) -> Self {
        Self(effect_m_receiver)
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
    Fut: Future<Output = ()> + Send + 'static,
{
    let stop_channel = oneshot::channel::<Stop>();
    let stopped_channel = mpsc::unbounded::<Stopped>();
    let m_channel = mpsc::channel::<M>(100);
    let effect_m_channel = mpsc::unbounded::<Effect<M>>();

    let guard = ActorDropGuard::new(stop_channel.0);
    let bounds = TestBounds::new(effect_m_channel.0);
    let cell = ActorCell::new(stop_channel.1, stopped_channel.0, m_channel.1, bounds);

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), stopped_channel.1, None);
    let testkit = Testkit::new(effect_m_channel.1);

    (Actor::new(task, guard, m_ref), testkit)
}
