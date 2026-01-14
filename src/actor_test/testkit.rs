use crate::actor_test::effect::create_child_effect::{
    CreateChildEffectToActor, UntypedCreateChildEffect, UntypedCreateChildEffectPrivate,
    UntypedCreateChildEffectToTestkit,
};
use crate::actor_test::effect::recv_effect::{RecvEffect, RecvEffectPrivate, RecvEffectToActor, RecvEffectToTestkit};
use crate::actor_test::effect::returned_effect::{
    ReturnedEffect, ReturnedEffectPrivate, ReturnedEffectToActor, ReturnedEffectToTestkit,
};
use crate::actor_test::effect::{Effect, EffectPrivate};
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::future::poll_fn;
use std::task::Poll;

pub struct Testkit<M, Output> {
    /// Becomes None once [ReturnedEffect] has been received.
    state: Option<TestkitState<M, Output>>,
}

impl<M, Output> Debug for Testkit<M, Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Testkit")
            .field("returned", &self.state.is_none())
            .finish_non_exhaustive()
    }
}

struct TestkitState<M, Output> {
    recv_effect_to_testkit_receiver: mpsc::Receiver<RecvEffectToTestkit<M>>,
    recv_effect_to_actor_sender: mpsc::Sender<RecvEffectToActor<M>>,
    create_child_to_testkit_receiver: mpsc::Receiver<UntypedCreateChildEffectToTestkit>,
    create_child_effect_to_actor_sender: mpsc::Sender<CreateChildEffectToActor>,
    returned_effect_to_testkit_receiver: oneshot::Receiver<ReturnedEffectToTestkit<Output>>,
    /// Wrapped in Option so that it can be taken.
    returned_effect_to_actor_sender: Option<oneshot::Sender<ReturnedEffectToActor<Output>>>,
}

