use crate::testkit::effect::Effect;
use futures::channel::mpsc;
use futures::{Stream, StreamExt};
use std::any::Any;
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
///     let mut root = testkit(root);
///     let handle = tokio::spawn(root.task.run_task());
///
///     root.m_ref.try_send(42).unwrap();
///
///     let effect = root.executor.next().await.unwrap().recv().unwrap();
///     assert_eq!(*effect.message().unwrap(), 42);
///     drop(effect);
///
///     let effect = root.executor.next().await.unwrap().recv().unwrap();
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
///     let mut parent = testkit(parent);
///     let handle = tokio::spawn(parent.task.run_task());
///
///     let mut effect = parent.executor.next().await.unwrap().spawn().unwrap();
///     let mut child = effect.executor().unwrap().downcast::<u32>().unwrap();
///     drop(effect);
///     // Use the child executor...
///
///     handle.await.unwrap();
/// }
/// ```
pub struct EffectExecutor<M> {
    effect_m_receiver: mpsc::UnboundedReceiver<Effect<M>>,
}

impl<M> Stream for EffectExecutor<M> {
    type Item = Effect<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.effect_m_receiver.poll_next_unpin(cx)
    }
}

impl<M> EffectExecutor<M> {
    pub(crate) fn new(effect_m_receiver: mpsc::UnboundedReceiver<Effect<M>>) -> Self {
        Self { effect_m_receiver }
    }
}

/// A boxed [EffectExecutor] which can be [downcast](AnyExecutor::downcast).
pub struct AnyExecutor(Option<Box<dyn Any + Send>>);

impl<M> From<EffectExecutor<M>> for AnyExecutor
where
    M: Send + 'static,
{
    fn from(executor: EffectExecutor<M>) -> Self {
        Self(Some(Box::new(executor)))
    }
}

impl AnyExecutor {
    /// Attempt to downcast to a concrete M-typed [EffectExecutor].
    pub fn downcast<M: 'static>(&mut self) -> Option<EffectExecutor<M>> {
        let any_executor = self.0.take()?;

        match any_executor.downcast::<EffectExecutor<M>>() {
            Ok(m_executor) => Some(*m_executor),
            Err(testkit) => {
                self.0 = Some(testkit);
                None
            }
        }
    }
}
