use crate::actor_test::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::core::message_receiver::MessageReceiver;
use crate::prelude::{ReceiveMessage, Recv};
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::future::poll_fn;
use std::task::{Poll, ready};

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

/// The future returned by the [recv](ReceiveMessage::recv) method
/// can be dropped before being polled to completion; for example, when it races with a timeout.
/// This fact, and the fact that the result of the future is temporarily transferred to the testkit,
/// makes future not cancel safe.
#[derive(Debug, Eq, PartialEq)]
pub enum RecvFutureStateMachine {
    /// We have called [crate::actor::create_child::Actor::recv] and obtained a future.
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

impl<M> ReceiveMessage<M> for MessageReceiver<M, MessageReceiverTestkitExtension<M>>
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
