use futures::channel::oneshot;
use std::fmt::{Debug, Formatter};

pub struct RecvEffect<M> {
    m: Option<M>,
    // wrap in Option so that it can be taken.
    recv_effect_out_m_sender: Option<oneshot::Sender<RecvEffectOut<M>>>,
    // wrap in Option so that it can be taken.
    recv_effect_in_m_sender: Option<oneshot::Sender<RecvEffectIn<M>>>,
}

impl<M> RecvEffect<M> {
    pub(crate) fn new(
        m: Option<M>,
        recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
        recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>,
    ) -> Self {
        Self {
            m,
            recv_effect_out_m_sender: Some(recv_effect_out_m_sender),
            recv_effect_in_m_sender: Some(recv_effect_in_m_sender),
        }
    }

    /// Inspect the return value of [recv](crate::actor_bounds::ActorBounds::recv).
    pub const fn message(&self) -> Option<&M> {
        self.m.as_ref()
    }
}

impl<M> Debug for RecvEffect<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recv")
    }
}

pub struct RecvEffectOut<M> {
    pub m: Option<M>,
    pub recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>,
}

impl<M> RecvEffectOut<M> {
    pub(crate) fn new(m: Option<M>, recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>) -> Self {
        Self {
            m,
            recv_effect_in_m_sender,
        }
    }
}

impl<M> Drop for RecvEffect<M> {
    fn drop(&mut self) {
        if let Some(recv_effect_in_m_sender) = self.recv_effect_in_m_sender.take() {
            let m = self.m.take();
            let effect = RecvEffectIn {
                m,
                recv_effect_out_m_sender: self.recv_effect_out_m_sender.take().unwrap(),
            };
            if recv_effect_in_m_sender.send(effect).is_err() {
                panic!("could not send the effect back to the actor")
            }
        }
    }
}

pub struct RecvEffectIn<M> {
    /// Return m back to the actor under test.
    pub m: Option<M>,
    pub recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
}
