use crate::core::actor_ref::ActorRef;

pub struct CreateActorResult<M, Scoped> {
    pub task: Scoped,
    pub actor_ref: ActorRef<M>,
}

impl<M, Scoped> CreateActorResult<M, Scoped> {
    pub(crate) const fn new(task: Scoped, actor_ref: ActorRef<M>) -> Self {
        Self { task, actor_ref }
    }
}
