use std::fmt::{Debug, Formatter};

pub(crate) struct ReturnedEffectImpl<Output> {
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

pub(crate) struct ReturnedEffectFromActorToTestkit<Output> {
    pub output: Output,
}

impl<Output> Debug for ReturnedEffectFromActorToTestkit<Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

pub(crate) struct ReturnedEffectFromTestkitToActor<Output> {
    pub output: Output,
}

impl<Output> Debug for ReturnedEffectFromTestkitToActor<Output> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}
