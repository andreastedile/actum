use std::fmt::{Debug, Formatter};

use futures::channel::oneshot;

use crate::testkit::{AnyTestkit, Testkit};

pub struct SpawnEffect {
    testkit: Option<AnyTestkit>,
    // wrap in Option so that it can be taken in Drop.
    unit_sender: Option<oneshot::Sender<()>>,
}

impl SpawnEffect {
    pub(crate) fn new<M>(testkit: Option<Testkit<M>>, unit_sender: oneshot::Sender<()>) -> Self
    where
        M: Send + 'static,
    {
        Self {
            testkit: testkit.map(AnyTestkit::from),
            unit_sender: Some(unit_sender),
        }
    }

    pub fn testkit(&mut self) -> Option<&mut AnyTestkit> {
        self.testkit.as_mut()
    }

    pub(crate) fn into_inner(mut self) -> Option<AnyTestkit> {
        self.unit_sender = None;
        self.testkit.take()
    }
}

impl Debug for SpawnEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Spawn")
    }
}

impl Drop for SpawnEffect {
    fn drop(&mut self) {
        if let Some(unit_sender) = self.unit_sender.take() {
            if unit_sender.send(()).is_err() {
                panic!("Executor has dropped prematurely")
            }
        } // else: SpawnEffect::into_inner was called
    }
}
