use crate::actor_input::ActorInput;
use crate::behavior::setup::Setup;
use crate::behavior::stopped::Stopped;

pub struct Receive<M, O = (), S = ()>(pub(crate)Box<dyn FnMut(ActorInput<M, S>) -> Next<M, O, S> + Send>);

/// Behaviors that can be returned by a [`Receive`] behavior.
pub enum Next<M, O = (), S = ()> {
    Receive(Receive<M, O, S>),
    Same,
    Stopped(Stopped<M, O, S>),
    Setup(Setup<M, O, S>),
    Unhandled,
    Ignore,
    Empty,
}

impl<M, O, S> std::fmt::Debug for Next<M, O, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Next::Receive(_) => write!(f, "Receive"),
            Next::Setup(_) => write!(f, "Setup"),
            Next::Same => write!(f, "Same"),
            Next::Stopped(_) => write!(f, "Stopped"),
            Next::Unhandled => write!(f, "Unhandled"),
            Next::Empty => write!(f, "Empty"),
            Next::Ignore => write!(f, "Ignore"),
        }
    }
}

impl<M, O, S> From<Receive<M, O, S>> for Next<M, O, S> {
    fn from(receive: Receive<M, O, S>) -> Self {
        Next::Receive(receive)
    }
}

impl<M, O, S> From<Setup<M, O, S>> for Next<M, O, S> {
    fn from(setup: Setup<M, O, S>) -> Self {
        Next::Setup(setup)
    }
}

impl<M, O, S> From<Stopped<M, O, S>> for Next<M, O, S> {
    fn from(stopped: Stopped<M, O, S>) -> Self {
        Next::Stopped(stopped)
    }
}
