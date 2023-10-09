use crate::actorcontext::ActorContext;
use crate::actorinput::ActorInput;
use crate::actorpath::ActorPath;
use crate::actortask;
use crate::behavior::setup::SetupBehavior;
use crate::behavior::stopped::StoppedBehavior;

pub type ReceiveBehavior<M, S = (), O = ()> =
    Box<dyn FnMut(ActorInput<M, S>) -> NextBehavior<M, S, O> + Send>;

pub enum NextBehavior<M, S = (), O = ()> {
    Receive(ReceiveBehavior<M, S, O>),
    Setup(SetupBehavior<M, S, O>),
    Same,
    Stopped(StoppedBehavior<M, S, O>),
    Unhandled,
    Empty,
    Ignore,
}

impl<M, S, O> std::fmt::Debug for NextBehavior<M, S, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NextBehavior::Receive(_) => write!(f, "Receive"),
            NextBehavior::Setup(_) => write!(f, "Setup"),
            NextBehavior::Same => write!(f, "Same"),
            NextBehavior::Stopped(_) => write!(f, "Stopped"),
            NextBehavior::Unhandled => write!(f, "Unhandled"),
            NextBehavior::Empty => write!(f, "Empty"),
            NextBehavior::Ignore => write!(f, "Ignore"),
        }
    }
}

pub fn receive<M, S, O>(
    handler: impl FnMut(ActorInput<M, S>) -> NextBehavior<M, S, O> + Send + 'static,
) -> ReceiveBehavior<M, S, O> {
    Box::new(handler)
}

pub fn receive_message<M, S, O>(
    mut handler: impl FnMut(&mut ActorContext<M, S>, &M) -> NextBehavior<M, S, O> + Send + 'static,
) -> ReceiveBehavior<M, S, O> {
    Box::new(move |input| {
        if let ActorInput::Message {
            actor_context: context,
            message,
        } = input
        {
            handler(context, message)
        } else {
            NextBehavior::Unhandled
        }
    })
}

pub fn receive_stopped<M, S, O>(
    mut handler: impl FnMut(
            &mut ActorContext<M, S>,
            &ActorPath,
            Option<Result<Option<&S>, &actortask::ActorError>>,
        ) -> NextBehavior<M, S, O>
        + Send
        + 'static,
) -> ReceiveBehavior<M, S, O> {
    Box::new(move |input| {
        if let ActorInput::Stopped {
            actor_context: context,
            actor_path: path,
            actor_result: output,
        } = input
        {
            handler(context, path, output)
        } else {
            NextBehavior::Unhandled
        }
    })
}

pub fn receive_poststop<M, S, O>(
    mut handler: impl FnMut(&ActorContext<M, S>) -> O + Send + 'static,
) -> ReceiveBehavior<M, S, O> {
    Box::new(move |input| {
        if let ActorInput::PostStop {
            actor_context: context,
        } = input
        {
            NextBehavior::Stopped(either::Either::Left(handler(context)))
        } else {
            NextBehavior::Unhandled
        }
    })
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

impl<M, S, O> From<SetupBehavior<M, S, O>> for NextBehavior<M, S, O> {
    fn from(behavior: SetupBehavior<M, S, O>) -> Self {
        NextBehavior::Setup(behavior)
    }
}

impl<M, S, O> From<StoppedBehavior<M, S, O>> for NextBehavior<M, S, O> {
    fn from(behavior: StoppedBehavior<M, S, O>) -> Self {
        NextBehavior::Stopped(behavior)
    }
}
