use crate::actor_cell::test_actor::TestExtension;
use crate::actor_cell::ActorCell;
use crate::actor_ref::{create_actor_ref_and_message_receiver, ActorRef};
use crate::actor_task::ActorTask;

use crate::actor_ref::MessageReceiver;
use crate::effect::recv_effect::{RecvEffect, RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::returned_effect::{
    ReturnedEffect, ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor,
};
use crate::effect::spawn_effect::{
    SpawnEffectFromTestkitToActor, UntypedSpawnEffect, UntypedSpawnEffectFromActorToTestkit,
};
use crate::effect::Effect;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::future::{poll_fn, Future};
use std::task::Poll;

pub fn create_testkit_pair<M, Ret>() -> (TestExtension<M, Ret>, Testkit<M, Ret>) {
    let recv_effect_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M>>(1);
    let recv_effect_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M>>(1);
    let spawn_effect_actor_to_testkit_channel = mpsc::channel::<UntypedSpawnEffectFromActorToTestkit>(1);
    let spawn_effect_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor>(1);
    let returned_effect_actor_to_testkit_channel = oneshot::channel::<ReturnedEffectFromActorToTestkit<Ret>>();
    let returned_effect_testkit_to_actor_channel = oneshot::channel::<ReturnedEffectFromTestkitToActor<Ret>>();
    let extension = TestExtension::<M, Ret>::new(
        recv_effect_actor_to_testkit_channel.0,
        recv_effect_testkit_to_actor_channel.1,
        spawn_effect_actor_to_testkit_channel.0,
        spawn_effect_testkit_to_actor_channel.1,
        returned_effect_actor_to_testkit_channel.0,
        returned_effect_testkit_to_actor_channel.1,
    );
    let testkit = Testkit::new(
        recv_effect_actor_to_testkit_channel.1,
        recv_effect_testkit_to_actor_channel.0,
        spawn_effect_actor_to_testkit_channel.1,
        spawn_effect_testkit_to_actor_channel.0,
        returned_effect_actor_to_testkit_channel.1,
        returned_effect_testkit_to_actor_channel.0,
    );

    (extension, testkit)
}

pub struct Testkit<M, Ret> {
    state: Option<TestkitState<M, Ret>>,
}

impl<M, Ret> Debug for Testkit<M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Testkit")
            .field("returned", &self.state.is_none())
            .finish_non_exhaustive()
    }
}

pub struct TestkitState<M, Ret> {
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
    recv_effect_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
    spawn_effect_receiver: mpsc::Receiver<UntypedSpawnEffectFromActorToTestkit>,
    spawn_effect_sender: mpsc::Sender<SpawnEffectFromTestkitToActor>,
    returned_effect_receiver: oneshot::Receiver<ReturnedEffectFromActorToTestkit<Ret>>,
    returned_effect_sender: Option<oneshot::Sender<ReturnedEffectFromTestkitToActor<Ret>>>,
}

