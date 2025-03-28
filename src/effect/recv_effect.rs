use crate::actor::Recv;
use std::fmt::{Debug, Formatter};

pub struct RecvEffect<M> {
    pub recv: Recv<M>,
    pub(crate) discarded: bool,
}

impl<M> Debug for RecvEffect<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.recv.fmt(f)
    }
}

impl<M> RecvEffect<M> {
    pub fn discard(&mut self) {
        self.discarded = true;
    }
}

pub struct RecvEffectFromActorToTestkit<M> {
    pub recv: Recv<M>,
}

impl<M> Debug for RecvEffectFromActorToTestkit<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect").field("recv", &self.recv).finish()
    }
}

pub struct RecvEffectFromTestkitToActor<M> {
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
