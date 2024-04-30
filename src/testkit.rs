pub mod effect;
pub mod executor;

use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::test_actor::TestBounds;
use crate::actor_cell::{ActorCell, Stop, Stopped};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::testkit::effect::Effect;
use crate::testkit::executor::EffectExecutor;
use futures::channel::{mpsc, oneshot};
use std::future::Future;

pub struct TestKit<M, RT> {
    pub task: RT,
    pub guard: ActorDropGuard,
    pub m_ref: ActorRef<M>,
    pub executor: EffectExecutor<M>,
}

impl<M, RT> TestKit<M, RT> {
    pub(crate) const fn new(task: RT, guard: ActorDropGuard, m_ref: ActorRef<M>, executor: EffectExecutor<M>) -> Self {
        Self {
            task,
            guard,
            m_ref,
            executor,
        }
    }
}

pub fn testkit<M, F, Fut>(f: F) -> TestKit<M, ActorTask<M, F, Fut, TestBounds<M>>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, TestBounds<M>>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let stop_channel = oneshot::channel::<Stop>();
    let stopped_channel = mpsc::unbounded::<Stopped>();
    let m_channel = mpsc::channel::<M>(100);
    let effect_m_channel = mpsc::unbounded::<Effect<M>>();

    let guard = ActorDropGuard::new(stop_channel.0);
    let bounds = TestBounds::new(effect_m_channel.0);
    let cell = ActorCell::new(stop_channel.1, stopped_channel.0, m_channel.1, bounds);

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), stopped_channel.1, None);
    let executor = EffectExecutor::new(effect_m_channel.1);

    TestKit::new(task, guard, m_ref, executor)
}
