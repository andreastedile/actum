use crate::actor_context::ActorContext;
use either::Either;

pub struct Stopped<M, O = (), S = ()>(pub(crate) Either<O, Box<dyn FnOnce(&ActorContext<M, S>) -> O + Send>>);
