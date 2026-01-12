use crate::actor_test::run_task::{BoxTestActor, UntypedBoxTestActor};
use crate::actor_test::testkit::{Testkit, UntypedTestkit};
use std::fmt::{Debug, Formatter};

pub(crate) struct UntypedCreateChildEffectPrivate {
    /// Wrapped in Option so that it can be taken.
    pub untyped_testkit: Option<UntypedTestkit>,
    pub injected: Option<UntypedBoxTestActor>,
}

impl Debug for UntypedCreateChildEffectPrivate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("untyped_testkit", &self.untyped_testkit)
            .field("injected", &self.injected)
            .finish()
    }
}

pub struct UntypedCreateChildEffect<'a> {
    pub untyped_testkit: UntypedTestkit,
    pub(crate) injected: &'a mut Option<UntypedBoxTestActor>,
}

impl<'a> Debug for UntypedCreateChildEffect<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UntypedCreateChildEffect")
            .field("untyped_testkit", &self.untyped_testkit)
            .field("injected", &self.injected)
            .finish()
    }
}

impl<'a> UntypedCreateChildEffect<'a> {
    pub fn downcast<M: 'static, Output: 'static>(mut self) -> Result<CreateChildEffect<'a, M, Output>, Self> {
        match self.untyped_testkit.downcast::<M, Output>() {
            Ok(testkit) => Ok(CreateChildEffect {
                testkit,
                injected: &mut *self.injected,
            }),
            Err(untyped_testkit) => {
                self.untyped_testkit = untyped_testkit;
                Err(self)
            }
        }
    }

    pub fn downcast_unwrap<M: 'static, Output: 'static>(self) -> CreateChildEffect<'a, M, Output> {
        CreateChildEffect {
            testkit: self.untyped_testkit.downcast::<M, Output>().unwrap(),
            injected: &mut *self.injected,
        }
    }
}

pub struct CreateChildEffect<'a, M, Output> {
    pub testkit: Testkit<M, Output>,
    pub(crate) injected: &'a mut Option<UntypedBoxTestActor>,
}

impl<'a, M, Output> Debug for CreateChildEffect<'a, M, Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("testkit", &self.testkit)
            .finish()
    }
}

impl<'a, M, Output> CreateChildEffect<'a, M, Output> {
    pub fn inject_actor(self, actor: BoxTestActor<M, Output>) -> Testkit<M, Output>
    where
        M: Send + 'static,
        Output: Send + 'static,
    {
        *self.injected = Some(actor.into());
        self.testkit
    }
}

/// From the actor under test to the testkit.
pub(crate) struct UntypedCreateChildEffectToTestkit {
    pub untyped_testkit: UntypedTestkit,
}

impl Debug for UntypedCreateChildEffectToTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("testkit", &self.untyped_testkit)
            .finish()
    }
}

/// From the testkit to the actor under test.
pub(crate) struct CreateChildEffectToActor {
    pub injected: Option<UntypedBoxTestActor>,
}

impl Debug for CreateChildEffectToActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("injected", &self.injected)
            .finish()
    }
}
