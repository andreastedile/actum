use crate::actor_test::effect::create_child_effect::{
    CreateChildEffectFromTestkitToActor, UntypedCreateChildEffect, UntypedCreateChildEffectFromActorToTestkit,
    UntypedCreateChildEffectImpl,
};
use crate::actor_test::effect::recv_effect::{
    RecvEffect, RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor, RecvEffectImpl,
};
use crate::actor_test::effect::returned_effect::{
    ReturnedEffect, ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor, ReturnedEffectImpl,
};
use crate::actor_test::effect::{Effect, EffectImpl};
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
                Poll::Ready(None) => { /* ActorCell has dropped */ }
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(EffectImpl::CreateChild(UntypedCreateChildEffectImpl {
                        untyped_testkit: Some(effect.untyped_testkit),
                        injected: None,
                    }));
                }
                Poll::Pending => {}
            }
            match state.returned_effect_from_actor_to_testkit_receiver.poll_unpin(cx) {
                Poll::Ready(Err(oneshot::Canceled)) => panic!("ActorTask did not send ReturnedEffect"),
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
                let recv_effect_from_testkit_to_actor = RecvEffectFromTestkitToActor {
                    recv: inner.recv,
                    discarded: inner.discarded,
                };
                if state
                    .recv_effect_from_testkit_to_actor_sender
                    .try_send(recv_effect_from_testkit_to_actor)
                    .is_err()
                {
                    // MessageReceiver has dropped.
                    // This is possible because it is passed by value to the actor closure.
                    // This means that the actor will not see this message and
                    // will lose the ability to receive new messages.
                }
            }
            EffectImpl::CreateChild(inner) => {
                let create_child_effect_from_testkit_to_actor = CreateChildEffectFromTestkitToActor {
                    injected: inner.injected,
                };
                if state
                    .create_child_effect_from_testkit_to_actor_sender
                    .try_send(create_child_effect_from_testkit_to_actor)
                    .is_err()
                {
                    // ActorCell has dropped.
                    // This is possible because it is passed by value to the actor closure.
                    // This means that the actor will not be able to create this child actor and
                    // will lose the ability to create new child actors.
                }
            }
            EffectImpl::Returned(inner) => {
                let returned_effect_from_testkit_to_actor = ReturnedEffectFromTestkitToActor { ret: inner.ret };
                state
                    .returned_effect_from_testkit_to_actor_sender
                    .take()
                    .unwrap()
                    .send(returned_effect_from_testkit_to_actor)
                    .expect("could not send effect back to ActorTask");

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

        let recv_effect_from_testkit_to_actor = RecvEffectFromTestkitToActor {
            recv: effect_impl.recv,
            discarded: effect_impl.discarded,
        };
        state
            .recv_effect_from_testkit_to_actor_sender
            .try_send(recv_effect_from_testkit_to_actor)
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
    /// Test whether the actor called [create_child](crate::core::create_child::CreateChild::create_child).
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

        let create_child_effect_from_testkit_to_actor = CreateChildEffectFromTestkitToActor {
            injected: effect_impl.injected,
        };
        state
            .create_child_effect_from_testkit_to_actor_sender
            .try_send(create_child_effect_from_testkit_to_actor)
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

        let returned_effect_from_testkit_to_actor = ReturnedEffectFromTestkitToActor { ret: effect_impl.ret };
        state
            .returned_effect_from_testkit_to_actor_sender
            .take()
            .unwrap()
            .send(returned_effect_from_testkit_to_actor)
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

#[cfg(test)]
mod tests {
    use crate::actor_test::actum_with_testkit::ActumWithTestkit;
    use crate::actor_test::actum_with_testkit::actum_with_testkit;
    use crate::core::actor_task::RunTask;

    #[tokio::test]
    async fn test_that_state_is_set_to_none_after_the_returned_effect_is_received() {
        let ActumWithTestkit { task, mut testkit, .. } =
            actum_with_testkit::<(), _, _, u32>(|cell, _, _| async move { (cell, 42) });
        let handle = tokio::spawn(task.run_task());

        let _ = testkit
            .expect_returned_effect(async |effect| {
                assert_eq!(*effect.ret, 42);
            })
            .await;

        assert!(testkit.state.is_none());

        handle.await.unwrap();
    }
}
