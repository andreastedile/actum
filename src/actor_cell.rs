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

#[cfg(test)]
mod tests {
    #[test]
    fn test_it() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/ui/return_type_v1.rs");
    }
}
