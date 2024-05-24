use futures::channel::oneshot;
use std::fmt::{Debug, Formatter};

pub struct RecvEffect<M> {
    m: Option<M>,
    // wrap in Option so that it can be taken in Drop.
    m_sender: Option<oneshot::Sender<Option<M>>>,
}

impl<M> RecvEffect<M> {
    pub(crate) fn new(m: Option<M>, m_sender: oneshot::Sender<Option<M>>) -> Self {
        Self {
            m,
            m_sender: Some(m_sender),
        }
    }

    /// Inspect the return value of [recv](crate::actor_bounds::ActorBounds::recv).
    pub const fn message(&self) -> Option<&M> {
        self.m.as_ref()
    }

    pub(crate) fn into_inner(mut self) -> Option<M> {
        self.m_sender = None;
        self.m.take()
    }
}

impl<M> Debug for RecvEffect<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recv")
    }
}

impl<M> Drop for RecvEffect<M> {
    fn drop(&mut self) {
        if let Some(m_sender) = self.m_sender.take() {
            let m = self.m.take();
            if m_sender.send(m).is_err() {
                panic!("Executor has dropped prematurely")
            }
        } // else: RecvEffect::into_inner was called
    }
}
