use futures::channel::oneshot;

use crate::actor_cell::Stop;

/// Gracefully stops the child actor when dropped.
///
/// # Example
/// ```
/// use actum::prelude::*;
///
/// async fn root<AB>(mut cell: AB, mut me: ActorRef<u64>)
/// where
///     AB: ActorBounds<u64>,
/// {
///     while let Recv::Message(m) = cell.recv().await {
///         println!("{}", m);
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut root = actum(root);
///     root.m_ref.try_send(1).unwrap();
///     drop(root.guard);
///     let _ = root.task.run_task().await;
/// }
/// ```
pub struct ActorDropGuard(Option<oneshot::Sender<Stop>>);

impl Drop for ActorDropGuard {
    fn drop(&mut self) {
        let _ = self.0.take().unwrap().send(Stop);
    }
}

impl ActorDropGuard {
    pub(crate) const fn new(stop_sender: oneshot::Sender<Stop>) -> Self {
        Self(Some(stop_sender))
    }
}
