use crate::actor_ref::ActorRef;
use crate::actor_task::{ExtensibleActorTask, RunTask};
use crate::actor_to_spawn::ActorToSpawn;
use crate::create_child::{ActorCell, CreateChild};
use crate::effect::create_child_effect::{
    CreateChildEffectFromTestkitToActor, UntypedCreateChildEffectFromActorToTestkit,
};
use crate::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::prelude::{ReceiveMessage, Recv};
use crate::receive_message::ExtendableMessageReceiver;
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::future::{Future, poll_fn};
use std::marker::PhantomData;
use std::task::{Poll, ready};

pub struct ActorCellTestkitExtension {
    /// used to send "create child effects" from the actor under test to the corresponding testkit.
    create_child_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedCreateChildEffectFromActorToTestkit>,
    /// used to receive "create child effects" from the testkit to the actor.
    create_child_effect_from_testkit_to_actor_receiver: mpsc::Receiver<CreateChildEffectFromTestkitToActor>,
}

impl ActorCellTestkitExtension {
    pub(crate) const fn new(
        create_child_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedCreateChildEffectFromActorToTestkit>,
        create_child_effect_from_testkit_to_actor_receiver: mpsc::Receiver<CreateChildEffectFromTestkitToActor>,
    ) -> Self {
        Self {
            create_child_effect_from_actor_to_testkit_sender,
            create_child_effect_from_testkit_to_actor_receiver,
        }
    }
}

impl CreateChild for ActorCell<ActorCellTestkitExtension> {
    type MessageReceiverT<M>
        = MessageReceiverWithTestkitExtension<M>
    where
        M: Send + 'static;
    type HasRunTask<M, F, Fut, Ret>
        = ExtensibleActorTask<
        M,
        ActorInner<F, M, Ret>,
        Fut,
        Ret,
        ActorCellTestkitExtension,
        MessageReceiverTestkitExtension<M>,
        ActorTaskTestkitExtension<Ret>,
    >
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiverWithTestkitExtension<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;

    async fn create_child<M, F, Fut, Ret>(&mut self, f: F) -> ActorToSpawn<M, Self::HasRunTask<M, F, Fut, Ret>>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiverWithTestkitExtension<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        let recv_effect_from_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M>>(1);
        let recv_effect_from_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M>>(1);
        let create_child_effect_from_actor_to_testkit_channel =
            mpsc::channel::<UntypedCreateChildEffectFromActorToTestkit>(1);
        let create_child_effect_from_testkit_to_actor_channel = mpsc::channel::<CreateChildEffectFromTestkitToActor>(1);
        let returned_effect_from_actor_to_testkit_channel = oneshot::channel::<ReturnedEffectFromActorToTestkit<Ret>>();
        let returned_effect_from_testkit_to_actor_channel = oneshot::channel::<ReturnedEffectFromTestkitToActor<Ret>>();

        let m_channel = mpsc::channel::<M>(100);
        let actor_ref = ActorRef::new(m_channel.0);
        let receiver = ExtendableMessageReceiver::new(
            m_channel.1,
            MessageReceiverTestkitExtension::new(
                recv_effect_from_actor_to_testkit_channel.0,
                recv_effect_from_testkit_to_actor_channel.1,
            ),
        );

        let cell = Self::new(ActorCellTestkitExtension::new(
            create_child_effect_from_actor_to_testkit_channel.0,
            create_child_effect_from_testkit_to_actor_channel.1,
        ));

        let extension = ActorTaskTestkitExtension::new(
            returned_effect_from_actor_to_testkit_channel.0,
            returned_effect_from_testkit_to_actor_channel.1,
        );

        let testkit = Testkit::new(
            recv_effect_from_actor_to_testkit_channel.1,
            recv_effect_from_testkit_to_actor_channel.0,
            create_child_effect_from_actor_to_testkit_channel.1,
            create_child_effect_from_testkit_to_actor_channel.0,
            returned_effect_from_actor_to_testkit_channel.1,
            returned_effect_from_testkit_to_actor_channel.0,
        );

