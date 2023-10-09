use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct ActorPath {
    pub(crate) inner: Arc<ActorPathInner>,
}

pub struct ActorPathInner {
    pub parent: Option<ActorPath>,
    pub name: String,
    pub full_name: String,
}

impl Deref for ActorPath {
    type Target = ActorPathInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Debug for ActorPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.full_name)
    }
}

impl PartialEq<Self> for ActorPath {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for ActorPath {}

impl Hash for ActorPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.full_name.hash(state)
    }
}

impl ActorPath {
    pub fn is_same_as(&self, other: &ActorPath) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn is_ancestor_of(&self, other: &ActorPath) -> bool {
        other
            .parent
            .as_ref()
            .is_some_and(|path| Arc::ptr_eq(&self.inner, &path.inner) || self.is_ancestor_of(&path))
    }

    pub fn is_parent_of(&self, other: &ActorPath) -> bool {
        other
            .parent
            .as_ref()
            .is_some_and(|path| Arc::ptr_eq(&self.inner, &path.inner))
    }

    pub fn is_child_of(&self, other: &ActorPath) -> bool {
        self.parent
            .as_ref()
            .is_some_and(|path| Arc::ptr_eq(&other.inner, &path.inner))
    }

    pub fn is_descendant_of(&self, other: &ActorPath) -> bool {
        other.is_ancestor_of(&self)
    }

    pub fn make_child_path(&self, name: &str) -> ActorPath {
        ActorPath {
            inner: Arc::new(ActorPathInner {
                parent: Some(self.clone()),
                name: name.to_string(),
                full_name: format!("{}/{}", self.full_name, name),
            }),
        }
    }
}
