pub mod recv;
pub mod spawn;

use crate::effect::recv::RecvEffect;
use crate::effect::spawn::SpawnEffect;
use std::fmt::{Debug, Formatter};

/// An actor that calls the [recv](crate::actor_bounds::ActorBounds::recv) or [spawn](crate::actor_bounds::ActorBounds::spawn) methods
/// sends the corresponding effect to the [Testkit](crate::testkit::Testkit) and suspends
/// until the effect is tested and dropped.
pub enum Effect<M> {
    Recv(RecvEffect<M>),
    Spawn(SpawnEffect),
}

impl<M> Debug for Effect<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recv(effect) => effect.fmt(f),
            Self::Spawn(effect) => effect.fmt(f),
        }
    }
}

impl<M> Effect<M> {
    pub fn recv(self) -> Option<RecvEffect<M>> {
        match self {
            Self::Recv(effect) => Some(effect),
            Self::Spawn(_) => None,
        }
    }

    pub const fn is_recv(&self) -> bool {
        matches!(self, Self::Recv(_))
    }

    pub fn is_recv_and(self, f: impl FnOnce(RecvEffect<M>) -> bool) -> bool {
        if let Self::Recv(effect) = self {
            f(effect)
        } else {
            false
        }
    }

    pub fn spawn(self) -> Option<SpawnEffect> {
        match self {
            Self::Recv(_) => None,
            Self::Spawn(effect) => Some(effect),
        }
    }

    pub const fn is_spawn(&self) -> bool {
        matches!(self, Self::Spawn(_))
    }

    pub fn is_spawn_and(self, f: impl FnOnce(SpawnEffect) -> bool) -> bool {
        if let Self::Spawn(effect) = self {
            f(effect)
        } else {
            false
        }
    }
}

impl<M> From<RecvEffect<M>> for Effect<M> {
    fn from(effect: RecvEffect<M>) -> Self {
        Self::Recv(effect)
    }
}

impl<M> From<SpawnEffect> for Effect<M> {
    fn from(effect: SpawnEffect) -> Self {
        Self::Spawn(effect)
    }
}
