use crate::core::children_tracker::ChildrenTracker;

pub struct ActorCell<D> {
    pub(crate) tracker: ChildrenTracker,
    pub(crate) dependency: D,
}

impl<D> ActorCell<D> {
    pub fn new(dependency: D) -> Self {
        Self {
            tracker: ChildrenTracker::new(),
            dependency,
        }
    }
}
