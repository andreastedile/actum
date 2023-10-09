use crate::actorcontext::ActorContext;
use crate::actorpath::ActorPath;
use crate::actortask;

pub enum ActorInput<'a, M, S = ()> {
    Message {
        actor_context: &'a mut ActorContext<M, S>,
        message: &'a M,
    },
    Stopped {
        actor_context: &'a mut ActorContext<M, S>,
        actor_path: &'a ActorPath,
        actor_result: Option<Result<Option<&'a S>, &'a actortask::ActorError>>,
    },
    PostStop {
        actor_context: &'a ActorContext<M, S>,
    },
}
