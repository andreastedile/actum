use crate::prelude::Recv;
use crate::testkit::{AnyTestkit, Testkit};
use std::fmt::{Debug, Formatter};

pub enum Effect<'a, M> {
    Recv(RecvEffect<'a, M>),
    Spawn(SpawnEffect),
}

impl<M> Debug for Effect<'_, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Recv(inner) => inner.fmt(f),
            Effect::Spawn(inner) => inner.fmt(f),
        }
    }
}

impl<'a, M> Effect<'a, M> {
    pub fn unwrap_recv(self) -> RecvEffect<'a, M> {
        match self {
            Effect::Recv(inner) => inner,
            other => panic!("called `Effect::unwrap_recv()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_spawn(self) -> SpawnEffect {
        match self {
            Effect::Spawn(inner) => inner,
            other => panic!("called `Effect::unwrap_spawn()` on a `{:?}` value", other),
        }
    }

    pub const fn is_recv(&self) -> bool {
        matches!(self, Self::Recv(_))
    }

    pub const fn is_spawn(&self) -> bool {
        matches!(self, Self::Spawn(_))
    }
}

pub struct RecvEffect<'a, M> {
    pub recv: Recv<&'a M>,
    pub(crate) discarded: &'a mut bool,
}

impl<M> Debug for RecvEffect<'_, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.recv.fmt(f)
    }
}

impl<M> RecvEffect<'_, M> {
    pub fn discard(&mut self) {
        *self.discarded = true;
    }
}

pub struct SpawnEffect {
    pub testkit: AnyTestkit,
}

impl Debug for SpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Testkit")
    }
}

pub enum EffectFromActorToTestkit<M> {
    Recv(RecvEffectFromActorToTestkit<M>),
    Spawn(SpawnEffectFromActorToTestkit),
}

impl<M> Debug for EffectFromActorToTestkit<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recv(inner) => inner.fmt(f),
            Self::Spawn(inner) => inner.fmt(f),
        }
    }
}

pub struct RecvEffectFromActorToTestkit<M>(pub Recv<M>);

impl<M> Debug for RecvEffectFromActorToTestkit<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct SpawnEffectFromActorToTestkit(pub Option<AnyTestkit>);

impl Debug for SpawnEffectFromActorToTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpawnEffect")
    }
}

impl SpawnEffectFromActorToTestkit {
    pub fn new<M>(testkit: Testkit<M>) -> Self
    where
        M: Send + 'static,
    {
        Self(Some(testkit.into()))
    }
}

pub struct RecvEffectFromTestkitToActor<M> {
    pub recv: Recv<M>,
    pub discarded: bool,
}

impl<M> Debug for RecvEffectFromTestkitToActor<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecvEffect")
            .field("recv", &self.recv)
            .field("discarded", &self.discarded)
            .finish()
    }
}

pub struct SpawnEffectFromTestkitToActor;

impl Debug for SpawnEffectFromTestkitToActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpawnEffect")
    }
}
