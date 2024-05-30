use std::fmt::{Debug, Formatter};

use futures::channel::oneshot;

use crate::testkit::{AnyTestkit, Testkit};

pub struct SpawnEffect {
    testkit: Option<AnyTestkit>,
    // wrap in Option so that it can be taken.
    spawn_effect_out_sender: Option<oneshot::Sender<SpawnEffectOut>>,
    // wrap in Option so that it can be taken.
    spawn_effect_in_sender: Option<oneshot::Sender<SpawnEffectIn>>,
}

impl SpawnEffect {
    pub(crate) fn new(
        testkit: Option<AnyTestkit>,
        spawn_effect_out_sender: oneshot::Sender<SpawnEffectOut>,
        spawn_effect_in_sender: oneshot::Sender<SpawnEffectIn>,
    ) -> Self {
        Self {
            testkit,
            spawn_effect_out_sender: Some(spawn_effect_out_sender),
            spawn_effect_in_sender: Some(spawn_effect_in_sender),
        }
    }

    pub fn testkit(&mut self) -> Option<&mut AnyTestkit> {
        self.testkit.as_mut()
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
        if let Some(spawn_effect_in_sender) = self.spawn_effect_in_sender.take() {
            let effect = SpawnEffectIn {
                spawn_effect_out_sender: self.spawn_effect_out_sender.take().unwrap(),
            };
            if spawn_effect_in_sender.send(effect).is_err() {
                panic!("could not send the effect back to the actor")
            }
        }
    }
}

pub struct SpawnEffectIn {
    pub spawn_effect_out_sender: oneshot::Sender<SpawnEffectOut>,
}
