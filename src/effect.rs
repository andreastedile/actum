pub mod recv_effect;
pub mod spawn_effect;

use crate::effect::recv_effect::{RecvEffect, RecvEffectFromActorToTestkit};
use crate::effect::spawn_effect::{SpawnEffect, SpawnEffectFromActorToTestkit};
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
