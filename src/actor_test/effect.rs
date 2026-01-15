pub mod completed_effect;
pub mod create_child_effect;
pub mod recv_effect;

use crate::actor_test::effect::completed_effect::{CompletedEffect, CompletedEffectPrivate, CompletedEffectToTestkit};
use crate::actor_test::effect::create_child_effect::{
    UntypedCreateChildEffect, UntypedCreateChildEffectPrivate, UntypedCreateChildEffectToTestkit,
};
use crate::actor_test::effect::recv_effect::{RecvEffect, RecvEffectPrivate, RecvEffectToTestkit};
use enum_as_inner::EnumAsInner;
use std::fmt::{Debug, Formatter};

#[derive(EnumAsInner)]
pub enum Effect<'a, M, Output> {
    Recv(RecvEffect<'a, M>),
    CreateChild(UntypedCreateChildEffect<'a>),
    Completed(CompletedEffect<'a, Output>),
}

impl<'a, M, Output> Debug for Effect<'a, M, Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recv(inner) => inner.fmt(f),
            Self::CreateChild(inner) => inner.fmt(f),
            Self::Completed(inner) => inner.fmt(f),
        }
    }
}

impl<'a, M, Output> From<&'a mut EffectPrivate<M, Output>> for Effect<'a, M, Output> {
    fn from(effect: &'a mut EffectPrivate<M, Output>) -> Self {
        match effect {
            EffectPrivate::Recv(variant) => Self::Recv(RecvEffect {
                recv: &mut variant.recv,
                discarded: &mut variant.discarded,
            }),
            EffectPrivate::CreateChild(variant) => Self::CreateChild(UntypedCreateChildEffect {
                untyped_testkit: variant.untyped_testkit.take().unwrap(),
                injected: &mut variant.injected,
            }),
            EffectPrivate::Completed(variant) => Self::Completed(CompletedEffect {
                output: &mut variant.output,
            }),
        }
    }
}

pub(crate) enum EffectPrivate<M, Output> {
    Recv(RecvEffectPrivate<M>),
    CreateChild(UntypedCreateChildEffectPrivate),
    Completed(CompletedEffectPrivate<Output>),
}

impl<M, Output> From<RecvEffectToTestkit<M>> for EffectPrivate<M, Output> {
    fn from(effect: RecvEffectToTestkit<M>) -> Self {
        Self::Recv(RecvEffectPrivate {
            recv: effect.recv,
            discarded: false,
        })
    }
}

impl<M, Output> From<UntypedCreateChildEffectToTestkit> for EffectPrivate<M, Output> {
    fn from(effect: UntypedCreateChildEffectToTestkit) -> Self {
        Self::CreateChild(UntypedCreateChildEffectPrivate {
            untyped_testkit: Some(effect.untyped_testkit),
            injected: None,
        })
    }
}

impl<M, Output> From<CompletedEffectToTestkit<Output>> for EffectPrivate<M, Output> {
    fn from(effect: CompletedEffectToTestkit<Output>) -> Self {
        Self::Completed(CompletedEffectPrivate { output: effect.output })
    }
}
