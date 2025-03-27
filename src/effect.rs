use crate::prelude::Recv;
use crate::testkit::{AnyTestkit, Testkit};
use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};

#[derive(EnumAsInner)]
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
