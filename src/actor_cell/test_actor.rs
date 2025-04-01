use crate::actor::{Actor, Recv};
use crate::actor_cell::ActorCell;
use crate::actor_ref::MessageReceiver;
use crate::actor_ref::{create_actor_ref_and_message_receiver, ActorRef};
use crate::actor_task::{ActorInner, ActorTask};
use crate::actor_to_spawn::ActorToSpawn;
use crate::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::effect::spawn_effect::{SpawnEffectFromTestkitToActor, UntypedSpawnEffectFromActorToTestkit};
use crate::testkit::create_testkit_pair;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::future::{poll_fn, Future};
use std::marker::PhantomData;
use std::task::{ready, Poll};

pub struct TestExtension<M, Ret> {
    /// used to send recv effects from the actor under test to the corresponding testkit.
    pub(crate) recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
    /// used to receive recv effects from the testkit to the actor.
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,

    state: RecvFutureStateMachine,

    /// used to send spawn effects from the actor under test to the corresponding testkit.
    spawn_effect_sender: mpsc::Sender<UntypedSpawnEffectFromActorToTestkit>,
    /// used to receive spawn effects from the testkit to the actor.
    spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,

    /// used to send returned effects from the actor under test to the corresponding testkit.
    pub(crate) returned_effect_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
    /// used to receive returned effects from the testkit to the actor.
    pub(crate) returned_effect_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,

    _ret: PhantomData<Ret>,
}

impl<M, Ret> TestExtension<M, Ret> {
    pub(crate) const fn new(
        recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
        spawn_effect_sender: mpsc::Sender<UntypedSpawnEffectFromActorToTestkit>,
        spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,
        returned_effect_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            recv_effect_sender,
            recv_effect_receiver,
            state: RecvFutureStateMachine::S0,
            spawn_effect_sender,
            spawn_effect_receiver,
            returned_effect_sender,
            returned_effect_receiver,
            _ret: PhantomData,
        }
    }
}

/// The [Actor::recv] future can be dropped before being polled to completion; for example, when it
/// races with a short timeout.
/// This fact, and the fact that the result of the future is temporarily transferred to the testkit,
/// makes future not cancel safe.
///
/// This state machine makes the future cancel safe.
#[derive(Debug, Eq, PartialEq)]
pub enum RecvFutureStateMachine {
    /// We have called [Actor::recv] and obtained a future.
    /// We can now poll or drop it.
    ///
    /// - If we poll the future, it internally polls the [MessageReceiver::recv] future, which is cancel safe,
    /// to obtain the [Recv] value.
    /// If the latter resolves into a value, we send it to the testkit and go to S1;
    /// otherwise, we remain in S0 and arrange our future to be polled again.
    ///
    /// - If we drop the future, we remain in S0.
    /// If we create a new future afterward, we will resume from there.
    S0,
    /// The testkit has received the [Recv] value from us.
    /// We can now poll or drop our future.
    ///
    /// - If we poll the future, it attempts to receive a response with the value back from the testkit.
    /// If we receive those, we resolve our future into the value and go to S0;
    /// otherwise, we remain in S1 and arrange our future to be polled again.
    ///
    /// - If we drop the future, we go to S2.
    /// If we create a new future afterward, we will resume from there.
    S1,
    /// The testkit still has the [Recv] value we sent to it.
    /// We can now poll or drop our future.
    ///
    /// - If we poll the future, it attempts to receive a response and the value back from the testkit.
    /// If we receive those, we resend the value to the testkit and go to S1;
    /// otherwise, we remain in S2 and arrange our future to be polled again.
    /// The reason we resend the value is that the response we have just received corresponds to a
    /// future which we previously dropped and is thus now meaningless.
    /// Note that **we resend the very same value**, because the [Recv] could contain a message which
    /// we do not want to lose.
    ///
    /// - If we drop the future, we remain in S2.
    /// If we create a new future afterward, we will resume from there.
    S2,
}

pub(crate) struct UntypedTestExtension(Box<dyn Any + Send>);

impl<M, Ret> From<TestExtension<M, Ret>> for UntypedTestExtension
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    fn from(extension: TestExtension<M, Ret>) -> Self {
        Self(Box::new(extension))
    }
}

impl UntypedTestExtension {
    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> TestExtension<M, Ret> {
        *self.0.downcast::<TestExtension<M, Ret>>().unwrap()
    }
}

