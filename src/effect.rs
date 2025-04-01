pub mod recv_effect;
pub mod returned_effect;
pub mod spawn_effect;

use crate::effect::recv_effect::{RecvEffect, RecvEffectImpl};
use crate::effect::returned_effect::{ReturnedEffect, ReturnedEffectImpl};
use crate::effect::spawn_effect::{UntypedSpawnEffect, UntypedSpawnEffectImpl};
use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};

#[derive(EnumAsInner)]
pub enum Effect<'a, M, Ret> {
    Recv(RecvEffect<'a, M>),
    Spawn(UntypedSpawnEffect<'a>),
    Returned(ReturnedEffect<'a, Ret>),
}

impl<'a, M, Ret> Debug for Effect<'a, M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recv(inner) => inner.fmt(f),
            Self::Spawn(inner) => inner.fmt(f),
            Self::Returned(inner) => inner.fmt(f),
        }
    }
}

pub(crate) enum EffectImpl<M, Ret> {
    Recv(RecvEffectImpl<M>),
    Spawn(UntypedSpawnEffectImpl),
    Returned(ReturnedEffectImpl<Ret>),
}
