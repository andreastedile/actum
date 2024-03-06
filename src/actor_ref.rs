use crate::actor_path::ActorPath;
use std::fmt::{Debug, Formatter};
use tokio::sync::mpsc;

pub struct ActorRef<M> {
    pub path: ActorPath,
    sender: mpsc::Sender<M>,
}

impl<M> Debug for ActorRef<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorRef")
            .field("path", &self.path)
            .finish()
    }
}

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl<M> ActorRef<M> {
    pub(crate) const fn new(path: ActorPath, sender: mpsc::Sender<M>) -> Self {
        Self { path, sender }
    }

    pub fn send(&self, message: M) -> bool {
        self.sender.try_send(message).is_ok()
    }
}
