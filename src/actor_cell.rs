use crate::resolve_when_one::ResolveWhenOne;
use futures::channel::{mpsc, oneshot};

pub mod standard_actor;
pub mod test_actor;

pub struct ActorCell<M, D> {
    stop_receiver: oneshot::Receiver<Stop>,
    m_receiver: mpsc::Receiver<M>,
    pub(crate) subtree: Option<ResolveWhenOne>,
    dependency: D,
}

impl<M, D> ActorCell<M, D> {
    pub(crate) const fn new(
        stop_receiver: oneshot::Receiver<Stop>,
        m_receiver: mpsc::Receiver<M>,
        dependency: D,
    ) -> Self {
        Self {
            stop_receiver,
            m_receiver,
            subtree: None,
            dependency,
        }
    }
}

pub struct Stop;
