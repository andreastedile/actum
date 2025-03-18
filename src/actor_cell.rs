use crate::children_tracker::ChildrenTracker;
use futures::channel::{mpsc, oneshot};

pub mod standard_actor;
pub mod test_actor;

pub struct ActorCell<M, D> {
    stop_receiver: oneshot::Receiver<Stop>,
    m_receiver: mpsc::Receiver<M>,
    pub(crate) tracker: Option<ChildrenTracker>,
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
            tracker: None,
            dependency,
        }
    }
}

pub struct Stop;
