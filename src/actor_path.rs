use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::Arc;

pub struct ActorPath(Arc<ActorPathInner>);

impl Debug for ActorPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.full_name())
    }
}

pub struct ActorPathInner {
    pub parent: Option<ActorPath>,
    pub name: String,
}

impl Clone for ActorPath {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl PartialEq<Self> for ActorPath {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ActorPath {}

impl Deref for ActorPath {
    type Target = ActorPathInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ActorPath {
    pub(crate) fn new(name: &str) -> Self {
        Self(Arc::new(ActorPathInner {
            parent: None,
            name: name.to_string(),
        }))
    }

    pub(crate) fn make_child(&self, name: &str) -> Self {
        Self(Arc::new(ActorPathInner {
            parent: Some(self.clone()),
            name: name.to_string(),
        }))
    }

    pub fn is_same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn is_parent_of(&self, other: &Self) -> bool {
        other
            .parent
            .as_ref()
            .is_some_and(|path| Arc::ptr_eq(&self.0, &path.0))
    }

    pub fn is_child_of(&self, other: &Self) -> bool {
        self.parent
            .as_ref()
            .is_some_and(|path| Arc::ptr_eq(&other.0, &path.0))
    }

    pub fn full_name(&self) -> String {
        let mut full_name = String::with_capacity(1 + self.name.len());

        let mut path = self;

        loop {
            full_name.insert_str(0, &path.name);
            full_name.insert(0, '/');

            if let Some(parent) = &path.parent {
                path = parent;
            } else {
                break;
            }
        }

        full_name
    }
}
