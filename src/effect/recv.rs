use crate::actor_bounds::Recv;
use futures::channel::oneshot;
use std::fmt::{Debug, Formatter};

pub struct RecvEffect<M> {
    // wrap in Option so that it can be taken.
    recv: Option<Recv<M>>,
    // wrap in Option so that it can be taken.
    recv_effect_out_m_sender: Option<oneshot::Sender<RecvEffectOut<M>>>,
    // wrap in Option so that it can be taken.
    recv_effect_in_m_sender: Option<oneshot::Sender<RecvEffectIn<M>>>,
}

impl<M> RecvEffect<M> {
    pub(crate) fn new(
        recv: Recv<M>,
        recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
        recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>,
    ) -> Self {
        Self {
            recv: Some(recv),
            recv_effect_out_m_sender: Some(recv_effect_out_m_sender),
            recv_effect_in_m_sender: Some(recv_effect_in_m_sender),
        }
    }

    /// Inspect the return value of [recv](crate::actor_bounds::ActorBounds::recv).
    pub fn recv(&self) -> Recv<&M> {
        self.recv.as_ref().unwrap().as_ref()
    }
}

impl<M> Debug for RecvEffect<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recv")
    }
}

pub struct RecvEffectOut<M> {
    pub recv: Recv<M>,
    pub recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>,
}

impl<M> RecvEffectOut<M> {
    pub(crate) fn new(recv: Recv<M>, recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>) -> Self {
        Self {
            recv,
            recv_effect_in_m_sender,
        }
    }
}

impl<M> Drop for RecvEffect<M> {
    fn drop(&mut self) {
        if let Some(recv_effect_in_m_sender) = self.recv_effect_in_m_sender.take() {
            let recv = self.recv.take().unwrap();
            let effect = RecvEffectIn {
                recv,
                recv_effect_out_m_sender: self.recv_effect_out_m_sender.take().unwrap(),
            };
            if recv_effect_in_m_sender.send(effect).is_err() {
                panic!("could not send the effect back to the actor")
            }
        }
    }
}

pub struct RecvEffectIn<M> {
    /// Return recv back to the actor under test.
    pub recv: Recv<M>,
    pub recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
}
