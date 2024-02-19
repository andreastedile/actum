use crate::actor_path::ActorPath;
use std::any::Any;
use std::fmt::{Debug, Formatter};

pub enum ActorInput<M> {
    Message(M),
    Supervision {
        path: ActorPath,
        panic: Option<Box<dyn Any + Send>>,
    },
}

impl<M> Debug for ActorInput<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(_) => f.write_str("Message"),
            Self::Supervision { path, panic } => f
                .debug_struct("Supervision")
                .field("path", path)
                .field("panic", panic)
                .finish(),
        }
    }
}
