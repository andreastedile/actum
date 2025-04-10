use crate::actor_ref::ActorRef;
use crate::effect::create_child_effect::{
    CreateChildEffectFromTestkitToActor, UntypedCreateChildEffect, UntypedCreateChildEffectFromActorToTestkit,
    UntypedCreateChildEffectImpl,
};
use crate::effect::recv_effect::{
    RecvEffect, RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor, RecvEffectImpl,
};
use crate::effect::returned_effect::{
    ReturnedEffect, ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor, ReturnedEffectImpl,
};
use crate::effect::{Effect, EffectImpl};
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::future::poll_fn;
use std::task::Poll;

pub struct Testkit<M, Ret> {
    /// Becomes None once [ReturnedEffect] has been received.
    state: Option<TestkitState<M, Ret>>,
}

impl<M, Ret> Debug for Testkit<M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Testkit")
            .field("returned", &self.state.is_none())
            .finish_non_exhaustive()
    }
}

struct TestkitState<M, Ret> {
    recv_effect_from_actor_to_testkit_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
    recv_effect_from_testkit_to_actor_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
    create_child_effect_from_actor_to_testkit_receiver: mpsc::Receiver<UntypedCreateChildEffectFromActorToTestkit>,
    create_child_effect_from_testkit_to_actor_sender: mpsc::Sender<CreateChildEffectFromTestkitToActor>,
    returned_effect_from_actor_to_testkit_receiver: oneshot::Receiver<ReturnedEffectFromActorToTestkit<Ret>>,
    /// Wrapped in Option so that it can be taken.
    returned_effect_from_testkit_to_actor_sender: Option<oneshot::Sender<ReturnedEffectFromTestkitToActor<Ret>>>,
}

