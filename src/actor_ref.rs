use crate::actor_path::ActorPath;
use std::ops::Deref;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct ActorRef<M>(pub(crate) Arc<ActorRefInner<M>>);

impl<M> Eq for ActorRef<M> {}

impl<M> PartialEq for ActorRef<M> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

// https://github.com/rust-lang/rust/issues/26925
impl<M> Clone for ActorRef<M> {
    fn clone(&self) -> Self {
        ActorRef(Arc::clone(&self.0))
    }
}

impl<M> Deref for ActorRef<M> {
    type Target = ActorRefInner<M>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M> ActorRef<M> {
    pub fn send(&self, message: M) -> bool {
        (self.0.send_fn)(message)
    }

    pub fn narrow<M1>(&self, message: M) -> ActorRef<M1>
    where
        M: 'static,
        M1: Into<M>,
    {
        let inner = self.clone();

        ActorRef::<M1>(Arc::new(ActorRefInner {
            path: self.path.clone(),
            cancellation: self.cancellation.clone(),
            send_fn: Box::new(move |message| inner.send(message.into())),
        }))
    }
}

pub struct ActorRefInner<M: Sized> {
    pub path: ActorPath,
    /// Will get cancelled whenever the DropGuard is dropped.
    pub(crate) cancellation: CancellationToken,
    pub(crate) send_fn: Box<dyn Fn(M) -> bool + Send + Sync>,
}
