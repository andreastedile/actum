use crate::children_tracker::ChildrenTracker;

pub mod actor;
pub mod test_actor;

pub struct ActorCell<D> {
    pub(crate) tracker: Option<ChildrenTracker>,
    dependency: D,
}

impl<D> ActorCell<D> {
    pub(crate) const fn new(dependency: D) -> Self {
        Self {
            tracker: None,
            dependency,
        }
    }
}
