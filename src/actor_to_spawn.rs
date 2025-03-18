use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;

pub struct ActorToSpawn<M, RT> {
    pub task: RT,
    pub guard: ActorDropGuard,
    pub actor_ref: ActorRef<M>,
}

impl<M, RT> ActorToSpawn<M, RT> {
    pub(crate) const fn new(task: RT, guard: ActorDropGuard, actor_ref: ActorRef<M>) -> Self {
        Self { task, guard, actor_ref }
    }
}
