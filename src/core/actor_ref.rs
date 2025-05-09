use enum_as_inner::EnumAsInner;
use futures::channel::mpsc;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

/// Reference to an actor that can be used to send messages to it, enabling communication.
///
/// It is first obtained by creating an actor either at the top level of the actor tree hierarchy or as a child of an existing actor.
/// In both cases, both the newly created actor and the caller that created the actor obtain a copy of the reference.
///
/// It can be cloned and shared between actors in messages.
///
/// **Reference count**: If all references to an actor are dropped (including the actor's own copy — that is, no more senders exist), any subsequent call by the actor to the [recv](crate::core::receive_message::ReceiveMessage::recv) method of its receiver will return the [NoMoreSenders](crate::core::receive_message::Recv::NoMoreSenders) variant.
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

    /// Attempts to send a message to the referenced actor, returning the message if an error occurs.
    ///
    /// Errors if the actor's receiver has been dropped or if the underlying channel is full.
    /// The receiver may be dropped in two cases:
    /// 1. The actor's future has completed, in which case the receiver is automatically dropped at the end of its scope.
    /// 2. The receiver is explicitly dropped by the user (for example, to decrease the [reference count](ActorRef)).
    ///
    /// Therefore, a send error does not necessarily indicate that the actor's future has completed.
    pub fn try_send(&mut self, message: M) -> Result<(), TrySendError<M>> {
        self.m_sender.try_send(message).map_err(|err| {
            if err.is_full() {
                TrySendError::Full(err.into_inner())
            } else {
                TrySendError::ReceiverDropped(err.into_inner())
            }
        })
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

/// Error returned from [`try_send`](ActorRef::try_send).
#[derive(EnumAsInner)]
pub enum TrySendError<M> {
    /// The error is a result of the receiver being dropped.
    Full(M),
    /// The error is a result of the receiver being dropped.
    ReceiverDropped(M),
}

impl<M> TrySendError<M> {
    pub fn into_message(self) -> M {
        match self {
            TrySendError::Full(m) => m,
            TrySendError::ReceiverDropped(m) => m,
        }
    }
}

impl<M> Error for TrySendError<M> {}

impl<M> Debug for TrySendError<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("TrySendError::Full"),
            TrySendError::ReceiverDropped(_) => f.write_str("TrySendError::ReceiverDropped"),
        }
    }
}

impl<M> Display for TrySendError<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("the channel is full"),
            TrySendError::ReceiverDropped(_) => f.write_str("the receiver dropped"),
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
