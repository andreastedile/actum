use futures::channel::mpsc;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

pub struct ActorRef<M> {
    m_sender: mpsc::Sender<M>,
}

impl<M> Debug for ActorRef<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorRef")
            .field("closed", &self.m_sender.is_closed())
            .finish()
    }
}

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        Self {
            m_sender: self.m_sender.clone(),
        }
    }
}

impl<M> PartialEq<Self> for ActorRef<M> {
    fn eq(&self, other: &Self) -> bool {
        self.m_sender.same_receiver(&other.m_sender)
    }
}

impl<M> Eq for ActorRef<M> {}

impl<M> ActorRef<M> {
    pub(crate) const fn new(m_sender: mpsc::Sender<M>) -> Self {
        Self { m_sender }
    }

    /// Attempts to send a message to the actor behind this reference, returning the message if there was an error.
    ///
    /// Errors if the [receiver](crate::core::receive_message::ReceiveMessage) of the intended actor has been dropped,
    /// either manually or automatically upon return.
    pub fn try_send(&mut self, message: M) -> Result<(), M> {
        self.m_sender.try_send(message).map_err(mpsc::TrySendError::into_inner)
    }

    pub fn narrow<Subtype, F>(mut self, f: F) -> MappedActorRef<Subtype>
    where
        M: Send + 'static,
        F: Fn(Subtype) -> M + Send + 'static,
    {
        MappedActorRef {
            f: Arc::new(Mutex::new(move |subtype: Subtype| {
                let m = f(subtype);
                self.try_send(m).is_ok()
            })),
        }
    }

    pub fn widen<Supertype, F>(mut self, f: F) -> MappedActorRef<Supertype>
    where
        M: Send + 'static,
        F: Fn(Supertype) -> M + Send + 'static,
    {
        MappedActorRef {
            f: Arc::new(Mutex::new(move |supertype: Supertype| {
                let m = f(supertype);
                self.try_send(m).is_ok()
            })),
        }
    }
}

pub struct MappedActorRef<M> {
    f: Arc<Mutex<dyn FnMut(M) -> bool + Send + 'static>>,
}

impl<M> Clone for MappedActorRef<M> {
    fn clone(&self) -> Self {
        Self { f: Arc::clone(&self.f) }
    }
}

impl<M> PartialEq<Self> for MappedActorRef<M> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.f, &other.f)
    }
}

impl<M> Eq for MappedActorRef<M> {}

impl<M> Debug for MappedActorRef<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("MappedActorRef")
    }
}

impl<M> MappedActorRef<M> {
    pub fn try_send(&mut self, message: M) -> bool {
        let mut f = self.f.lock().unwrap();
        f(message)
    }
}

#[cfg(test)]
mod actorref_mapping_tests {
    use super::*;

    #[derive(Debug)]
    enum ABC {
        A,
        B,
        C,
    }

    #[derive(Debug)]
    enum BC {
        B,
        C,
    }

    impl From<BC> for ABC {
        fn from(value: BC) -> Self {
            match value {
                BC::B => Self::B,
                BC::C => Self::C,
            }
        }
    }

    #[test]
    fn test_actorref_narrow() {
        let m_channel = mpsc::channel::<ABC>(10);
        let actor_ref = ActorRef { m_sender: m_channel.0 };
        let mut narrower = actor_ref.narrow::<BC, _>(|bc| bc.into());
        assert!(narrower.try_send(BC::B));
        assert!(narrower.try_send(BC::C));
    }

    #[test]
    fn test_actorref_widen() {
        let m_channel = mpsc::channel::<BC>(10);
        let actor_ref = ActorRef { m_sender: m_channel.0 };
        let mut wider = actor_ref.widen::<ABC, _>(|abc| match abc {
            ABC::A => unreachable!(),
            ABC::B => BC::B,
            ABC::C => BC::C,
        });
        assert!(wider.try_send(ABC::B));
        assert!(wider.try_send(ABC::C));
    }

    #[test]
    #[should_panic]
    fn test_actorref_widen_panic() {
        let m_channel = mpsc::channel::<BC>(10);
        let actor_ref = ActorRef { m_sender: m_channel.0 };
        let mut wider = actor_ref.widen::<ABC, _>(|abc| match abc {
            ABC::A => panic!(),
            ABC::B => BC::B,
            ABC::C => BC::C,
        });
        wider.try_send(ABC::A);
    }
}
