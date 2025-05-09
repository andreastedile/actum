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

    pub fn narrow<Subtype>(mut self) -> MappedActorRef<Subtype>
    where
        M: From<Subtype> + Send + 'static,
        Subtype: TryFrom<M>,
    {
        MappedActorRef {
            f: Arc::new(Mutex::new(move |subtype: Subtype| {
                let m = subtype.into();
                self.try_send(m).map_err(|m| {
                    m.try_into().unwrap_or_else(|_| {
                        panic!(
                            "conversion failed from {} to {}",
                            std::any::type_name::<M>(),
                            std::any::type_name::<Subtype>()
                        )
                    })
                })
            })),
        }
    }

    pub fn widen<Supertype>(mut self) -> MappedActorRef<Supertype>
    where
        M: TryFrom<Supertype> + Send + 'static,
        Supertype: From<M>,
    {
        MappedActorRef {
            f: Arc::new(Mutex::new(move |supertype: Supertype| {
                let m = supertype.try_into().unwrap_or_else(|_| {
                    panic!(
                        "conversion failed from {} to {}",
                        std::any::type_name::<Supertype>(),
                        std::any::type_name::<M>()
                    )
                });
                self.try_send(m).map_err(|m| m.into())
            })),
        }
    }
}

pub struct MappedActorRef<M> {
    f: Arc<Mutex<dyn FnMut(M) -> Result<(), M> + Send + 'static>>,
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
    pub fn try_send(&mut self, message: M) -> Result<(), M> {
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

    impl TryFrom<ABC> for BC {
        type Error = ();

        fn try_from(value: ABC) -> Result<Self, Self::Error> {
            match value {
                ABC::A => Err(()),
                ABC::B => Ok(BC::B),
                ABC::C => Ok(BC::C),
            }
        }
    }

    #[test]
    fn test_actorref_narrow() {
        let m_channel = mpsc::channel::<ABC>(10);
        let actor_ref = ActorRef { m_sender: m_channel.0 };
        let mut narrower = actor_ref.narrow::<BC>();
        narrower.try_send(BC::B).unwrap();
        narrower.try_send(BC::C).unwrap();
    }

    #[test]
    fn test_actorref_widen() {
        let m_channel = mpsc::channel::<BC>(10);
        let actor_ref = ActorRef { m_sender: m_channel.0 };
        let mut wider = actor_ref.widen::<ABC>();
        wider.try_send(ABC::B).unwrap();
        wider.try_send(ABC::C).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_actorref_widen_panic() {
        let m_channel = mpsc::channel::<BC>(10);
        let actor_ref = ActorRef { m_sender: m_channel.0 };
        let mut wider = actor_ref.widen::<ABC>();
        wider.try_send(ABC::A).unwrap();
    }
}
