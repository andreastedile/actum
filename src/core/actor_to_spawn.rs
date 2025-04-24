use crate::core::actor_ref::ActorRef;

pub struct ActorToSpawn<M, RT> {
    pub task: RT,
    pub actor_ref: ActorRef<M>,
}

impl<M, RT> ActorToSpawn<M, RT> {
    pub(crate) const fn new(task: RT, actor_ref: ActorRef<M>) -> Self {
        Self { task, actor_ref }
    }
}
