use crate::testkit::{AnyTestkit, Testkit};
use std::fmt::{Debug, Formatter};

pub struct UntypedSpawnEffect {
    pub(crate) any_testkit: Option<AnyTestkit>,
}

impl Debug for UntypedSpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Testkit")
    }
}

impl UntypedSpawnEffect {
    pub fn downcast<M: 'static, Ret: 'static>(&mut self) -> Option<Testkit<M, Ret>> {
        self.any_testkit.take().unwrap().downcast::<M, Ret>()
    }

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(&mut self) -> Testkit<M, Ret> {
        self.any_testkit.take().unwrap().downcast_unwrap::<M, Ret>()
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
