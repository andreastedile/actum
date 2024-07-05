use crate::resolve_when_one::ResolveWhenOne;
use futures::channel::{mpsc, oneshot};

pub mod actor_task;
pub mod standard_actor;
pub mod test_actor;

pub struct ActorCell<M, AB> {
    stop_receiver: oneshot::Receiver<Stop>,
    m_receiver: mpsc::Receiver<M>,
    subtree: Option<ResolveWhenOne>,
    bounds: AB,
}

impl<M, AB> ActorCell<M, AB> {
    pub(crate) fn new(stop_receiver: oneshot::Receiver<Stop>, m_receiver: mpsc::Receiver<M>, bounds: AB) -> Self {
        Self {
            stop_receiver,
            m_receiver,
            subtree: None,
            bounds,
        }
    }
}

pub struct Stop;
