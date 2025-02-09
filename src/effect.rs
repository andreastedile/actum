use crate::prelude::Recv;
use crate::testkit::AnyTestkit;
use either::Either;
use std::fmt::{Debug, Formatter, Pointer};

pub enum Effect<'m, M> {
    Recv(Recv<&'m M>),
    Spawn(Either<AnyTestkit, Option<&'m M>>),
}

impl<M> Debug for Effect<'_, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Recv(inner) => inner.fmt(f),
            Effect::Spawn(inner) => inner.fmt(f),
        }
    }
}

impl<'m, M> Effect<'m, M> {
    pub fn unwrap_recv(self) -> Recv<&'m M> {
        match self {
            Effect::Recv(inner) => inner,
            other => panic!("called `Effect::unwrap_recv()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_spawn(self) -> Either<AnyTestkit, Option<&'m M>> {
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

pub enum EffectFromActorToTestkit<M> {
    Recv(RecvEffectFromActorToTestkit<M>),
    Spawn(SpawnEffectFromActorToTestkit<M>),
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

pub struct SpawnEffectFromActorToTestkit<M>(pub Either<Option<AnyTestkit>, Option<M>>);

impl<M> Debug for SpawnEffectFromActorToTestkit<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Either::Left(_) => f.write_str("AnyTestkit"),
            Either::Right(_) => f.write_str("Message"),
        }
    }
}

pub struct RecvEffectFromTestkitToActor<M>(pub Recv<M>);

impl<M> Debug for RecvEffectFromTestkitToActor<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct SpawnEffectFromTestkitToActor<M>(pub Either<(), Option<M>>);

impl<M> Debug for SpawnEffectFromTestkitToActor<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Either::Left(_) => f.write_str(""),
            Either::Right(_) => f.write_str("Message"),
        }
    }
}
