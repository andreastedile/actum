use std::fmt::{Debug, Formatter};

pub(crate) struct ReturnedEffectPrivate<Output> {
    pub output: Output,
}

pub struct ReturnedEffect<'a, Output> {
    pub output: &'a Output,
}

impl<'a, Output> Debug for ReturnedEffect<'a, Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

/// From the actor under test to the testkit.
pub(crate) struct ReturnedEffectToTestkit<Output> {
    pub output: Output,
}

impl<Output> Debug for ReturnedEffectToTestkit<Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

/// From the testkit to the actor under test.
pub(crate) struct ReturnedEffectToActor<Output> {
    pub output: Output,
}

impl<Output> Debug for ReturnedEffectToActor<Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

impl<Output> From<ReturnedEffectPrivate<Output>> for ReturnedEffectToActor<Output> {
    fn from(effect: ReturnedEffectPrivate<Output>) -> Self {
        ReturnedEffectToActor { output: effect.output }
    }
}
