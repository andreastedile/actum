use crate::actorcontext::ActorContext;
use crate::behavior::receive::ReceiveBehavior;
use crate::behavior::stopped::StoppedBehavior;

pub type SetupBehavior<M, S = (), O = ()> =
Box<dyn FnMut(&mut ActorContext<M, S>) -> NextBehavior<M, S, O> + Send>;

pub enum NextBehavior<M, S = (), O = ()> {
    Receive(ReceiveBehavior<M, S, O>),
    Stopped(StoppedBehavior<M, S, O>),
    Empty,
    Ignore,
}

impl<M, S, O> std::fmt::Debug for NextBehavior<M, S, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NextBehavior::Receive(_) => write!(f, "Receive"),
            NextBehavior::Stopped(_) => write!(f, "Stopped"),
            NextBehavior::Empty => write!(f, "Empty"),
            NextBehavior::Ignore => write!(f, "Ignore"),
        }
    }
}

pub fn setup<M, S, O>(
    handler: impl FnMut(&mut ActorContext<M, S>) -> NextBehavior<M, S, O> + Send + 'static,
) -> SetupBehavior<M, S, O> {
    Box::new(handler)
}

pub fn stop<M, S, O>(output: O) -> NextBehavior<M, S, O> {
    NextBehavior::Stopped(either::Either::Left(output))
}

pub fn stop_and<M, S, O>(
    handler: impl FnOnce(&ActorContext<M, S>) -> O + Send + 'static,
) -> NextBehavior<M, S, O> {
    NextBehavior::Stopped(either::Either::Right(Box::new(handler)))
}

impl<M, S, O> From<ReceiveBehavior<M, S, O>> for NextBehavior<M, S, O> {
    fn from(behavior: ReceiveBehavior<M, S, O>) -> Self {
        NextBehavior::Receive(behavior)
    }
}

impl<M, S, O> From<StoppedBehavior<M, S, O>> for NextBehavior<M, S, O> {
    fn from(behavior: StoppedBehavior<M, S, O>) -> Self {
        NextBehavior::Stopped(behavior)
    }
}
