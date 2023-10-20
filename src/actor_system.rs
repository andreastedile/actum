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
    pub(crate) name: String,
    pub(crate) runtime: runtime::Runtime,
    pub(crate) cancellation: CancellationToken,
    pub(crate) created_at: Instant,
}

pub struct ActorSystemConfig {
    pub n_threads: usize,
}

/// Errors that can occur using [`ActorSystem::new`].
#[derive(Debug)]
pub enum SpawnError {
    EmptyName,
    NonAlphanumeric,
    IO(io::Error),
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

impl Default for ActorSystemConfig {
    fn default() -> Self {
        ActorSystemConfig { n_threads: 1 }
    }
}

impl ActorSystem {
    pub fn new(name: &str, config: ActorSystemConfig) -> Result<Self, SpawnError> {
        if name.is_empty() {
            return Err(SpawnError::EmptyName);
        }

        if name.chars().any(|char| !char.is_ascii_alphanumeric()) {
            return Err(SpawnError::NonAlphanumeric);
        }

        let runtime = runtime::Builder::new_multi_thread()
            .thread_name("actum")
            .worker_threads(config.n_threads)
            .enable_all()
            .build();

        let runtime = match runtime {
            Ok(runtime) => runtime,
            Err(error) => return Err(SpawnError::IO(error)),
        };

        let inner = ActorSystemInner {
            name: name.to_string(),
            runtime,
            cancellation: CancellationToken::new(),
            created_at: Instant::now(),
        };

        Ok(ActorSystem { inner: Arc::new(inner) })
    }

    pub fn stop(&mut self) {
        self.inner.cancellation.cancel()
    }

    pub fn created_at(&self) -> &Instant {
        &self.inner.created_at
    }

    pub fn uptime(&self) -> Duration {
        Instant::now().duration_since(self.inner.created_at.clone())
    }
}
