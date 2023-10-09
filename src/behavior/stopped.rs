use crate::actorcontext::ActorContext;

pub type StoppedBehavior<M, S = (), O = ()> =
    either::Either<O, Box<dyn FnOnce(&ActorContext<M, S>) -> O + Send>>;
