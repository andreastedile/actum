use crate::actor_test::run_task::{BoxTestActor, UntypedBoxTestActor};
use crate::actor_test::testkit::{Testkit, UntypedTestkit};
use std::fmt::{Debug, Formatter};

pub(crate) struct UntypedCreateChildEffectImpl {
    /// Wrapped in Option so that it can be taken.
    pub untyped_testkit: Option<UntypedTestkit>,
    pub injected: Option<UntypedBoxTestActor>,
}

impl Debug for UntypedCreateChildEffectImpl {
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
    pub fn downcast<M: 'static, Ret: 'static>(mut self) -> Result<CreateChildEffect<'a, M, Ret>, Self> {
        match self.untyped_testkit.downcast::<M, Ret>() {
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

    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> CreateChildEffect<'a, M, Ret> {
        CreateChildEffect {
            testkit: self.untyped_testkit.downcast::<M, Ret>().unwrap(),
            injected: &mut *self.injected,
        }
    }
}

pub struct CreateChildEffect<'a, M, Ret> {
    pub testkit: Testkit<M, Ret>,
    pub(crate) injected: &'a mut Option<UntypedBoxTestActor>,
}

impl<'a, M, Ret> Debug for CreateChildEffect<'a, M, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("testkit", &self.testkit)
            .finish()
    }
}

impl<'a, M, Ret> CreateChildEffect<'a, M, Ret> {
    pub fn inject_actor(self, actor: BoxTestActor<M, Ret>) -> Testkit<M, Ret>
    where
        M: Send + 'static,
        Ret: Send + 'static,
    {
        *self.injected = Some(actor.into());
        self.testkit
    }
}

pub(crate) struct UntypedCreateChildEffectFromActorToTestkit {
    pub untyped_testkit: UntypedTestkit,
}

impl Debug for UntypedCreateChildEffectFromActorToTestkit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("testkit", &self.untyped_testkit)
            .finish()
    }
}

pub(crate) struct CreateChildEffectFromTestkitToActor {
    pub injected: Option<UntypedBoxTestActor>,
}

impl Debug for CreateChildEffectFromTestkitToActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateChildEffect")
            .field("injected", &self.injected)
            .finish()
    }
}
