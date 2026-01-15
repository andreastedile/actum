use crate::core::receive_message::Recv;
use std::fmt::{Debug, Formatter};

pub(crate) struct RecvEffectPrivate<M> {
    pub recv: Recv<M>,
    pub discarded: bool,
}

impl<M> Debug for RecvEffectPrivate<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect")
            .field("recv", &self.recv)
            .field("discarded", &self.discarded)
            .finish()
    }
}

pub struct RecvEffect<'a, M> {
    pub recv: &'a Recv<M>,
    pub discarded: &'a mut bool,
}

impl<'a, M> Debug for RecvEffect<'a, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect")
            .field("recv", self.recv)
            .field("discarded", self.discarded)
            .finish()
    }
}

impl<'a, M> RecvEffect<'a, M> {
    pub const fn discard(&mut self) {
        *self.discarded = true;
    }
}

/// From the actor under test to the testkit.
pub(crate) struct RecvEffectToTestkit<M> {
    pub recv: Recv<M>,
}

impl<M> Debug for RecvEffectToTestkit<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect").field("recv", &self.recv).finish()
    }
}

/// From the testkit to the actor under test.
pub(crate) struct RecvEffectToActor<M> {
    pub recv: Recv<M>,
    pub discarded: bool,
}

impl<M> From<RecvEffectPrivate<M>> for RecvEffectToActor<M> {
    fn from(effect: RecvEffectPrivate<M>) -> Self {
        RecvEffectToActor {
            recv: effect.recv,
            discarded: effect.discarded,
        }
    }
}

impl<M> Debug for RecvEffectToActor<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect")
            .field("recv", &self.recv)
            .field("discarded", &self.discarded)
            .finish()
    }
}
