use crate::actor_cell::{ActorCell, Stop};
use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use crate::drop_guard::ActorDropGuard;
use futures::channel::{mpsc, oneshot};
use std::future::Future;

pub mod actor;
pub mod actor_cell;
pub mod actor_ref;
pub mod actor_task;
pub mod actor_to_spawn;
pub mod drop_guard;
pub mod effect;
pub mod prelude;
mod resolve_when_one;
pub mod testkit;

/// Define the root actor of the actor tree hierarchy.
///
/// Compared to using the [testkit](testkit::testkit) function, this function does not incur in any overhead.
///
/// # Examples
///
/// ## Pass a function pointer
/// ```
/// use actum::prelude::*;
///
/// async fn root<A>(mut cell: A, mut me: ActorRef<u64>) -> (A, ())
/// where
///     A: Actor<u64>,
/// {
///     let m = cell.recv().await.message().unwrap();
///     println!("{}", m);
///     (cell, ())
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut root = actum(root);
///     root.m_ref.try_send(1).unwrap();
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
///     let mut root = actum::<u64, _, _, _>(|mut cell, me| async move {
///         let m = cell.recv().await.message().unwrap();
///         println!("{}", m);
///         (cell, ())
///     });
///     root.m_ref.try_send(1).unwrap();
///     root.task.run_task().await;
/// }
/// ```
///
/// ## Pass arguments to the actor
/// ```
/// use actum::prelude::*;
///
/// async fn root<A>(mut cell: A, me: ActorRef<u64>, mut vec: Vec<u64>) -> (A, ())
/// where
///     A: Actor<u64>,
/// {
///     let m = cell.recv().await.message().unwrap();
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
///     let mut root = actum(|cell, me| root(cell, me, vec));
///     root.m_ref.try_send(4).unwrap();
///     root.task.run_task().await;
/// }
/// ```
pub fn actum<M, F, Fut, Ret>(f: F) -> ActorToSpawn<M, ActorTask<M, F, Fut, Ret, ()>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<M, ()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let stop_channel = oneshot::channel::<Stop>();
    let m_channel = mpsc::channel::<M>(100);

    let guard = ActorDropGuard::new(stop_channel.0);
    let cell = ActorCell::new(stop_channel.1, m_channel.1, ());

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), None);
    ActorToSpawn::new(task, guard, m_ref)
}
