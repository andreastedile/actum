use std::fmt::{Debug, Formatter};

pub(crate) struct CompletedEffectPrivate<Output> {
    pub output: Output,
}

pub struct CompletedEffect<'a, Output> {
    pub output: &'a Output,
}

impl<'a, Output> Debug for CompletedEffect<'a, Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedEffect").finish_non_exhaustive()
    }
}

/// From the actor under test to the testkit.
pub(crate) struct CompletedEffectToTestkit<Output> {
    pub output: Output,
}

impl<Output> Debug for CompletedEffectToTestkit<Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedEffect").finish_non_exhaustive()
    }
}

/// From the testkit to the actor under test.
pub(crate) struct CompletedEffectToActor<Output> {
    pub output: Output,
}

impl<Output> Debug for CompletedEffectToActor<Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedEffect").finish_non_exhaustive()
    }
}

impl<Output> From<CompletedEffectPrivate<Output>> for CompletedEffectToActor<Output> {
    fn from(effect: CompletedEffectPrivate<Output>) -> Self {
        CompletedEffectToActor { output: effect.output }
    }
}
