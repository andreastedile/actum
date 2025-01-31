use crate::actor::Actor;
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::test_actor::TestBounds;
use crate::actor_cell::{ActorCell, Stop};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;

use crate::actor_bounds::Recv;
use crate::effect::{
    EffectType, RecvMessageEffect, RecvNoMoreSendersEffect, RecvStoppedEffect, ReturnedEffect, SpawnEffect,
};
use futures::channel::{mpsc, oneshot};
use futures::future::Either;
use futures::{future, StreamExt};
use std::any::Any;
use std::future::Future;

/// Receive and test actor [effects](EffectType).
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
/// async fn parent<AB>(mut cell: AB, me: ActorRef<u64>) -> (AB, ())
/// where
///     AB: ActorBounds<u64>,
/// {
///     let child = cell.spawn(child).await.unwrap();
///     let handle = tokio::spawn(child.task.run_task());
///     handle.await.unwrap();
///     (cell, ())
/// }
///
/// async fn child<AB>(mut cell: AB, me: ActorRef<u32>) -> (AB, ())
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
///     let mut effect = parent_testkit.next().await.unwrap().spawn().unwrap();
///     let mut child_testkit = effect.testkit().unwrap().downcast::<u32>().unwrap();
///     drop(effect);
///     // Use the child testkit...
///
///     handle.await.unwrap();
/// }
/// ```
pub struct Testkit<M> {
    /// Used to send a [Recv] from the actor.
    /// See [ActorBounds::recv].
    recv_m_receiver: mpsc::Receiver<Recv<M>>,
    /// Used to send the [Recv] back to the actor from which it was received.
    recv_m_sender: mpsc::Sender<Recv<M>>,
    /// Used to receive the optional [Testkit] of a child actor from the actor.
    /// See [ActorBounds::spawn].
    testkit_receiver: mpsc::Receiver<Option<AnyTestkit>>,
    /// Used to receive a confirmation back to the actor from which the [Testkit] of the child actor was received.
    testkit_sender: mpsc::Sender<()>,
}

impl<M> Testkit<M> {
    pub fn new(
        recv_m_receiver: mpsc::Receiver<Recv<M>>,
        recv_m_sender: mpsc::Sender<Recv<M>>,
        testkit_receiver: mpsc::Receiver<Option<AnyTestkit>>,
        testkit_sender: mpsc::Sender<()>,
    ) -> Self {
        Self {
            recv_m_receiver,
            recv_m_sender,
            testkit_receiver,
            testkit_sender,
        }
    }

    /// Receive an effect from the actor and test it.
    ///
    /// To enforce that the received effect is executed without being dropped, it is made necessary to
    /// call the [execute](super::effect::EffectExecution::execute) method of the effect and return the result from
    /// the closure, as the result is not otherwise constructible by the user.
    pub async fn test_next_effect<'a, 'm, T>(&'a mut self, handler: impl FnOnce(&EffectType<'m, M>) -> T) -> T
    where
        'a: 'm,
        M: Send + 'static,
    {
        let select = future::select(self.recv_m_receiver.next(), self.testkit_receiver.next()).await;
        let effect: EffectType<M> = match select {
            Either::Left((inner, _)) => {
                //
                match inner {
                    None => EffectType::Returned(ReturnedEffect),
                    Some(Recv::Message(m)) => EffectType::Message(RecvMessageEffect::new(&mut self.recv_m_sender, m)),
                    Some(Recv::Stopped(Some(m))) => {
                        EffectType::Stopped(RecvStoppedEffect::new(&mut self.recv_m_sender, Some(m)))
                    }
                    Some(Recv::Stopped(None)) => {
                        EffectType::Stopped(RecvStoppedEffect::new(&mut self.recv_m_sender, None))
                    }
                    Some(Recv::NoMoreSenders) => {
                        EffectType::NoMoreSenders(RecvNoMoreSendersEffect::new(&mut self.recv_m_sender))
                    }
                }
            }
            Either::Right((inner, _)) => {
                //
                match inner {
                    None => EffectType::Returned(ReturnedEffect),
                    Some(None) => EffectType::Spawn(SpawnEffect::new(&mut self.testkit_sender, None)),
                    Some(Some(testkit)) => EffectType::Spawn(SpawnEffect::new(&mut self.testkit_sender, Some(testkit))),
                }
            }
        };

        let t = handler(&effect);

        match effect {
            EffectType::Stopped(inner) => {
                inner
                    .recv_effect_out_m_sender
                    .try_send(Recv::Stopped(inner.m))
                    .expect("could not send effect back to actor");
            }
            EffectType::Message(inner) => {
                inner
                    .recv_effect_out_m_sender
                    .try_send(Recv::Message(inner.m))
                    .expect("could not send effect back to actor");
            }
            EffectType::NoMoreSenders(inner) => {
                inner
                    .recv_effect_out_m_sender
                    .try_send(Recv::NoMoreSenders)
                    .expect("could not send effect back to actor");
            }
            EffectType::Spawn(inner) => {
                inner
                    .spawn_effect_out_sender
                    .try_send(())
                    .expect("could not send effect back to actor");
            }
            EffectType::Returned(_) => {}
        }

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
    let recv_m_channel_out = mpsc::channel::<Recv<M>>(1);
    let recv_m_channel_in = mpsc::channel::<Recv<M>>(1);
    let testkit_channel_out = mpsc::channel::<Option<AnyTestkit>>(1);
    let testkit_channel_in = mpsc::channel::<()>(1);
    let bounds = TestBounds::new(
        recv_m_channel_out.0,
        recv_m_channel_in.1,
        testkit_channel_out.0,
        testkit_channel_in.1,
    );
    let cell = ActorCell::new(stop_channel.1, m_channel.1, bounds);

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), None);
    let testkit = Testkit::new(
        recv_m_channel_out.1,
        recv_m_channel_in.0,
        testkit_channel_out.1,
        testkit_channel_in.0,
    );

    (Actor::new(task, guard, m_ref), testkit)
}