impl<M, Ret> Actor<M, Ret> for ActorCell<TestExtension<M, Ret>, Ret>
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    type ChildActorDependency<M2: Send + 'static, Ret2: Send + 'static> = TestExtension<M2, Ret2>;
    type ChildActor<M2: Send + 'static, Ret2: Send + 'static> = ActorCell<TestExtension<M2, Ret2>, Ret2>;
    type HasRunTask<M2, F, Fut, Ret2>
        = ActorTask<M2, ActorInner<F, M2, Ret2>, Fut, Ret2, TestExtension<M2, Ret2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<TestExtension<M2, Ret2>, Ret2>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<TestExtension<M2, Ret2>, Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    fn recv<'a>(&'a mut self, receiver: &'a mut MessageReceiver<M>) -> impl Future<Output = Recv<M>> + Send + 'a {
        if self.dependency.state == RecvFutureStateMachine::S1 {
            // If the state is S1, it means that the previous future was dropped while it was waiting for the effect to
            // come back from the testkit.
            self.dependency.state = RecvFutureStateMachine::S2;
        }

        poll_fn(|cx| {
            if self.dependency.recv_effect_sender.is_closed() {
                panic!("the testkit has dropped");
            }

            loop {
                match self.dependency.state {
                    RecvFutureStateMachine::S0 => {
                        let recv = ready!(receiver.recv().poll_unpin(cx));

                        let effect_out = RecvEffectFromActorToTestkit { recv };

                        self.dependency
                            .recv_effect_sender
                            .try_send(effect_out)
                            .expect("could not send the effect to the testkit");

                        self.dependency.state = RecvFutureStateMachine::S1;
                    }
                    RecvFutureStateMachine::S1 => {
                        let effect_in = match self.dependency.recv_effect_receiver.poll_next_unpin(cx) {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(effect_in)) => effect_in,
                            Poll::Pending => return Poll::Pending,
                        };

                        self.dependency.state = RecvFutureStateMachine::S0;

                        if !effect_in.discarded {
                            return Poll::Ready(effect_in.recv);
                        } // else: poll the channels in the next iteration
                    }
                    RecvFutureStateMachine::S2 => {
                        let effect_in = match self.dependency.recv_effect_receiver.poll_next_unpin(cx) {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(effect_in)) => effect_in,
                            Poll::Pending => return Poll::Pending,
                        };

                        let effect_out = RecvEffectFromActorToTestkit { recv: effect_in.recv };

                        self.dependency
                            .recv_effect_sender
                            .try_send(effect_out)
                            .expect("could not send the effect to the testkit");

                        self.dependency.state = RecvFutureStateMachine::S1;
                    }
                }
            }
        })
    }

    async fn create_child<M2, F, Fut, Ret2>(
        &mut self,
        f: F,
    ) -> ActorToSpawn<M2, ActorTask<M2, ActorInner<F, M2, Ret2>, Fut, Ret2, TestExtension<M2, Ret2>>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<TestExtension<M2, Ret2>, Ret2>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<TestExtension<M2, Ret2>, Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static,
    {
        assert!(!self.dependency.spawn_effect_sender.is_closed());

        let (actor_ref, receiver) = create_actor_ref_and_message_receiver::<M2>();
        let (extension, testkit) = create_testkit_pair::<M2, Ret2>();

        let cell = ActorCell::new(extension);

        let tracker = self.tracker.get_or_insert_default();
        let mut task = ActorTask::new(
            ActorInner::Unboxed(f),
            cell,
            receiver,
            actor_ref.clone(),
            Some(tracker.make_child()),
        );

        let spawn_effect_to_testkit = UntypedSpawnEffectFromActorToTestkit {
            untyped_testkit: testkit.into(),
        };

        self.dependency
            .spawn_effect_sender
            .try_send(spawn_effect_to_testkit)
            .expect("could not send the effect to the testkit");

        let spawn_effect_from_actor = self
            .dependency
            .spawn_effect_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        if let Some(inner) = spawn_effect_from_actor.injected {
            let actor = inner.actor.downcast_unwrap::<M2, Ret2>();
            let extension = inner.extension.downcast_unwrap::<M2, Ret2>();
            task.f = ActorInner::Boxed(actor);
            task.cell = ActorCell::new(extension);
        };

        ActorToSpawn::new(task, actor_ref)
    }
}
