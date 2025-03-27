use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use actor_ref::MessageReceiver;
use futures::channel::mpsc;
use std::future::Future;

pub mod actor;
pub mod actor_cell;
pub mod actor_ref;
pub mod actor_task;
pub mod actor_to_spawn;
pub mod children_tracker;
pub mod effect;
pub mod prelude;
pub mod testkit;

/// Creates the root actor of the actor tree hierarchy.
/// The actor should then be spawned onto the runtime of choice.
///
/// Compared to using the [testkit](testkit::actum_with_testkit) function, this function does not incur any overhead.
///
/// # Examples
///
/// ## Pass a function pointer
/// ```
/// use actum::prelude::*;
///
/// async fn root<A>(mut cell: A, mut receiver: MessageReceiver<u64>, mut me: ActorRef<u64>) -> (A, ())
/// where
///     A: Actor<u64>,
/// {
///     let m = cell.recv(&mut receiver).await.into_message().unwrap();
///     println!("{}", m);
///     (cell, ())
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut root = actum(root);
///     root.actor_ref.try_send(1).unwrap();
///     root.task.run_task().await;
/// }
/// ```
///
/// ## Pass a closure
/// ```
/// use actum::prelude::*;
///
/// #[tokio::main]
/// async fn main() {
///     let mut root = actum::<u64, _, _, _>(|mut cell, mut receiver, me| async move {
///         let m = cell.recv(&mut receiver).await.into_message().unwrap();
///         println!("{}", m);
///         (cell, ())
///     });
///     root.actor_ref.try_send(1).unwrap();
///     root.task.run_task().await;
/// }
/// ```
///
/// ## Pass arguments to the actor
/// ```
/// use actum::prelude::*;
///
/// async fn root<A>(mut cell: A, mut receiver: MessageReceiver<u64>, me: ActorRef<u64>, mut vec: Vec<u64>) -> (A, ())
/// where
///     A: Actor<u64>,
/// {
///     let m = cell.recv(&mut receiver).await.into_message().unwrap();
///     vec.push(m);
///     for m in vec {
///         println!("{}", m);
///     }
///     (cell, ())
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let vec = vec![1, 2, 3];
///     let mut root = actum(|cell, receiver, me| root(cell, receiver, me, vec));
///     root.actor_ref.try_send(4).unwrap();
///     root.task.run_task().await;
/// }
/// ```
pub fn actum<M, F, Fut, Ret>(f: F) -> ActorToSpawn<M, ActorTask<M, F, Fut, Ret, ()>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<()>, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let m_channel = mpsc::channel::<M>(100);
    let receiver = MessageReceiver::<M>::new(m_channel.1);
    let actor_ref = ActorRef::<M>::new(m_channel.0);

    let cell = ActorCell::<()>::new(());

    let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), None);

    ActorToSpawn::new(task, actor_ref)
}
