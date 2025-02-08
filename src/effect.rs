use crate::testkit::AnyTestkit;
use std::fmt::{Debug, Formatter};

/// TODO
pub enum EffectType<'a, M> {
    Stopped(RecvStoppedEffect<'a, M>),
    Message(RecvMessageEffect<'a, M>),
    NoMoreSenders,
    Spawn(SpawnEffect),
}

impl<'a, M> Debug for EffectType<'a, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectType::Stopped(_) => f.write_str("Stopped"),
            EffectType::Message(_) => f.write_str("Message"),
            EffectType::NoMoreSenders => f.write_str("NoMoreSenders"),
            EffectType::Spawn(_) => f.write_str("Spawn"),
        }
    }
}

impl<'a, M> EffectType<'a, M> {
    pub fn unwrap_message(self) -> RecvMessageEffect<'a, M> {
        match self {
            EffectType::Message(inner) => inner,
            other => panic!("called `EffectType::unwrap_message()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_stopped(self) -> RecvStoppedEffect<'a, M> {
        match self {
            EffectType::Stopped(inner) => inner,
            other => panic!("called `EffectType::unwrap_stopped()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_spawn(self) -> SpawnEffect {
        match self {
            EffectType::Spawn(inner) => inner,
            other => panic!("called `Effect::unwrap_spawn()` on a `{:?}` value", other),
        }
    }

    pub const fn is_spawn(&self) -> bool {
        matches!(self, Self::Spawn(_))
    }
}

pub struct RecvMessageEffect<'a, M> {
    pub m: &'a M,
}

pub struct RecvStoppedEffect<'a, M> {
    pub m: Option<&'a M>,
}

pub struct SpawnEffect {
    pub testkit: Option<AnyTestkit>,
}

pub struct ReturnedEffect;

impl<'a, M> RecvMessageEffect<'a, M> {
    pub fn new(m: &'a M) -> Self {
        Self { m }
    }
}

impl<'a, M> RecvStoppedEffect<'a, M> {
    pub fn new(m: Option<&'a M>) -> Self {
        Self { m }
    }
}

impl SpawnEffect {
    pub fn new(testkit: Option<AnyTestkit>) -> Self {
        Self { testkit }
    }

    pub fn unwrap_testkit(&mut self) -> AnyTestkit {
        self.testkit.take().unwrap()
    }
}
