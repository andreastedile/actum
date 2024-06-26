use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;

pub struct Actor<M, RT> {
    pub task: RT,
    pub guard: ActorDropGuard,
    pub m_ref: ActorRef<M>,
}

impl<M, RT> Actor<M, RT> {
    pub(crate) const fn new(task: RT, guard: ActorDropGuard, m_ref: ActorRef<M>) -> Self {
        Self { task, guard, m_ref }
    }
}
