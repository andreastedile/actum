use crate::core::children_tracker::ChildrenTracker;

pub struct ActorCell<D> {
    pub(crate) tracker: Option<ChildrenTracker>,
    pub(crate) dependency: D,
}

impl<D> ActorCell<D> {
    pub const fn new(dependency: D) -> Self {
        Self {
            tracker: None,
            dependency,
        }
    }
}