impl<M, Ret> Testkit<M, Ret> {
    pub const fn new(
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
        recv_effect_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
        spawn_effect_receiver: mpsc::Receiver<UntypedSpawnEffectFromActorToTestkit>,
        spawn_effect_sender: mpsc::Sender<SpawnEffectFromTestkitToActor>,
        returned_effect_receiver: oneshot::Receiver<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_sender: oneshot::Sender<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            state: Some(TestkitState {
                recv_effect_receiver,
                recv_effect_sender,
                spawn_effect_receiver,
                spawn_effect_sender,
                returned_effect_receiver,
                returned_effect_sender: Some(returned_effect_sender),
            }),
        }
    }

    /// Receives an effect from the actor under test and evaluates it with the provided closure.
    ///
    /// The closure can return a generic object, such as the [Testkit] of a child actor.
    ///
    /// # Panics
    /// If the testkit has already received the [ReturnedEffect].
    #[must_use]
    pub async fn test_next_effect<'a, T>(&'a mut self, handler: impl AsyncFnOnce(&mut Effect<M, Ret>) -> T) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect = poll_fn(|cx| {
            match state.recv_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(Effect::Recv(RecvEffect {
                        recv: effect.recv,
                        discarded: false,
                    }));
                }
                Poll::Pending => {}
            };
            match state.spawn_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(Effect::Spawn(UntypedSpawnEffect {
                        any_testkit: Some(effect.any_testkit),
                    }));
                }
                Poll::Pending => {}
            }
            match state.returned_effect_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    return Poll::Ready(Effect::Returned(ReturnedEffect { ret: effect.ret }));
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let t = handler(&mut effect).await;

        match effect {
            Effect::Recv(inner) => {
                let effect_to_actor = RecvEffectFromTestkitToActor {
                    recv: inner.recv,
                    discarded: inner.discarded,
                };
                state
                    .recv_effect_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            Effect::Spawn(_) => {
                let effect_to_actor = SpawnEffectFromTestkitToActor;
                state
                    .spawn_effect_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            Effect::Returned(inner) => {
                let effect_to_actor = ReturnedEffectFromTestkitToActor { ret: inner.ret };
                state
                    .returned_effect_sender
                    .take()
                    .unwrap()
                    .send(effect_to_actor)
                    .expect("could not send effect back to actor");

                self.state = None;
            }
        }

        t
    }

    /// Receives a [RecvEffect] from the actor under test and evaluates it with the provided closure.
    ///
    /// The closure can return a generic object.
    ///
    /// # Panics
    /// If the testkit has already received the [ReturnedEffect] or the received effect is not the right type.
    ///
    /// # Example
    /// ```
    /// use actum::prelude::*;
    /// use actum::testkit::actum_with_testkit;
    /// use actum::testkit::ActumWithTestkit;
    ///
    /// async fn root<A>(mut cell: A, mut receiver: MessageReceiver<u64>, mut me: ActorRef<u64>) -> (A, ())
    /// where
    ///     A: Actor<u64, ()>,
    /// {
    ///     let m1 = cell.recv(&mut receiver).await.into_message().unwrap();
    ///     me.try_send(m1 * 2).unwrap();
    ///     let m2 = cell.recv(&mut receiver).await.into_message().unwrap();
    ///     (cell, ())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let ActumWithTestkit { task, mut actor_ref, mut testkit } = actum_with_testkit(root);
    ///     let handle = tokio::spawn(task.run_task());
    ///
    ///     actor_ref.try_send(1).unwrap();
    ///
    ///     let _ = testkit
    ///         .expect_recv_effect(async |effect| {
    ///             let m = effect.recv.as_ref().into_message().unwrap();
    ///             assert_eq!(*m, 1);
    ///         })
    ///         .await;
    ///
    ///     let _ = testkit
    ///         .expect_recv_effect(async |effect| {
    ///             let m = effect.recv.as_ref().into_message().unwrap();
    ///             assert_eq!(*m, 2);
    ///         })
    ///         .await;
    ///
    ///     let _ = testkit.expect_returned_effect(async |_| {}).await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_recv_effect<T>(&mut self, handler: impl AsyncFnOnce(&mut RecvEffect<M>) -> T) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect = poll_fn(|cx| {
            match state.recv_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(RecvEffect {
                        recv: effect.recv,
                        discarded: false,
                    });
                }
                Poll::Pending => {}
            };
            match state.spawn_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `RecvEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            match state.returned_effect_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    panic!("Expected `RecvEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let t = handler(&mut effect).await;

        let effect_to_actor = RecvEffectFromTestkitToActor {
            recv: effect.recv,
            discarded: effect.discarded,
        };
        state
            .recv_effect_sender
            .try_send(effect_to_actor)
            .expect("could not send effect back to actor");

        t
    }

    /// Receives a [UntypedSpawnEffect] from the actor under test and evaluates it with the provided closure.
    ///
    /// The closure can return a generic object, such as the [Testkit] of the child actor.
    ///
    /// # Panics
    /// If the testkit has already received the [ReturnedEffect] or the received effect is not the right type.
    ///
    /// # Examples
    /// Test whether the actor called [spawn](crate::actor::Actor::create_child).
    ///
    /// # Example
    /// ```
    /// use actum::prelude::*;
    /// use actum::testkit::actum_with_testkit;
    /// use actum::testkit::ActumWithTestkit;
    ///
    /// async fn parent<A>(mut cell: A, _receiver: MessageReceiver<u64>, _me: ActorRef<u64>) -> (A, ())
    /// where
    ///     A: Actor<u64, ()>,
    /// {
    ///     println!("got here");
    ///     let child = cell.create_child(child).await;
    ///     tracing::trace!("there");
    ///     let handle = tokio::spawn(child.task.run_task());
    ///     handle.await.unwrap();
    ///     (cell, ())
    /// }
    ///
    /// async fn child<A>(mut cell: A, _receiver: MessageReceiver<u32>, _me: ActorRef<u32>) -> (A, ())
    /// where
    ///     A: Actor<u32, ()>,
    /// {
    ///     (cell, ())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let ActumWithTestkit { task, mut actor_ref, testkit: mut parent_tk } = actum_with_testkit(parent);
    ///     let handle = tokio::spawn(task.run_task());
    ///
    ///     let mut child_tk = parent_tk.expect_spawn_effect(async |mut effect| {
    ///         let effect = effect.downcast_unwrap::<u32, ()>();
    ///         effect.testkit
    ///     }).await;
    ///
    ///     child_tk.expect_returned_effect(async |_| {}).await;
    ///     parent_tk.expect_returned_effect(async |_| {}).await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_spawn_effect<T>(&mut self, handler: impl AsyncFnOnce(UntypedSpawnEffect) -> T) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let effect = poll_fn(|cx| {
            match state.recv_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `SpawnEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            };
            match state.spawn_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(UntypedSpawnEffect {
                        any_testkit: Some(effect.any_testkit),
                    });
                }
                Poll::Pending => {}
            }
            match state.returned_effect_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    panic!("Expected `SpawnEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let t = handler(effect).await;

        let effect_to_actor = SpawnEffectFromTestkitToActor;
        state
            .spawn_effect_sender
            .try_send(effect_to_actor)
            .expect("could not send effect back to actor");

        t
    }

    /// Receives the [ReturnedEffect] from the actor under test and evaluates it with the provided closure.
    ///
    /// The closure can return a generic object, such as the [Testkit] of the child actor.
    ///
    /// # Panics
    /// If the testkit has already received the [ReturnedEffect] or the received effect is not the right type.
    ///
    /// # Example
    /// Test whether the actor returned.
    /// ```
    /// use actum::prelude::*;
    /// use actum::testkit::actum_with_testkit;
    /// use actum::testkit::ActumWithTestkit;
    ///
    /// async fn parent<A>(mut cell: A, _receiver: MessageReceiver<u64>, _me: ActorRef<u64>) -> (A, &'static str)
    /// where
    ///     A: Actor<u64, &'static str>,
    /// {
    ///     (cell, "returned")
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let ActumWithTestkit { task, mut actor_ref, mut testkit } = actum_with_testkit(parent);
    ///     let handle = tokio::spawn(task.run_task());
    ///
    ///     testkit.expect_returned_effect(async |effect| {
    ///         assert_eq!(effect.ret, "returned");
    ///     }).await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_returned_effect<T>(&mut self, handler: impl AsyncFnOnce(&mut ReturnedEffect<Ret>) -> T) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect = poll_fn(|cx| {
            match state.recv_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `ReturnedEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            };
            match state.spawn_effect_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `ReturnedEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            match state.returned_effect_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    return Poll::Ready(ReturnedEffect { ret: effect.ret });
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let t = handler(&mut effect).await;

        let effect_to_actor = ReturnedEffectFromTestkitToActor { ret: effect.ret };
        state
            .returned_effect_sender
            .take()
            .unwrap()
            .send(effect_to_actor)
            .expect("could not send effect back to actor");

        self.state = None;

        t
    }
}

