use std::fmt::{Debug, Formatter};

pub(crate) struct ReturnedEffectImpl<Ret> {
    pub ret: Ret,
}

pub struct ReturnedEffect<'a, Ret> {
    pub ret: &'a Ret,
}

impl<'a, Ret> Debug for ReturnedEffect<'a, Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

pub(crate) struct ReturnedEffectFromActorToTestkit<Ret> {
    pub ret: Ret,
}

impl<Ret> Debug for ReturnedEffectFromActorToTestkit<Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

pub(crate) struct ReturnedEffectFromTestkitToActor<Ret> {
    pub ret: Ret,
}

impl<Ret> Debug for ReturnedEffectFromTestkitToActor<Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}
