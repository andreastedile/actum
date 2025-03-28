use crate::children_tracker::ChildrenTracker;
use std::marker::PhantomData;

pub mod actor;
pub mod test_actor;

pub struct ActorCell<D, Ret> {
    pub(crate) tracker: Option<ChildrenTracker>,
    dependency: D,
    _ret: PhantomData<Ret>,
}

impl<D, Ret> ActorCell<D, Ret> {
    pub(crate) const fn new(dependency: D) -> Self {
        Self {
            tracker: None,
            dependency,
            _ret: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_it() {
        let t = trybuild::TestCases::new();
        t.pass("tests/ui/return_type_v1.rs");
        t.compile_fail("tests/ui/return_type_v2.rs");
    }
}
