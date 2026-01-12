pub mod create_child_effect;
pub mod recv_effect;
pub mod returned_effect;

use crate::actor_test::effect::create_child_effect::{UntypedCreateChildEffect, UntypedCreateChildEffectPrivate};
use crate::actor_test::effect::recv_effect::{RecvEffect, RecvEffectPrivate};
use crate::actor_test::effect::returned_effect::{ReturnedEffect, ReturnedEffectPrivate};
use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};

#[derive(EnumAsInner)]
pub enum Effect<'a, M, Output> {
    Recv(RecvEffect<'a, M>),
    CreateChild(UntypedCreateChildEffect<'a>),
    Returned(ReturnedEffect<'a, Output>),
}

impl<'a, M, Output> Debug for Effect<'a, M, Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recv(inner) => inner.fmt(f),
            Self::CreateChild(inner) => inner.fmt(f),
            Self::Returned(inner) => inner.fmt(f),
        }
    }
}

pub(crate) enum EffectPrivate<M, Output> {
    Recv(RecvEffectPrivate<M>),
    CreateChild(UntypedCreateChildEffectPrivate),
    Returned(ReturnedEffectPrivate<Output>),
}