/// A boxed [Testkit] which can be [downcast](AnyTestkit::downcast).
pub struct AnyTestkit(Option<Box<dyn Any + Send>>);

impl Debug for AnyTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.0.is_none() {
            f.write_str("AnyTestkit(None)")
        } else {
            f.write_str("AnyTestkit(Some(..))")
        }
    }
}

impl<M, Ret> From<Testkit<M, Ret>> for AnyTestkit
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    fn from(testkit: Testkit<M, Ret>) -> Self {
        Self(Some(Box::new(testkit)))
    }
}

impl AnyTestkit {
    /// Attempt to downcast to a concrete M-typed [Testkit].
    pub fn downcast<M: 'static, Ret: 'static>(&mut self) -> Option<Testkit<M, Ret>> {
        let any_testkit = self.0.take()?;

        match any_testkit.downcast::<Testkit<M, Ret>>() {
            Ok(m_testkit) => Some(*m_testkit),
            Err(testkit) => {
                self.0 = Some(testkit);
                None
            }
        }
    }

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(&mut self) -> Testkit<M, Ret> {
        self.downcast().unwrap_or_else(|| {
            panic!(
                "testkit is not downcastable to Testkit<{}, {}>",
                std::any::type_name::<M>(),
                std::any::type_name::<Ret>()
            )
        })
    }
}

