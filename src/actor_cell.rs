use crate::children_tracker::ChildrenTracker;
use futures::channel::mpsc;

pub mod standard_actor;
pub mod test_actor;

pub struct ActorCell<M, D> {
    m_receiver: mpsc::Receiver<M>,
    pub(crate) tracker: Option<ChildrenTracker>,
    dependency: D,
}

impl<M, D> ActorCell<M, D> {
    pub(crate) const fn new(m_receiver: mpsc::Receiver<M>, dependency: D) -> Self {
        Self {
            m_receiver,
            tracker: None,
            dependency,
        }
    }
}

pub struct Stop;