impl<M, Ret> Testkit<M, Ret> {
    pub(crate) const fn new(
        recv_effect_from_actor_to_testkit_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
        recv_effect_from_testkit_to_actor_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
        create_child_effect_from_actor_to_testkit_receiver: mpsc::Receiver<UntypedCreateChildEffectFromActorToTestkit>,
        create_child_effect_from_testkit_to_actor_sender: mpsc::Sender<CreateChildEffectFromTestkitToActor>,
        returned_effect_from_actor_to_testkit_receiver: oneshot::Receiver<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_from_testkit_to_actor_sender: oneshot::Sender<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            state: Some(TestkitState {
                recv_effect_from_actor_to_testkit_receiver,
                recv_effect_from_testkit_to_actor_sender,
                create_child_effect_from_actor_to_testkit_receiver,
                create_child_effect_from_testkit_to_actor_sender,
                returned_effect_from_actor_to_testkit_receiver,
                returned_effect_from_testkit_to_actor_sender: Some(returned_effect_from_testkit_to_actor_sender),
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
    pub async fn test_next_effect<T>(&mut self, handler: impl for<'a> AsyncFnOnce(Effect<'a, M, Ret>) -> T) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect_impl = poll_fn(|cx| {
            match state.recv_effect_from_actor_to_testkit_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => { /* MessageReceiver has dropped */ }
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(EffectImpl::Recv(RecvEffectImpl {
                        recv: effect.recv,
                        discarded: false,
                    }));
                }
                Poll::Pending => {}
            };
            match state
                .create_child_effect_from_actor_to_testkit_receiver
                .next()
                .poll_unpin(cx)
            {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(EffectImpl::CreateChild(UntypedCreateChildEffectImpl {
                        untyped_testkit: Some(effect.untyped_testkit),
                        injected: None,
                    }));
                }
                Poll::Pending => {}
            }
            match state.returned_effect_from_actor_to_testkit_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    return Poll::Ready(EffectImpl::Returned(ReturnedEffectImpl { ret: effect.ret }));
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let effect = match &mut effect_impl {
            EffectImpl::Recv(effect) => Effect::Recv(RecvEffect {
                recv: &effect.recv,
                discarded: &mut effect.discarded,
            }),
            EffectImpl::CreateChild(effect) => Effect::CreateChild(UntypedCreateChildEffect {
                untyped_testkit: effect.untyped_testkit.take().unwrap(),
                injected: &mut effect.injected,
            }),
            EffectImpl::Returned(effect) => Effect::Returned(ReturnedEffect { ret: &effect.ret }),
        };

        let t = handler(effect).await;

        match effect_impl {
            EffectImpl::Recv(inner) => {
                let effect_to_actor = RecvEffectFromTestkitToActor {
                    recv: inner.recv,
                    discarded: inner.discarded,
                };
                state
                    .recv_effect_from_testkit_to_actor_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            EffectImpl::CreateChild(inner) => {
                let effect_to_actor = CreateChildEffectFromTestkitToActor {
                    injected: inner.injected,
                };
                state
                    .create_child_effect_from_testkit_to_actor_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            EffectImpl::Returned(inner) => {
                let effect_to_actor = ReturnedEffectFromTestkitToActor { ret: inner.ret };
                state
                    .returned_effect_from_testkit_to_actor_sender
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
    ///
    /// async fn root<C, R>(mut cell: C, mut receiver: R, mut me: ActorRef<u64>) -> (C, ())
    /// where
    ///     C: CreateChild,
    ///     R: ReceiveMessage<u64>,
    /// {
    ///     let m1 = receiver.recv().await.into_message().unwrap();
    ///     me.try_send(m1 * 2).unwrap();
    ///     let m2 = receiver.recv().await.into_message().unwrap();
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
    pub async fn expect_recv_effect<T>(&mut self, handler: impl for<'a> AsyncFnOnce(RecvEffect<'a, M>) -> T) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect_impl = poll_fn(|cx| {
            match state.recv_effect_from_actor_to_testkit_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(RecvEffectImpl {
                        recv: effect.recv,
                        discarded: false,
                    });
                }
                Poll::Pending => {}
            };
            match state
                .create_child_effect_from_actor_to_testkit_receiver
                .next()
                .poll_unpin(cx)
            {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `RecvEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            match state.returned_effect_from_actor_to_testkit_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    panic!("Expected `RecvEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let effect = RecvEffect {
            recv: &effect_impl.recv,
            discarded: &mut effect_impl.discarded,
        };

        let t = handler(effect).await;

        let effect_to_actor = RecvEffectFromTestkitToActor {
            recv: effect_impl.recv,
            discarded: effect_impl.discarded,
        };
        state
            .recv_effect_from_testkit_to_actor_sender
            .try_send(effect_to_actor)
            .expect("could not send effect back to actor");

        t
    }

    /// Receives a [UntypedCreateChildEffect] from the actor under test and evaluates it with the provided closure.
    ///
    /// The closure can return a generic object, such as the [Testkit] of the child actor.
    ///
    /// # Panics
    /// If the testkit has already received the [ReturnedEffect] or the received effect is not the right type.
    ///
    /// # Examples
    /// Test whether the actor called [create_child](crate::create_child::CreateChild::create_child).
    ///
    /// # Example
    /// ```
    /// use actum::prelude::*;
    ///
    /// async fn parent<C, R>(mut cell: C, _receiver: R, _me: ActorRef<u64>) -> (C, ())
    /// where
    ///     C: CreateChild,
    ///     R: ReceiveMessage<u64>,
    /// {
    ///     let child = cell.create_child(child).await;
    ///     let handle = tokio::spawn(child.task.run_task());
    ///     handle.await.unwrap();
    ///     (cell, ())
    /// }
    ///
    /// async fn child<C, R>(mut cell: C, _receiver: R, _me: ActorRef<u32>) -> (C, ())
    /// where
    ///     C: CreateChild,
    ///     R: ReceiveMessage<u32>,
    /// {
    ///     (cell, ())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let ActumWithTestkit { task, mut actor_ref, testkit: mut parent_tk } = actum_with_testkit(parent);
    ///     let handle = tokio::spawn(task.run_task());
    ///
    ///     let mut child_tk = parent_tk.expect_create_child_effect(async |mut effect| {
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
    pub async fn expect_create_child_effect<T>(
        &mut self,
        handler: impl for<'a> AsyncFnOnce(UntypedCreateChildEffect) -> T,
    ) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect_impl = poll_fn(|cx| {
            match state.recv_effect_from_actor_to_testkit_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `CreateChildEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            };
            match state
                .create_child_effect_from_actor_to_testkit_receiver
                .next()
                .poll_unpin(cx)
            {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(UntypedCreateChildEffectImpl {
                        untyped_testkit: Some(effect.untyped_testkit),
                        injected: None,
                    });
                }
                Poll::Pending => {}
            }
            match state.returned_effect_from_actor_to_testkit_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    panic!("Expected `CreateChildEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let effect = UntypedCreateChildEffect {
            untyped_testkit: effect_impl.untyped_testkit.take().unwrap(),
            injected: &mut effect_impl.injected,
        };

        let t = handler(effect).await;

        let effect_to_actor = CreateChildEffectFromTestkitToActor {
            injected: effect_impl.injected,
        };
        state
            .create_child_effect_from_testkit_to_actor_sender
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
    ///
    /// async fn parent<C, R>(mut cell: C, _receiver: R, _me: ActorRef<u64>) -> (C, &'static str)
    /// where
    ///     C: CreateChild,
    ///     R: ReceiveMessage<u64>
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
    ///         assert_eq!(*effect.ret, "returned");
    ///     }).await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_returned_effect<T>(
        &mut self,
        handler: impl for<'a> AsyncFnOnce(ReturnedEffect<'a, Ret>) -> T,
    ) -> T
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let effect_impl = poll_fn(|cx| {
            match state.recv_effect_from_actor_to_testkit_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => { /* MessageReceiver has dropped */ }
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `ReturnedEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            };
            match state
                .create_child_effect_from_actor_to_testkit_receiver
                .next()
                .poll_unpin(cx)
            {
                Poll::Ready(None) => panic!(),
                Poll::Ready(Some(effect)) => {
                    panic!("Expected `ReturnedEffect`, received {:?}", effect);
                }
                Poll::Pending => {}
            }
            match state.returned_effect_from_actor_to_testkit_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!(),
                Poll::Ready(Ok(effect)) => {
                    return Poll::Ready(ReturnedEffectImpl { ret: effect.ret });
                }
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let effect = ReturnedEffect { ret: &effect_impl.ret };

        let t = handler(effect).await;

        let effect_to_actor = ReturnedEffectFromTestkitToActor { ret: effect_impl.ret };
        state
            .returned_effect_from_testkit_to_actor_sender
            .take()
            .unwrap()
            .send(effect_to_actor)
            .expect("could not send effect back to actor");

        self.state = None;

        t
    }
}

/// A boxed [Testkit] which can be [downcast](UntypedTestkit::downcast).
pub struct UntypedTestkit(Box<dyn Any + Send>);

impl Debug for UntypedTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("UntypedTestkit")
    }
}

impl<M, Ret> From<Testkit<M, Ret>> for UntypedTestkit
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    fn from(testkit: Testkit<M, Ret>) -> Self {
        Self(Box::new(testkit))
    }
}

impl UntypedTestkit {
    /// Attempt to downcast to a concrete typed [Testkit].
    pub fn downcast<M: 'static, Ret: 'static>(self) -> Result<Testkit<M, Ret>, Self> {
        match self.0.downcast::<Testkit<M, Ret>>() {
            Ok(testkit) => Ok(*testkit),
            Err(testkit) => Err(Self(testkit)),
        }
    }

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> Testkit<M, Ret> {
        *self.0.downcast::<Testkit<M, Ret>>().unwrap()
    }
}

pub struct ActumWithTestkit<M, RT, Ret> {
    pub task: RT,
    pub actor_ref: ActorRef<M>,
    pub testkit: Testkit<M, Ret>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actum_with_testkit;
    use crate::prelude::*;
    use futures::FutureExt;
    use std::future::poll_fn;
    use std::task::Poll;
    use std::time::Duration;
    use tracing::{Instrument, info_span};

    /// Non-cloneable type.
    /// If the actor receives it, it certainly could not have been cloned by Actum.
    struct NonClone;

    #[tokio::test]
    async fn test_slow_testkit() {
        let _ = tracing_subscriber::fmt()
            .with_span_events(
                tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
            )
            .with_target(false)
            .with_line_number(true)
            .with_max_level(tracing::Level::TRACE)
            .try_init();

        let ActumWithTestkit {
            task,
            mut actor_ref,
            mut testkit,
        } = actum_with_testkit::<NonClone, _, _, ()>(|cell, mut receiver, _| async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let mut recv_future = receiver.recv();
            poll_fn(|cx| match recv_future.poll_unpin(cx) {
                Poll::Ready(_) => panic!("the testkit should be slow"),
                Poll::Pending => Poll::Ready(()),
            })
            .await;
            drop(recv_future);

            let _ = receiver.recv().await.into_message().unwrap();
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
        } = actum_with_testkit::<u32, _, _, ()>(|cell, mut receiver, _| async move {
            let m = receiver.recv().await.into_message().unwrap();
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
            .expect_recv_effect(async |mut effect| {
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
