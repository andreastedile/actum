use crate::core::actor_ref::ActorRef;

pub struct CreateActorResult<M, RT> {
    pub task: RT,
    pub actor_ref: ActorRef<M>,
}

impl<M, RT> CreateActorResult<M, RT> {
    pub(crate) const fn new(task: RT, actor_ref: ActorRef<M>) -> Self {
        Self { task, actor_ref }
    }
}