impl<M, Output> Testkit<M, Output> {
    pub(crate) const fn new(
        recv_effect_to_testkit_receiver: mpsc::Receiver<RecvEffectToTestkit<M>>,
        recv_effect_to_actor_sender: mpsc::Sender<RecvEffectToActor<M>>,
        create_child_to_testkit_receiver: mpsc::Receiver<UntypedCreateChildEffectToTestkit>,
        create_child_effect_to_actor_sender: mpsc::Sender<CreateChildEffectToActor>,
        returned_effect_to_testkit_receiver: oneshot::Receiver<ReturnedEffectToTestkit<Output>>,
        returned_effect_to_actor_sender: oneshot::Sender<ReturnedEffectToActor<Output>>,
    ) -> Self {
        Self {
            state: Some(TestkitState {
                recv_effect_to_testkit_receiver,
                recv_effect_to_actor_sender,
                create_child_to_testkit_receiver,
                create_child_effect_to_actor_sender,
                returned_effect_to_testkit_receiver,
                returned_effect_to_actor_sender: Some(returned_effect_to_actor_sender),
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
    pub async fn test_next_effect<T>(&mut self, handler: impl for<'a> AsyncFnOnce(Effect<'a, M, Output>) -> T) -> T
    where
        M: Send + 'static,
        Output: Send + 'static,
    {
        let state = self.state.as_mut().unwrap();

        let mut effect_private = poll_fn(|cx| {
            match state.recv_effect_to_testkit_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => {
                    // this kind of effect comes from within the actor's receiver:
                    // therefore, this case implies that the receiver has been dropped.
                }
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(EffectPrivate::Recv(RecvEffectPrivate {
                        recv: effect.recv,
                        discarded: false,
                    }));
                }
                Poll::Pending => {}
            };
            match state.create_child_to_testkit_receiver.next().poll_unpin(cx) {
                Poll::Ready(None) => {
                    // this kind of effect comes from within the actor's cell:
                    // therefore, this case implies that the cell has been dropped.
                }
                Poll::Ready(Some(effect)) => {
                    return Poll::Ready(EffectPrivate::CreateChild(UntypedCreateChildEffectPrivate {
                        untyped_testkit: Some(effect.untyped_testkit),
                        injected: None,
                    }));
                }
                Poll::Pending => {}
            }
            match state.returned_effect_to_testkit_receiver.poll_unpin(cx) {
                Poll::Ready(Ok(effect)) => {
                    return Poll::Ready(EffectPrivate::Returned(ReturnedEffectPrivate { output: effect.output }));
                }
                Poll::Ready(Err(oneshot::Canceled)) => panic!("scoped actor task did not send the returned effect"),
                Poll::Pending => {}
            }
            Poll::Pending
        })
        .await;

        let effect = match &mut effect_private {
            EffectPrivate::Recv(effect) => Effect::Recv(RecvEffect {
                recv: &effect.recv,
                discarded: &mut effect.discarded,
            }),
            EffectPrivate::CreateChild(effect) => Effect::CreateChild(UntypedCreateChildEffect {
                untyped_testkit: effect.untyped_testkit.take().unwrap(),
                injected: &mut effect.injected,
            }),
            EffectPrivate::Returned(effect) => Effect::Returned(ReturnedEffect { output: &effect.output }),
        };

        let t = handler(effect).await;

        match effect_private {
            EffectPrivate::Recv(inner) => {
                let recv_effect_to_actor = RecvEffectToActor {
                    recv: inner.recv,
                    discarded: inner.discarded,
                };
                if state
                    .recv_effect_to_actor_sender
                    .try_send(recv_effect_to_actor)
                    .is_err()
                {
                    // The receiver of the actor under test has been dropped.
                }
            }
            EffectPrivate::CreateChild(inner) => {
                let create_child_effect_to_actor = CreateChildEffectToActor {
                    injected: inner.injected,
                };
                if state
                    .create_child_effect_to_actor_sender
                    .try_send(create_child_effect_to_actor)
                    .is_err()
                {
                    // The cell of the actor under test has been dropped.
                }
            }
            EffectPrivate::Returned(inner) => {
                let returned_effect_to_actor = ReturnedEffectToActor { output: inner.output };
                state
                    .returned_effect_to_actor_sender
                    .take()
                    .unwrap()
                    .send(returned_effect_to_actor)
                    .expect("could not send the returned effect back to scoped actor task");

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
    ///     let handle = tokio::spawn(task);
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
        Output: Send + 'static,
    {
        self.test_next_effect(async |effect| {
            //
            if let Effect::Recv(effect) = effect {
                handler(effect).await
            } else {
                panic!("unexpected effect: {:?}", effect);
            }
        })
        .await
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
    ///     let handle = tokio::spawn(child.task);
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
    ///     let handle = tokio::spawn(task);
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
        Output: Send + 'static,
    {
        self.test_next_effect(async |effect| {
            //
            if let Effect::CreateChild(effect) = effect {
                handler(effect).await
            } else {
                panic!("unexpected effect: {:?}", effect);
            }
        })
        .await
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
    ///     let handle = tokio::spawn(task);
    ///
    ///     testkit.expect_returned_effect(async |effect| {
    ///         assert_eq!(*effect.output, "returned");
    ///     }).await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_returned_effect<T>(
        &mut self,
        handler: impl for<'a> AsyncFnOnce(ReturnedEffect<'a, Output>) -> T,
    ) -> T
    where
        M: Send + 'static,
        Output: Send + 'static,
    {
        let t = self
            .test_next_effect(async |effect| {
                //
                if let Effect::Returned(effect) = effect {
                    handler(effect).await
                } else {
                    panic!("unexpected effect: {:?}", effect);
                }
            })
            .await;
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

impl<M, Output> From<Testkit<M, Output>> for UntypedTestkit
where
    M: Send + 'static,
    Output: Send + 'static,
{
    fn from(testkit: Testkit<M, Output>) -> Self {
        Self(Box::new(testkit))
    }
}

impl UntypedTestkit {
    /// Attempt to downcast to a concrete typed [Testkit].
    pub fn downcast<M: 'static, Output: 'static>(self) -> Result<Testkit<M, Output>, Self> {
        match self.0.downcast::<Testkit<M, Output>>() {
            Ok(testkit) => Ok(*testkit),
            Err(testkit) => Err(Self(testkit)),
        }
    }

    pub fn downcast_unwrap<M: 'static, Output: 'static>(self) -> Testkit<M, Output> {
        *self.0.downcast::<Testkit<M, Output>>().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::actor_test::actum_with_testkit::ActumWithTestkit;
    use crate::actor_test::actum_with_testkit::actum_with_testkit;

    #[tokio::test]
    async fn test_that_state_is_set_to_none_after_the_returned_effect_is_received() {
        let ActumWithTestkit { task, mut testkit, .. } =
            actum_with_testkit::<(), _, _, u32>(|cell, _, _| async move { (cell, 42) });
        let handle = tokio::spawn(task);

        let _ = testkit
            .expect_returned_effect(async |effect| {
                assert_eq!(*effect.output, 42);
            })
            .await;

        assert!(testkit.state.is_none());

        handle.await.unwrap();
    }
}
