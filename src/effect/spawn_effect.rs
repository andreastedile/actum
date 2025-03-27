use crate::testkit::AnyTestkit;
use std::fmt::{Debug, Formatter};

pub struct SpawnEffect {
    pub any_testkit: AnyTestkit,
}

impl Debug for SpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Testkit")
    }
}

pub struct SpawnEffectFromActorToTestkit {
    pub any_testkit: Option<AnyTestkit>,
}

impl Debug for SpawnEffectFromActorToTestkit {
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
