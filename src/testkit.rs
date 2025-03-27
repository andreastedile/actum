use crate::actor_cell::test_actor::TestExtension;
use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;

use crate::actor_ref::MessageReceiver;
use crate::effect::recv_effect::{RecvEffect, RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::spawn_effect::{SpawnEffect, SpawnEffectFromActorToTestkit, SpawnEffectFromTestkitToActor};
use crate::effect::Effect;
use either::Either;
use futures::channel::mpsc;
use futures::{future, StreamExt};
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::future::Future;

pub fn create_testkit_pair<M>() -> (TestExtension<M>, Testkit<M>) {
    let recv_effect_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M>>(1);
    let recv_effect_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M>>(1);
    let spawn_effect_actor_to_testkit_channel = mpsc::channel::<SpawnEffectFromActorToTestkit>(1);
    let spawn_effect_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor>(1);
    let extension = TestExtension::<M>::new(
        recv_effect_actor_to_testkit_channel.0,
        recv_effect_testkit_to_actor_channel.1,
        spawn_effect_actor_to_testkit_channel.0,
        spawn_effect_testkit_to_actor_channel.1,
    );
    let testkit = Testkit::new(
        recv_effect_actor_to_testkit_channel.1,
        recv_effect_testkit_to_actor_channel.0,
        spawn_effect_actor_to_testkit_channel.1,
        spawn_effect_testkit_to_actor_channel.0,
    );

    (extension, testkit)
}

pub struct Testkit<M> {
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
    recv_effect_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
    spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromActorToTestkit>,
    spawn_effect_sender: mpsc::Sender<SpawnEffectFromTestkitToActor>,
}

impl<M> Testkit<M> {
    pub const fn new(
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromActorToTestkit<M>>,
        recv_effect_sender: mpsc::Sender<RecvEffectFromTestkitToActor<M>>,
        spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromActorToTestkit>,
        spawn_effect_sender: mpsc::Sender<SpawnEffectFromTestkitToActor>,
    ) -> Self {
        Self {
            recv_effect_receiver,
            recv_effect_sender,
            spawn_effect_receiver,
            spawn_effect_sender,
        }
    }

    /// Receives an effect from the actor under test and evaluates it with the provided closure.
    /// If the actor has returned, the argument of the closure will be None, for all subsequent calls as well.
    ///
    /// The closure can return a generic object, such as the [Testkit] of a child actor.
    #[must_use]
    pub async fn test_next_effect<T>(&mut self, handler: impl AsyncFnOnce(Option<Effect<M>>) -> T) -> T
    where
        M: Send + 'static,
    {
        let mut effect_from_actor = self.recv_next_effect().await;

        let mut discarded = false;
        let effect = match &mut effect_from_actor {
            None => None,
            Some(Either::Left(effect)) => {
                let effect = RecvEffect {
                    recv: effect.recv.as_ref(),
                    discarded: &mut discarded,
                };
                Some(Effect::Recv(effect))
            }
            Some(Either::Right(effect)) => {
                let effect = SpawnEffect {
                    any_testkit: effect.any_testkit.take().unwrap(),
                };
                Some(Effect::Spawn(effect))
            }
        };

        let t = handler(effect).await;

        match effect_from_actor {
            None => {}
            Some(Either::Left(effect)) => {
                let effect_to_actor = RecvEffectFromTestkitToActor {
                    recv: effect.recv,
                    discarded,
                };
                self.recv_effect_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
            Some(Either::Right(effect)) => {
                assert!(effect.any_testkit.is_none(), "testkit is previously unwrapped");
                let effect_to_actor = SpawnEffectFromTestkitToActor;
                self.spawn_effect_sender
                    .try_send(effect_to_actor)
                    .expect("could not send effect back to actor");
            }
        };

        t
    }

    /// Receives a [RecvEffect] from the actor under test and evaluates it with the provided closure.
    /// If the actor has returned, the argument of the closure will be None, for all subsequent calls as well.
    ///
    /// The closure can return a generic object.
    ///
    /// # Panics
    /// If the received effect is not the right type.
    ///
    /// # Example
    /// ```
    /// use actum::prelude::*;
    /// use actum::testkit::actum_with_testkit;
    ///
    /// async fn first<A>(mut cell: A, mut receiver: MessageReceiver<u64>, mut me: ActorRef<u64>) -> (A, ())
    /// where
    ///     A: Actor<u64>,
    /// {
    ///     let m1 = cell.recv(&mut receiver).await.into_message().unwrap();
    ///     me.try_send(m1 * 2).unwrap();
    ///     let m2 = cell.recv(&mut receiver).await.into_message().unwrap();
    ///     (cell, ())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (mut first, mut testkit) = actum_with_testkit(first);
    ///     let handle = tokio::spawn(first.task.run_task());
    ///
    ///     first.actor_ref.try_send(1).unwrap();
    ///
    ///     let _ = testkit
    ///         .test_next_effect(async |effect| {
    ///             let effect = effect.unwrap();
    ///             let m = effect.into_recv().unwrap().recv.into_message().unwrap();
    ///             assert_eq!(*m, 1);
    ///         })
    ///         .await;
    ///
    ///     let _ = testkit
    ///         .test_next_effect(async |effect| {
    ///             let effect = effect.unwrap();
    ///             let m = effect.into_recv().unwrap().recv.into_message().unwrap();
    ///             assert_eq!(*m, 2);
    ///         })
    ///         .await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_recv_effect<T>(&mut self, handler: impl AsyncFnOnce(RecvEffect<M>) -> T) -> T
    where
        M: Send + 'static,
    {
        let effect_from_actor = self.recv_next_effect().await.unwrap().unwrap_left();

        let mut discarded = false;
        let effect = RecvEffect {
            recv: effect_from_actor.recv.as_ref(),
            discarded: &mut discarded,
        };

        let t = handler(effect).await;

        let effect_to_actor = RecvEffectFromTestkitToActor {
            recv: effect_from_actor.recv,
            discarded,
        };
        self.recv_effect_sender
            .try_send(effect_to_actor)
            .expect("could not send effect back to actor");

        t
    }

    /// Receives a [SpawnEffect] from the actor under test and evaluates it with the provided closure.
    /// If the actor has returned, the argument of the closure will be None, for all subsequent calls as well.
    ///
    /// The closure can return a generic object, such as the [Testkit] of the child actor.
    ///
    /// # Panics
    /// If the received effect is not the right type.
    ///
    /// # Examples
    /// Test whether the actor called [spawn](crate::actor::Actor::create_child).
    ///
    /// # Example
    /// ```
    /// use actum::prelude::*;
    /// use actum::testkit::actum_with_testkit;
    ///
    /// async fn parent<A>(mut cell: A, _receiver: MessageReceiver<u64>, _me: ActorRef<u64>) -> (A, ())
    /// where
    ///     A: Actor<u64>,
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
    ///     A: Actor<u32>,
    /// {
    ///     (cell, ())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (parent, mut parent_tk) = actum_with_testkit(parent);
    ///     let handle = tokio::spawn(parent.task.run_task());
    ///     
    ///     let mut child_tk = parent_tk.expect_spawn_effect(async |mut effect| {
    ///         let testkit = effect.any_testkit.downcast_unwrap::<u32>();
    ///         testkit
    ///     }).await;
    ///
    ///     child_tk.expect_returned().await;
    ///
    ///     handle.await.unwrap();
    /// }
    /// ```
    #[must_use]
    pub async fn expect_spawn_effect<T>(&mut self, handler: impl AsyncFnOnce(SpawnEffect) -> T) -> T
    where
        M: Send + 'static,
    {
        let mut effect_from_actor = self.recv_next_effect().await.unwrap().unwrap_right();

        let effect = SpawnEffect {
            any_testkit: effect_from_actor.any_testkit.take().unwrap(),
        };

        let t = handler(effect).await;

        assert!(
            effect_from_actor.any_testkit.is_none(),
            "testkit is previously unwrapped"
        );
        let effect_to_actor = SpawnEffectFromTestkitToActor;
        self.spawn_effect_sender
            .try_send(effect_to_actor)
            .expect("could not send effect back to actor");

        t
    }

    /// Checks that the actor has returned.
    /// If this is the case, subsequent calls are guaranteed to succeed.
    ///
    /// # Panics
    /// If the actor has not returned yet.
    pub async fn expect_returned(&mut self)
    where
        M: Send + 'static,
    {
        let effect_from_actor = self.recv_next_effect().await;
        match effect_from_actor {
            None => {}
            Some(Either::Left(effect)) => {
                panic!("expected `None`, got an `Either::Left` value: {:?}", effect);
            }
            Some(Either::Right(effect)) => {
                panic!("expected `None`, got an `Either::Right` value: {:?}", effect);
            }
        }
    }

    async fn recv_next_effect(
        &mut self,
    ) -> Option<Either<RecvEffectFromActorToTestkit<M>, SpawnEffectFromActorToTestkit>>
    where
        M: Send + 'static,
    {
        let select = future::select(self.recv_effect_receiver.next(), self.spawn_effect_receiver.next()).await;

        let effect_from_actor = match select {
            future::Either::Left((Some(effect), _)) => Some(Either::Left(effect)),
            future::Either::Right((Some(effect), _)) => Some(Either::Right(effect)),
            future::Either::Left((None, _)) => {
                // The actor has returned -> TestExtension has been dropped at the end of the ActorTask::run_task scope
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

        effect_from_actor
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

pub fn actum_with_testkit<M, F, Fut, Ret>(
    f: F,
) -> (ActorToSpawn<M, ActorTask<M, F, Fut, Ret, TestExtension<M>>>, Testkit<M>)
where
    M: Send + 'static,
    F: FnOnce(ActorCell<TestExtension<M>>, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<TestExtension<M>>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let m_channel = mpsc::channel::<M>(100);

    let (extension, testkit) = create_testkit_pair::<M>();

    let cell = ActorCell::<TestExtension<M>>::new(extension);
    let receiver = MessageReceiver::<M>::new(m_channel.1);
    let actor_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), None);

    (ActorToSpawn::new(task, actor_ref), testkit)
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

        let (mut root, mut root_tk) =
            actum_with_testkit::<NonClone, _, _, ()>(|mut cell, mut receiver, _| async move {
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

        let root_handle = tokio::spawn(root.task.run_task().instrument(info_span!("root")));

        // Immediately send the NonClone.
        assert!(root.actor_ref.try_send(NonClone).is_ok());

        let _ = root_tk
            .test_next_effect(async |effect| {
                let _ = effect.unwrap();
                tracing::info!("effect received; sleeping");
                tokio::time::sleep(Duration::from_millis(1000)).await;
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = root_tk
            .test_next_effect(async |effect| {
                let _ = effect.unwrap();
                tracing::info!("effect received");
            })
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

        let (mut root, mut root_tk) = actum_with_testkit::<u32, _, _, ()>(|mut cell, mut receiver, _| async move {
            let m = cell.recv(&mut receiver).await.into_message().unwrap();
            assert_eq!(m, 2);
            tracing::info!("received 2");

            (cell, ())
        });

        let root_handle = tokio::spawn(root.task.run_task().instrument(info_span!("root")));

        // Send two messages and discard the first. Only the second can be received.

        tracing::info!("sending 1 to actor");
        let _ = root.actor_ref.try_send(1);

        tracing::info!("sending 2 to actor");
        let _ = root.actor_ref.try_send(2);

        let _ = root_tk
            .expect_recv_effect(async |mut effect| {
                let m = effect.recv.as_ref().into_message().unwrap();
                assert_eq!(**m, 1);
                tracing::info!("discarding 1");
                effect.discard();
            })
            .instrument(info_span!("testkit"))
            .await;

        let _ = root_tk
            .expect_recv_effect(async |effect| {
                let m = effect.recv.as_ref().into_message().unwrap();
                assert_eq!(**m, 2);
            })
            .instrument(info_span!("testkit"))
            .await;

        root_handle.await.unwrap();
    }
}
