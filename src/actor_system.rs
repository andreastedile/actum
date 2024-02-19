use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::runtime;

#[derive(Clone)]
pub struct ActorSystem {
    inner: Arc<ActorSystemInner>,
}

pub struct ActorSystemInner {
    pub(crate) runtime: runtime::Handle,
    name: String,
    created_at: Instant,
}

#[derive(Debug, Eq, PartialEq)]
pub enum NameError {
    IsEmpty,
    HasWhitespaces,
}

impl Deref for ActorSystem {
    type Target = ActorSystemInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Debug for ActorSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl ActorSystem {
    pub fn new(name: &str, runtime: runtime::Handle) -> Result<Self, NameError> {
        if name.is_empty() {
            return Err(NameError::IsEmpty);
        }

        if name.contains(' ') {
            return Err(NameError::HasWhitespaces);
        }

        let inner = ActorSystemInner {
            name: name.to_string(),
            created_at: Instant::now(),
            runtime,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn created_at(&self) -> &Instant {
        &self.inner.created_at
    }

    pub fn uptime(&self) -> Duration {
        Instant::now().duration_since(self.inner.created_at)
    }
}
