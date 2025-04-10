use crate::actor_task::{BoxTestActor, UntypedBoxTestActor};
use crate::testkit::{Testkit, UntypedTestkit};
use std::fmt::{Debug, Formatter};

pub(crate) struct UntypedSpawnEffectImpl {
    /// Wrapped in Option so that it can be taken.
    pub untyped_testkit: Option<UntypedTestkit>,
    pub injected: Option<UntypedBoxTestActor>,
}

impl Debug for UntypedSpawnEffectImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect")
            .field("untyped_testkit", &self.untyped_testkit)
            .field("injected", &self.injected)
            .finish()
    }
}

pub struct UntypedSpawnEffect<'a> {
    pub untyped_testkit: UntypedTestkit,
    pub(crate) injected: &'a mut Option<UntypedBoxTestActor>,
}

impl<'a> Debug for UntypedSpawnEffect<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UntypedSpawnEffect")
            .field("untyped_testkit", &self.untyped_testkit)
            .field("injected", &self.injected)
            .finish()
    }
}

impl<'a> UntypedSpawnEffect<'a> {
    pub fn downcast<M: 'static, Ret: 'static>(mut self) -> Result<SpawnEffect<'a, M, Ret>, Self> {
        match self.untyped_testkit.downcast::<M, Ret>() {
            Ok(testkit) => Ok(SpawnEffect {
                testkit,
                injected: &mut *self.injected,
            }),
            Err(untyped_testkit) => {
                self.untyped_testkit = untyped_testkit;
                Err(self)
            }
        }
    }

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> SpawnEffect<'a, M, Ret> {
        SpawnEffect {
            testkit: self.untyped_testkit.downcast::<M, Ret>().unwrap(),
            injected: &mut *self.injected,
        }
    }
}

pub struct SpawnEffect<'a, M, Ret> {
    pub testkit: Testkit<M, Ret>,
    pub(crate) injected: &'a mut Option<UntypedBoxTestActor>,
}

impl<'a, M, Ret> Debug for SpawnEffect<'a, M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect").field("testkit", &self.testkit).finish()
    }
}

impl<'a, M, Ret> SpawnEffect<'a, M, Ret> {
    pub fn inject_actor(self, actor: BoxTestActor<M, Ret>) -> Testkit<M, Ret>
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        *self.injected = Some(actor.into());
        self.testkit
    }
}

pub(crate) struct UntypedSpawnEffectFromActorToTestkit {
    pub untyped_testkit: UntypedTestkit,
}

impl Debug for UntypedSpawnEffectFromActorToTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect")
            .field("testkit", &self.untyped_testkit)
            .finish()
    }
}

pub(crate) struct SpawnEffectFromTestkitToActor {
    pub injected: Option<UntypedBoxTestActor>,
}

impl Debug for SpawnEffectFromTestkitToActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect").field("injected", &self.injected).finish()
    }
}
