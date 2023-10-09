use std::fmt::{Debug, Formatter};
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ActorSystem {
    inner: Arc<ActorSystemInner>,
}

pub struct ActorSystemInner {
    pub(crate) actor_name: String,
    pub(crate) runtime: runtime::Runtime,
    pub(crate) cancellation: CancellationToken,
    pub(crate) created_at: Instant,
}

impl Deref for ActorSystem {
    type Target = ActorSystemInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Debug for ActorSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.actor_name)
    }
}

impl ActorSystem {
    pub fn new(name: &str) -> io::Result<Self> {
        assert!(!name.is_empty(), "ActorSystem name is empty");
        assert!(name.is_ascii(), "ActorSystem name is not ASCII");

        let rt = runtime::Builder::new_multi_thread()
            .thread_name(format!("{}/rt", name))
            .worker_threads(2)
            .enable_all()
            .build()?;

        let inner = ActorSystemInner {
            actor_name: name.to_string(),
            runtime: rt,
            cancellation: Default::default(),
            created_at: Instant::now(),
        };

        Ok(ActorSystem {
            inner: Arc::new(inner),
        })
    }

    pub fn stop(&self) {
        self.inner.cancellation.cancel()
    }

    pub fn created_at(&self) -> &Instant {
        &self.inner.created_at
    }

    pub fn uptime(&self) -> Duration {
        Instant::now().duration_since(self.inner.created_at.clone())
    }
}
