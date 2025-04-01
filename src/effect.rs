pub mod recv_effect;
pub mod returned_effect;
pub mod spawn_effect;

use crate::effect::recv_effect::RecvEffect;
use crate::effect::returned_effect::ReturnedEffect;
use crate::effect::spawn_effect::UntypedSpawnEffect;
use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};

#[derive(EnumAsInner)]
pub enum Effect<M, Ret> {
    Recv(RecvEffect<M>),
    Spawn(UntypedSpawnEffect),
    Returned(ReturnedEffect<Ret>),
}

impl<M, Ret> Debug for Effect<M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recv(inner) => inner.fmt(f),
            Self::Spawn(inner) => inner.fmt(f),
            Self::Returned(inner) => inner.fmt(f),
        }
    }
}
