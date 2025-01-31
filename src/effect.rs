use crate::actor_bounds::Recv;
use crate::testkit::AnyTestkit;
use futures::channel::mpsc;
use std::fmt::{Debug, Formatter};

/// TODO
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

impl<'a, M> EffectType<'a, M> {
    pub fn unwrap_message(&'a mut self) -> &'a mut RecvMessageEffect<'a, M> {
        match self {
            EffectType::Message(inner) => inner,
            other => panic!("called `EffectType::unwrap_message()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_stopped(&'a mut self) -> &'a mut RecvStoppedEffect<'a, M> {
        match self {
            EffectType::Stopped(inner) => inner,
            other => panic!("called `EffectType::unwrap_stopped()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_no_more_senders(&'a mut self) -> &'a mut RecvNoMoreSendersEffect<'a, M> {
        match self {
            EffectType::NoMoreSenders(inner) => inner,
            other => panic!("called `EffectType::unwrap_no_more_senders()` on a `{:?}` value", other),
        }
    }

    pub fn unwrap_spawn(&'a mut self) -> &'a mut SpawnEffect<'a> {
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
    pub(crate) recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>,
    pub m: M,
}

pub struct RecvStoppedEffect<'a, M> {
    pub(crate) recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>,
    pub m: Option<M>,
}

pub struct RecvNoMoreSendersEffect<'a, M> {
    pub(crate) recv_effect_out_m_sender: &'a mut mpsc::Sender<Recv<M>>,
}

pub struct SpawnEffect<'a> {
    pub(crate) spawn_effect_out_sender: &'a mut mpsc::Sender<()>,
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
