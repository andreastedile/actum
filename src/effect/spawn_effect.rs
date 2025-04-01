use crate::testkit::{AnyTestkit, Testkit};
use std::fmt::{Debug, Formatter};

pub struct UntypedSpawnEffect {
    pub(crate) any_testkit: Option<AnyTestkit>,
}

impl Debug for UntypedSpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UntypedSpawnEffect")
            .field("testkit", &self.any_testkit)
            .finish()
    }
}

impl UntypedSpawnEffect {
    pub fn downcast<M: 'static, Ret: 'static>(&mut self) -> Option<SpawnEffect<M, Ret>> {
        if let Some(mut any_testkit) = self.any_testkit.take() {
            if let Some(testkit) = any_testkit.downcast::<M, Ret>() {
                Some(SpawnEffect { testkit })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(&mut self) -> SpawnEffect<M, Ret> {
        SpawnEffect {
            testkit: self.any_testkit.take().unwrap().downcast_unwrap::<M, Ret>(),
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

pub struct UntypedSpawnEffectFromActorToTestkit {
    pub any_testkit: AnyTestkit,
}

impl Debug for UntypedSpawnEffectFromActorToTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnEffect")
            .field("any_testkit", &self.any_testkit)
            .finish()
    }
}

pub struct SpawnEffectFromTestkitToActor;

impl Debug for SpawnEffectFromTestkitToActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpawnEffect")
    }
}