        let tracker = self.tracker.get_or_insert_default();

        let mut task = ExtensibleActorTask::new(
            ActorInner::Unboxed(f),
            cell,
            receiver,
            actor_ref.clone(),
            extension,
            Some(tracker.make_child()),
        );

        let create_child_effect_from_actor_to_testkit = UntypedCreateChildEffectFromActorToTestkit {
            untyped_testkit: testkit.into(),
        };

        self.dependency
            .create_child_effect_from_actor_to_testkit_sender
            .try_send(create_child_effect_from_actor_to_testkit)
            .expect("could not send the effect to the testkit");

        let create_child_effect_from_testkit_to_actor = self
            .dependency
            .create_child_effect_from_testkit_to_actor_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        if let Some(inner) = create_child_effect_from_testkit_to_actor.injected {
            let actor = inner.downcast_unwrap::<M, Ret>();
            task.f = ActorInner::Boxed(actor);
        };

        ActorToSpawn::new(task, actor_ref)
    }
}

pub enum ActorInner<F, M, Ret> {
    Unboxed(F),
    Boxed(BoxTestActor<M, Ret>),
}

impl<M, F, Fut, Ret> RunTask<Ret>
    for ExtensibleActorTask<
        M,
        ActorInner<F, M, Ret>,
        Fut,
        Ret,
        ActorCellTestkitExtension,
        MessageReceiverTestkitExtension<M>,
        ActorTaskTestkitExtension<Ret>,
    >
where
    M: Send + 'static,
    F: FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiverWithTestkitExtension<M>, ActorRef<M>) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = (ActorCell<ActorCellTestkitExtension>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let (mut cell, ret) = match f {
            ActorInner::Unboxed(f) => {
                //
                let fut = f(self.cell, self.receiver, self.actor_ref);
                fut.await
            }
            ActorInner::Boxed(f) => {
                //
                let fut = f(self.cell, self.receiver, self.actor_ref);
                fut.await
            }
        };

        if let Some(mut tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        let returned_effect_from_actor_to_testkit = ReturnedEffectFromActorToTestkit { ret };
        self.dependency
            .returned_effect_from_actor_to_testkit_sender
            .send(returned_effect_from_actor_to_testkit)
            .expect("could not send the effect to the testkit");

        let returned_effect_from_testkit_to_actor = self
            .dependency
            .returned_effect_from_testkit_to_actor_receiver
            .await
            .expect("could not receive effect back from the testkit");

        returned_effect_from_testkit_to_actor.ret
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Ret> =
    Box<dyn FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiverWithTestkitExtension<M>, ActorRef<M>) -> BoxFuture<'static, (ActorCell<ActorCellTestkitExtension>, Ret)> + Send + 'static>;

pub(crate) struct UntypedBoxTestActor(Box<dyn Any + Send>);

impl Debug for UntypedBoxTestActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("UntypedBoxTestActor")
    }
}

impl<M, Ret> From<BoxTestActor<M, Ret>> for UntypedBoxTestActor
where
    M: 'static,
    Ret: 'static,
{
    fn from(actor: BoxTestActor<M, Ret>) -> Self {
        Self(Box::new(actor))
    }
}

impl UntypedBoxTestActor {
    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> BoxTestActor<M, Ret> {
        self.0.downcast::<BoxTestActor<M, Ret>>().unwrap()
    }
}

pub struct ActorTaskTestkitExtension<Ret> {
    /// used to send returned effects from the actor under test to the corresponding testkit.
    returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
    /// used to receive returned effects from the testkit to the actor.
    returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,

    _ret: PhantomData<Ret>,
}

impl<Ret> ActorTaskTestkitExtension<Ret> {
    pub(crate) const fn new(
        returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            returned_effect_from_actor_to_testkit_sender,
            returned_effect_from_testkit_to_actor_receiver,
            _ret: PhantomData,
        }
    }
}

