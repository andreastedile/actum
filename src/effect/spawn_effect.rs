use crate::testkit::{Testkit, UntypedTestkit};
use std::fmt::{Debug, Formatter};

pub(crate) struct UntypedSpawnEffectImpl {
    /// Wrapped in Option so that it can be taken.
    pub untyped_testkit: Option<UntypedTestkit>,
}

impl Debug for UntypedSpawnEffectImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect")
            .field("untyped_testkit", &self.untyped_testkit)
            .finish()
    }
}

pub struct UntypedSpawnEffect {
    pub untyped_testkit: UntypedTestkit,
}

impl Debug for UntypedSpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("UntypedSpawnEffect")
    }
}

impl UntypedSpawnEffect {
    pub fn downcast<M: 'static, Ret: 'static>(mut self) -> Result<SpawnEffect<M, Ret>, Self> {
        match self.untyped_testkit.downcast::<M, Ret>() {
            Ok(testkit) => Ok(SpawnEffect { testkit }),
            Err(untyped_testkit) => {
                self.untyped_testkit = untyped_testkit;
                Err(self)
            }
        }
    }

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> SpawnEffect<M, Ret> {
        SpawnEffect {
            testkit: self.untyped_testkit.downcast::<M, Ret>().unwrap(),
        }
    }
}

pub struct SpawnEffect<M, Ret> {
    pub testkit: Testkit<M, Ret>,
}

impl<M, Ret> Debug for SpawnEffect<M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect").field("testkit", &self.testkit).finish()
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

pub(crate) struct SpawnEffectFromTestkitToActor;

impl Debug for SpawnEffectFromTestkitToActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpawnEffect")
    }
}
