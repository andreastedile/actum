use crate::receive_message::Recv;
use std::fmt::{Debug, Formatter};

pub(crate) struct RecvEffectImpl<M> {
    pub recv: Recv<M>,
    pub discarded: bool,
}

impl<M> Debug for RecvEffectImpl<M> {
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

pub(crate) struct RecvEffectFromActorToTestkit<M> {
    pub recv: Recv<M>,
}

impl<M> Debug for RecvEffectFromActorToTestkit<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect").field("recv", &self.recv).finish()
    }
}

pub(crate) struct RecvEffectFromTestkitToActor<M> {
    pub recv: Recv<M>,
    pub discarded: bool,
}

impl<M> Debug for RecvEffectFromTestkitToActor<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect")
            .field("recv", &self.recv)
            .field("discarded", &self.discarded)
            .finish()
    }
}
