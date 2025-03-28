use std::fmt::{Debug, Formatter};

pub struct ReturnedEffect<Ret> {
    pub ret: Ret,
}

impl<Ret> Debug for ReturnedEffect<Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReturnedEffect")
    }
}

pub struct ReturnedEffectFromActorToTestkit<Ret> {
    pub ret: Ret,
}

impl<Ret> Debug for ReturnedEffectFromActorToTestkit<Ret> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}

pub struct ReturnedEffectFromTestkitToActor<Ret> {
    pub ret: Ret,
}

impl<M> Debug for ReturnedEffectFromTestkitToActor<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReturnedEffect").finish_non_exhaustive()
    }
}