pub struct ActumWithTestkit<M, RT, Ret> {
    pub task: RT,
    pub actor_ref: ActorRef<M>,
    pub testkit: Testkit<M, Ret>,
}

pub fn actum_with_testkit<M, F, Fut, Ret>(
    f: F,
) -> ActumWithTestkit<M, ActorTask<M, F, Fut, Ret, TestExtension<M, Ret>>, Ret>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<TestExtension<M, Ret>, Ret>, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<TestExtension<M, Ret>, Ret>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let (actor_ref, receiver) = create_actor_ref_and_message_receiver::<M>();

    let (extension, testkit) = create_testkit_pair::<M, Ret>();

    let cell = ActorCell::<TestExtension<M, Ret>, Ret>::new(extension);
    let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), None);

    ActumWithTestkit {
        task,
        actor_ref,
        testkit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use futures::FutureExt;
    use std::future::poll_fn;
    use std::task::Poll;
    use std::time::Duration;
    use tracing::{info_span, Instrument};

    /// Non-cloneable type.
    /// If the actor receives it, it certainly could not have been cloned by Actum.
    struct NonClone;

    #[tokio::test]
    async fn test_slow_testkit() {
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_line_number(true)
            .with_max_level(tracing::Level::TRACE)
            .try_init();

        let ActumWithTestkit {
            task,
            mut actor_ref,
            mut testkit,
        } = actum_with_testkit::<NonClone, _, _, ()>(|mut cell, mut receiver, _| async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let mut recv_future = cell.recv(&mut receiver);
            poll_fn(|cx| match recv_future.poll_unpin(cx) {
                Poll::Ready(_) => panic!("the testkit should be slow"),
                Poll::Pending => Poll::Ready(()),
            })
            .await;
            drop(recv_future);

            let _ = cell.recv(&mut receiver).await.into_message().unwrap();
            tracing::info!("received NonClone");

            (cell, ())
        });

        let root_handle = tokio::spawn(task.run_task().instrument(info_span!("root")));

        // Immediately send the NonClone.
        assert!(actor_ref.try_send(NonClone).is_ok());

        let _ = testkit
            .expect_recv_effect(async |_| {
                tracing::info!("effect received; sleeping");
                tokio::time::sleep(Duration::from_millis(1000)).await;
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = testkit
            .expect_recv_effect(async |_| {
                tracing::info!("effect received");
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = testkit
            .expect_returned_effect(async |_| {})
            .instrument(info_span!("testkit"))
            .await;

        root_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_recv_effect_discard() {
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_line_number(true)
            .with_max_level(tracing::Level::TRACE)
            .try_init();

        let ActumWithTestkit {
            task,
            mut actor_ref,
            mut testkit,
        } = actum_with_testkit::<u32, _, _, ()>(|mut cell, mut receiver, _| async move {
            let m = cell.recv(&mut receiver).await.into_message().unwrap();
            assert_eq!(m, 2);
            tracing::info!("received 2");

            (cell, ())
        });

        let root_handle = tokio::spawn(task.run_task().instrument(info_span!("root")));

        // Send two messages and discard the first. Only the second can be received.

        tracing::info!("sending 1 to actor");
        let _ = actor_ref.try_send(1);

        tracing::info!("sending 2 to actor");
        let _ = actor_ref.try_send(2);

        let _ = testkit
            .expect_recv_effect(async |effect| {
                let m = effect.recv.as_ref().into_message().unwrap();
                assert_eq!(*m, 1);
                tracing::info!("discarding 1");
                effect.discard();
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = testkit
            .expect_recv_effect(async |effect| {
                let m = effect.recv.as_ref().into_message().unwrap();
                assert_eq!(*m, 2);
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = testkit
            .expect_returned_effect(async |_| {})
            .instrument(info_span!("testkit"))
            .await;

        root_handle.await.unwrap();
    }
}
