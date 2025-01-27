use crate::actor_bounds::Recv;
use crate::testkit::AnyTestkit;
use futures::channel::mpsc;
use std::fmt::{Debug, Formatter};

/// An actor that calls the [recv](crate::actor_bounds::ActorBounds::recv) or [spawn](crate::actor_bounds::ActorBounds::spawn) methods
/// sends the corresponding effect to the [Testkit](crate::testkit::Testkit) and suspends
/// until the effect is tested and dropped.
pub enum EffectType<'a, M> {
    Stopped(RecvStoppedEffect<'a, M>),
    Message(RecvMessageEffect<'a, M>),
    NoMoreSenders(RecvNoMoreSendersEffect<'a, M>),
    Spawn(SpawnEffect<'a>),
    Returned(ReturnedEffect),
}

impl<'a, M> Debug for EffectType<'a, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectType::Stopped(_) => f.write_str("Stopped"),
            EffectType::Message(_) => f.write_str("Message"),
            EffectType::NoMoreSenders(_) => f.write_str("NoMoreSenders"),
            EffectType::Spawn(_) => f.write_str("Spawn"),
            EffectType::Returned(_) => f.write_str("Returned"),
        }
    }
}

/// A witness that the [EffectExecution::execute] method has been called on an effect.
pub struct EffectExecutionWitness {
    _private: (),
}

pub trait EffectExecution {
    fn execute(self) -> EffectExecutionWitness;
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

    pub fn unwrap_no_more_senders(self) -> RecvNoMoreSendersEffect<'a, M> {
        match self {
            EffectType::NoMoreSenders(inner) => inner,
            other => panic!("called `EffectType::unwrap_no_more_senders()` on a `{:?}` value", other),
        }
    }

    pub fn execute(self) -> EffectExecutionWitness {
        match self {
            EffectType::Stopped(inner) => inner.execute(),
            EffectType::Message(inner) => inner.execute(),
            EffectType::NoMoreSenders(inner) => inner.execute(),
            EffectType::Spawn(inner) => inner.execute(),
            EffectType::Returned(inner) => inner.execute(),
        }
    }

    pub fn unwrap_spawn(self) -> SpawnEffect<'a> {
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
    recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>,
    pub m: M,
}

pub struct RecvStoppedEffect<'a, M> {
    recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>,
    pub m: Option<M>,
}

pub struct RecvNoMoreSendersEffect<'a, M> {
    recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>,
}

pub struct SpawnEffect<'a> {
    spawn_effect_out_sender: &'a mut mpsc::Sender<()>,
    pub testkit: Option<AnyTestkit>,
}

pub struct ReturnedEffect;

impl<'a, M> RecvMessageEffect<'a, M> {
    pub fn new(recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>, m: M) -> Self {
        Self {
            recv_effect_out_m_sender,
            m,
        }
    }
}

impl<'a, M> RecvStoppedEffect<'a, M> {
    pub fn new(recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>, m: Option<M>) -> Self {
        Self {
            recv_effect_out_m_sender,
            m,
        }
    }
}

impl<'a, M> RecvNoMoreSendersEffect<'a, M> {
    pub fn new(recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>) -> Self {
        Self {
            recv_effect_out_m_sender,
        }
    }
}

impl<'a> SpawnEffect<'a> {
    pub fn new(spawn_effect_out_sender: &'a mut mpsc::Sender<()>, testkit: Option<AnyTestkit>) -> Self {
        Self {
            spawn_effect_out_sender,
            testkit,
        }
    }

    pub fn unwrap_testkit(&mut self) -> AnyTestkit {
        self.testkit.take().unwrap()
    }
}

impl<'a, M> EffectExecution for RecvMessageEffect<'a, M> {
    fn execute(self) -> EffectExecutionWitness {
        self.recv_effect_out_m_sender
            .try_send(Recv::Message(self.m))
            .expect("could not send effect back to actor");
        EffectExecutionWitness { _private: () }
    }
}

impl<'a, M> EffectExecution for RecvStoppedEffect<'a, M> {
    fn execute(self) -> EffectExecutionWitness {
        self.recv_effect_out_m_sender
            .try_send(Recv::Stopped(self.m))
            .expect("could not send effect back to actor");
        EffectExecutionWitness { _private: () }
    }
}

impl<'a, M> EffectExecution for RecvNoMoreSendersEffect<'a, M> {
    fn execute(self) -> EffectExecutionWitness {
        self.recv_effect_out_m_sender
            .try_send(Recv::NoMoreSenders)
            .expect("could not send effect back to actor");
        EffectExecutionWitness { _private: () }
    }
}

impl<'a> EffectExecution for SpawnEffect<'a> {
    fn execute(self) -> EffectExecutionWitness {
        self.spawn_effect_out_sender
            .try_send(())
            .expect("could not send effect back to actor");
        EffectExecutionWitness { _private: () }
    }
}

impl EffectExecution for ReturnedEffect {
    fn execute(self) -> EffectExecutionWitness {
        EffectExecutionWitness { _private: () }
    }
}