pub type MessageReceiverWithTestkitExtension<M> = ExtendableMessageReceiver<M, MessageReceiverTestkitExtension<M>>;

pub struct MessageReceiverTestkitExtension<M> {
    /// used to send recv effects from the actor under test to the corresponding testkit.
    recv_effect_from_actor_to_testkit_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
    /// used to receive recv effects from the testkit to the actor.
    recv_effect_from_testkit_to_actor_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,

    state: RecvFutureStateMachine,
}

impl<M> MessageReceiverTestkitExtension<M> {
    pub(crate) const fn new(
        recv_effect_from_actor_to_testkit_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
        recv_effect_from_testkit_to_actor_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
    ) -> Self {
        Self {
            recv_effect_from_actor_to_testkit_sender,
            recv_effect_from_testkit_to_actor_receiver,
            state: RecvFutureStateMachine::S0,
        }
    }
}

/// The future returned by the [recv](crate::receive_message::ReceiveMessage::recv) method
/// can be dropped before being polled to completion; for example, when it races with a timeout.
/// This fact, and the fact that the result of the future is temporarily transferred to the testkit,
/// makes future not cancel safe.
#[derive(Debug, Eq, PartialEq)]
pub enum RecvFutureStateMachine {
    /// We have called [crate::actor::Actor::recv] and obtained a future.
    /// We can now poll or drop it.
    ///
    /// - If we poll the future, it internally polls the [ExtendableMessageReceiver::recv] future, which is cancel safe,
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

impl<M> ReceiveMessage<M> for MessageReceiverWithTestkitExtension<M>
where
    M: Send + 'static,
{
    fn recv<'a>(&'a mut self) -> impl Future<Output = Recv<M>> + 'a {
        if self.dependency.state == RecvFutureStateMachine::S1 {
            // If the state is S1, it means that the previous future was dropped while it was waiting for the effect to
            // come back from the testkit.
            self.dependency.state = RecvFutureStateMachine::S2;
        }

        poll_fn(|cx| {
            loop {
                match self.dependency.state {
                    RecvFutureStateMachine::S0 => {
                        let m = ready!(self.m_receiver.next().poll_unpin(cx));
                        let recv = if let Some(m) = m {
                            Recv::Message(m)
                        } else {
                            Recv::NoMoreSenders
                        };

                        let recv_effect_from_actor_to_testkit = RecvEffectFromActorToTestkit { recv };

                        self.dependency
                            .recv_effect_from_actor_to_testkit_sender
                            .try_send(recv_effect_from_actor_to_testkit)
                            .expect("could not send the effect to the testkit");

                        self.dependency.state = RecvFutureStateMachine::S1;
                    }
                    RecvFutureStateMachine::S1 => {
                        let recv_effect_from_testkit_to_actor = match self
                            .dependency
                            .recv_effect_from_testkit_to_actor_receiver
                            .poll_next_unpin(cx)
                        {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(inner)) => inner,
                            Poll::Pending => return Poll::Pending,
                        };

                        self.dependency.state = RecvFutureStateMachine::S0;

                        if !recv_effect_from_testkit_to_actor.discarded {
                            return Poll::Ready(recv_effect_from_testkit_to_actor.recv);
                        } // else: poll the channels in the next iteration
                    }
                    RecvFutureStateMachine::S2 => {
                        let recv_effect_from_testkit_to_actor = match self
                            .dependency
                            .recv_effect_from_testkit_to_actor_receiver
                            .poll_next_unpin(cx)
                        {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(inner)) => inner,
                            Poll::Pending => return Poll::Pending,
                        };

                        let recv_effect_from_actor_to_testkit = RecvEffectFromActorToTestkit {
                            recv: recv_effect_from_testkit_to_actor.recv,
                        };

                        self.dependency
                            .recv_effect_from_actor_to_testkit_sender
                            .try_send(recv_effect_from_actor_to_testkit)
                            .expect("could not send the effect to the testkit");

                        self.dependency.state = RecvFutureStateMachine::S1;
                    }
                }
            }
        })
    }
}
