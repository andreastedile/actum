use crate::actor_bounds::Recv;
use futures::channel::oneshot;
use std::fmt::{Debug, Formatter};

// wrap in Option so that it can be taken in Drop.
pub struct RecvEffect<M>(Option<RecvEffectInner<M>>);

pub struct RecvEffectInner<M> {
    recv: Recv<M>,
    recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
    recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>,
}

impl<M> RecvEffect<M> {
    pub(crate) fn new(
        recv: Recv<M>,
        recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
        recv_effect_in_m_sender: oneshot::Sender<RecvEffectIn<M>>,
    ) -> Self {
        Self(Some(RecvEffectInner {
            recv,
            recv_effect_out_m_sender,
            recv_effect_in_m_sender,
        }))
    }

    pub fn message(&self) -> Option<&M> {
        if let Recv::Message(m) = &self.0.as_ref().unwrap().recv {
            Some(m)
        } else {
            None
        }
    }

    pub fn is_message(&self) -> bool {
        matches!(self.0.as_ref().unwrap().recv, Recv::Message(_))
    }

    pub fn is_message_and(&self, f: impl FnOnce(&M) -> bool) -> bool {
        if let Recv::Message(m) = &self.0.as_ref().unwrap().recv {
            f(m)
        } else {
            false
        }
    }

    pub fn stopped(&self) -> Option<Option<&M>> {
        if let Recv::Stopped(m) = &self.0.as_ref().unwrap().recv {
            Some(m.as_ref())
        } else {
            None
        }
    }

    pub fn is_stopped_and(&self, f: impl FnOnce(Option<&M>) -> bool) -> bool {
        if let Recv::Stopped(m) = &self.0.as_ref().unwrap().recv {
            f(m.as_ref())
        } else {
            false
        }
    }

    pub fn is_no_more_senders(&self) -> bool {
        matches!(self.0.as_ref().unwrap().recv, Recv::NoMoreSenders)
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
        let RecvEffectInner {
            recv,
            recv_effect_out_m_sender,
            recv_effect_in_m_sender,
        } = self.0.take().unwrap();
        let effect = RecvEffectIn {
            recv,
            recv_effect_out_m_sender,
        };
        if recv_effect_in_m_sender.send(effect).is_err() {
            panic!("could not send the effect back to the actor")
        }
    }
}

pub struct RecvEffectIn<M> {
    /// Return recv back to the actor under test.
    pub recv: Recv<M>,
    pub recv_effect_out_m_sender: oneshot::Sender<RecvEffectOut<M>>,
}
