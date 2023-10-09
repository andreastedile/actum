use crate::behavior::receive::ReceiveBehavior;
use crate::behavior::setup::SetupBehavior;

pub enum InitialBehavior<M, S = (), O = ()> {
    Receive(ReceiveBehavior<M, S, O>),
    Setup(SetupBehavior<M, S, O>),
    Empty,
    Ignore,
}

impl<M, S, O> std::fmt::Debug for InitialBehavior<M, S, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitialBehavior::Receive(_) => write!(f, "Receive"),
            InitialBehavior::Setup(_) => write!(f, "Setup"),
            InitialBehavior::Empty => write!(f, "Empty"),
            InitialBehavior::Ignore => write!(f, "Ignore"),
        }
    }
}

impl<M, S, O> From<SetupBehavior<M, S, O>> for InitialBehavior<M, S, O> {
    fn from(setup: SetupBehavior<M, S, O>) -> Self {
        InitialBehavior::Setup(setup)
    }
}

impl<M, S, O> From<ReceiveBehavior<M, S, O>> for InitialBehavior<M, S, O> {
    fn from(receive: ReceiveBehavior<M, S, O>) -> Self {
        InitialBehavior::Receive(receive)
    }
}
