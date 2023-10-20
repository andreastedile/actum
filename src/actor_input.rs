use crate::actor_context::ActorContext;
use crate::actor_path::ActorPath;
use crate::actor_task::ActorResult;

/// All possible inputs to a [`Receive`] behavior.
///
/// [`Receive`]: crate::behavior::receive::Receive
pub enum ActorInput<'a, M, S> {
    Message {
        context: &'a mut ActorContext<M, S>,
        message: M,
    },
    Supervision {
        context: &'a mut ActorContext<M, S>,
        path: ActorPath,
        supervision: ActorResult<S>,
    },
    PostStop {
        context: &'a ActorContext<M, S>,
    },
}
