use crate::actor_context::ActorContext;
use crate::behavior::receive::Receive;
use crate::behavior::stopped::Stopped;

pub struct Setup<M, O = (), S = ()>(pub(crate) Box<dyn FnOnce(&mut ActorContext<M, S>) -> Next<M, O, S> + Send>);

impl<M, O, S> std::ops::Deref for Setup<M, O, S> {
    type Target = Box<dyn FnOnce(&mut ActorContext<M, S>) -> Next<M, O, S> + Send>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M, O, S> std::ops::DerefMut for Setup<M, O, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Behaviors that can be returned by a [`Setup`] behavior.
pub enum Next<M, O = (), S = ()> {
    Receive(Receive<M, O, S>),
    Stopped(Stopped<M, O, S>),
    Ignore,
    Empty,
}

impl<M, O, S> std::fmt::Debug for Next<M, O, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Next::Receive(_) => write!(f, "Receive"),
            Next::Stopped(_) => write!(f, "Stopped"),
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

impl<M, O, S> From<Stopped<M, O, S>> for Next<M, O, S> {
    fn from(stopped: Stopped<M, O, S>) -> Self {
        Next::Stopped(stopped)
    }
}
