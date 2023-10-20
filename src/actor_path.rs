use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct ActorPath(pub(crate) Arc<ActorPathInner>);

pub struct ActorPathInner {
    pub parent: Option<ActorPath>,
    pub name: String,
}

impl Deref for ActorPath {
    type Target = ActorPathInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Debug for ActorPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.full_name())
    }
}

impl PartialEq<Self> for ActorPath {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ActorPath {}

impl Hash for ActorPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.full_name().hash(state)
    }
}

impl ActorPath {
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

    pub fn is_same_as(&self, other: &ActorPath) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn is_ancestor_of(&self, other: &ActorPath) -> bool {
        other
            .parent
            .as_ref()
            .is_some_and(|path| Arc::ptr_eq(&self.0, &path.0) || self.is_ancestor_of(&path))
    }

    pub fn is_parent_of(&self, other: &ActorPath) -> bool {
        other.parent.as_ref().is_some_and(|path| Arc::ptr_eq(&self.0, &path.0))
    }

    pub fn is_child_of(&self, other: &ActorPath) -> bool {
        self.parent.as_ref().is_some_and(|path| Arc::ptr_eq(&other.0, &path.0))
    }

    pub fn is_descendant_of(&self, other: &ActorPath) -> bool {
        other.is_ancestor_of(&self)
    }
}
