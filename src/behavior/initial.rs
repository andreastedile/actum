use crate::behavior::receive::Receive;
use crate::behavior::setup::Setup;

pub enum Initial<M, O = (), S = ()> {
    Setup(Setup<M, O, S>),
    Receive(Receive<M, O, S>),
}

impl<M, O, S> std::fmt::Debug for Initial<M, O, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Initial::Receive(_) => write!(f, "Receive"),
            Initial::Setup(_) => write!(f, "Setup"),
        }
    }
}

impl<M, O, S> From<Setup<M, O, S>> for Initial<M, O, S> {
    fn from(setup: Setup<M, O, S>) -> Self {
        Initial::Setup(setup)
    }
}

impl<M, O, S> From<Receive<M, O, S>> for Initial<M, O, S> {
    fn from(receive: Receive<M, O, S>) -> Self {
        Initial::Receive(receive)
    }
}
