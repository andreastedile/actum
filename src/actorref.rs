use crate::actorpath::ActorPath;
use std::fmt::{Debug, Formatter};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct ActorRef<M> {
    pub actor_path: ActorPath,
    pub(crate) sender: mpsc::Sender<M>,
    pub(crate) cancellation: CancellationToken,
}

impl<M> Debug for ActorRef<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", &self.actor_path))
    }
}

impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        ActorRef {
            actor_path: self.actor_path.clone(),
            sender: self.sender.clone(),
            cancellation: self.cancellation.clone(),
        }
    }
}

impl<M> ActorRef<M> {
    pub fn send(&self, message: M) -> bool {
        self.sender.try_send(message).is_ok()
    }
}
