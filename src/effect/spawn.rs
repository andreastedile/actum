use std::fmt::{Debug, Formatter};

use futures::channel::oneshot;

use crate::testkit::{AnyTestkit, Testkit};

// wrap in Option so that it can be taken in Drop.
pub struct SpawnEffect(Option<SpawnEffectInner>);

pub struct SpawnEffectInner {
    testkit: Option<AnyTestkit>,
    spawn_effect_out_sender: oneshot::Sender<SpawnEffectOut>,
    spawn_effect_in_sender: oneshot::Sender<SpawnEffectIn>,
}

impl SpawnEffect {
    pub(crate) fn new(
        testkit: Option<AnyTestkit>,
        spawn_effect_out_sender: oneshot::Sender<SpawnEffectOut>,
        spawn_effect_in_sender: oneshot::Sender<SpawnEffectIn>,
    ) -> Self {
        Self(Some(SpawnEffectInner {
            testkit,
            spawn_effect_out_sender,
            spawn_effect_in_sender,
        }))
    }

    pub fn testkit(&mut self) -> Option<&mut AnyTestkit> {
        self.0.as_mut().unwrap().testkit.as_mut()
    }
}

impl Debug for SpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Spawn")
    }
}

pub struct SpawnEffectOut {
    pub testkit: Option<AnyTestkit>,
    pub spawn_effect_in_sender: oneshot::Sender<SpawnEffectIn>,
}

impl SpawnEffectOut {
    pub(crate) fn new<M>(testkit: Option<Testkit<M>>, spawn_effect_in_sender: oneshot::Sender<SpawnEffectIn>) -> Self
    where
        M: Send + 'static,
    {
        Self {
            testkit: testkit.map(AnyTestkit::from),
            spawn_effect_in_sender,
        }
    }
}

impl Drop for SpawnEffect {
    fn drop(&mut self) {
        let SpawnEffectInner {
            spawn_effect_out_sender,
            spawn_effect_in_sender,
            ..
        } = self.0.take().unwrap();
        let effect = SpawnEffectIn {
            spawn_effect_out_sender,
        };
        if spawn_effect_in_sender.send(effect).is_err() {
            panic!("could not send the effect back to the actor")
        }
    }
}

pub struct SpawnEffectIn {
    pub spawn_effect_out_sender: oneshot::Sender<SpawnEffectOut>,
}
