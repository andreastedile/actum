use crate::actor::Actor;
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::standard_actor::StandardBounds;
use crate::actor_cell::{ActorCell, Stop, Stopped};
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use futures::channel::{mpsc, oneshot};
use std::future::Future;

pub mod actor;
pub mod actor_bounds;
pub mod actor_cell;
pub mod actor_ref;
pub mod drop_guard;
pub mod effect;
pub mod prelude;
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
/// async fn root<AB>(mut cell: AB, mut me: ActorRef<u64>)
/// where
///     AB: ActorBounds<u64>,
/// {
///     let m = cell.recv().await.unwrap();
///     println!("{}", m);
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
///     let mut root = actum::<u64, _, _>(|mut cell, me| async move {
///         let m = cell.recv().await.unwrap();
///         println!("{}", m);
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
/// async fn root<AB>(mut cell: AB, me: ActorRef<u64>, mut vec: Vec<u64>)
/// where
///     AB: ActorBounds<u64>,
/// {
///     let m = cell.recv().await.unwrap();
///     vec.push(m);
///     for m in vec {
///         println!("{}", m);
///     }
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
pub fn actum<M, F, Fut>(f: F) -> Actor<M, ActorTask<M, F, Fut, StandardBounds>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<M, StandardBounds>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let stop_channel = oneshot::channel::<Stop>();
    let stopped_channel = mpsc::unbounded::<Stopped>();
    let m_channel = mpsc::channel::<M>(100);

    let guard = ActorDropGuard::new(stop_channel.0);
    let bounds = StandardBounds;
    let cell = ActorCell::new(stop_channel.1, stopped_channel.0, m_channel.1, bounds);

    let m_ref = ActorRef::new(m_channel.0);
    let task = ActorTask::new(f, cell, m_ref.clone(), stopped_channel.1, None);
    Actor::new(task, guard, m_ref)
}
