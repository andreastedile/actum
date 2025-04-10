use crate::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::receive_message::{ReceiveMessage, Recv};
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::future::{poll_fn, Future};
use std::task::{ready, Poll};

pub struct ActorRef<M> {
    m_sender: mpsc::Sender<M>,
}

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        Self {
            m_sender: self.m_sender.clone(),
        }
    }
}

impl<M> PartialEq<Self> for ActorRef<M> {
    fn eq(&self, other: &Self) -> bool {
        self.m_sender.same_receiver(&other.m_sender)
    }
}

impl<M> Eq for ActorRef<M> {}

impl<M> ActorRef<M> {
    pub(crate) const fn new(m_sender: mpsc::Sender<M>) -> Self {
        Self { m_sender }
    }

    /// Attempt to send a message to this actor, returning the message if there was an error.
    pub fn try_send(&mut self, message: M) -> Result<(), M> {
        self.m_sender.try_send(message).map_err(mpsc::TrySendError::into_inner)
    }
}

pub struct ExtendableMessageReceiver<M, D> {
    m_receiver: mpsc::Receiver<M>,
    dependency: D,
}

impl<M> ReceiveMessage<M> for ExtendableMessageReceiver<M, ()>
where
    M: Send + 'static,
{
    fn recv(&mut self) -> impl Future<Output = Recv<M>> + '_ {
        poll_fn(|cx| match self.m_receiver.poll_next_unpin(cx) {
            Poll::Ready(None) => Poll::Ready(Recv::NoMoreSenders),
            Poll::Ready(Some(m)) => Poll::Ready(Recv::Message(m)),
            Poll::Pending => Poll::Pending,
        })
    }
}

impl<M> ExtendableMessageReceiver<M, ()> {
    pub(crate) const fn new(m_receiver: mpsc::Receiver<M>) -> Self {
        Self {
            m_receiver,
            dependency: (),
        }
    }
}

pub type MessageReceiverWithTestkitExtension<M> = ExtendableMessageReceiver<M, MessageReceiverTestkitExtension<M>>;

pub struct MessageReceiverTestkitExtension<M> {
    /// used to send recv effects from the actor under test to the corresponding testkit.
    recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
    /// used to receive recv effects from the testkit to the actor.
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,

    state: RecvFutureStateMachine,
}

pub(crate) fn create_actor_ref_and_message_receiver<M>() -> (ActorRef<M>, ExtendableMessageReceiver<M, ()>) {
    let (m_sender, m_receiver) = mpsc::channel::<M>(100);
    let actor_ref = ActorRef::<M>::new(m_sender);
    let receiver = ExtendableMessageReceiver::<M, ()>::new(m_receiver);
    (actor_ref, receiver)
}

/// The [crate::actor::Actor::recv] future can be dropped before being polled to completion; for example, when it
/// races with a short timeout.
/// This fact, and the fact that the result of the future is temporarily transferred to the testkit,
/// makes future not cancel safe.
///
/// This state machine makes the future cancel safe.
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

impl<M> MessageReceiverWithTestkitExtension<M> {
    pub(crate) const fn new(
        m_receiver: mpsc::Receiver<M>,
        recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
    ) -> Self {
        Self {
            m_receiver,
            dependency: MessageReceiverTestkitExtension {
                recv_effect_sender,
                recv_effect_receiver,
                state: RecvFutureStateMachine::S0,
            },
        }
    }
}

impl<M> ReceiveMessage<M> for MessageReceiverWithTestkitExtension<M>
where
    M: Send + 'static,
{
    fn recv<'a>(&'a mut self) -> impl Future<Output = Recv<M>> + 'a {
        let dependency = &mut self.dependency;

        if dependency.state == RecvFutureStateMachine::S1 {
            // If the state is S1, it means that the previous future was dropped while it was waiting for the effect to
            // come back from the testkit.
            dependency.state = RecvFutureStateMachine::S2;
        }

        poll_fn(|cx| {
            if dependency.recv_effect_sender.is_closed() {
                panic!("the testkit has dropped");
            }

            loop {
                match dependency.state {
                    RecvFutureStateMachine::S0 => {
                        let m = ready!(self.m_receiver.next().poll_unpin(cx));
                        let recv = if let Some(m) = m {
                            Recv::Message(m)
                        } else {
                            Recv::NoMoreSenders
                        };

                        let effect_out = RecvEffectFromActorToTestkit { recv };

                        dependency
                            .recv_effect_sender
                            .try_send(effect_out)
                            .expect("could not send the effect to the testkit");

                        dependency.state = RecvFutureStateMachine::S1;
                    }
                    RecvFutureStateMachine::S1 => {
                        let effect_in = match dependency.recv_effect_receiver.poll_next_unpin(cx) {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(effect_in)) => effect_in,
                            Poll::Pending => return Poll::Pending,
                        };

                        dependency.state = RecvFutureStateMachine::S0;

                        if !effect_in.discarded {
                            return Poll::Ready(effect_in.recv);
                        } // else: poll the channels in the next iteration
                    }
                    RecvFutureStateMachine::S2 => {
                        let effect_in = match dependency.recv_effect_receiver.poll_next_unpin(cx) {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(effect_in)) => effect_in,
                            Poll::Pending => return Poll::Pending,
                        };

                        let effect_out = RecvEffectFromActorToTestkit { recv: effect_in.recv };

                        dependency
                            .recv_effect_sender
                            .try_send(effect_out)
                            .expect("could not send the effect to the testkit");

                        dependency.state = RecvFutureStateMachine::S1;
                    }
                }
            }
        })
    }
}
