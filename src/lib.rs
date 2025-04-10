use crate::actor_ref::{create_actor_ref_and_message_receiver, ActorRef};
use crate::actor_task::ExtensibleActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use crate::create_child::ActorCell;
use actor_ref::ExtendableMessageReceiver;
use std::future::Future;

pub mod actor;
pub mod actor_ref;
pub mod actor_task;
pub mod actor_to_spawn;
pub mod children_tracker;
pub mod create_child;
pub mod effect;
pub mod prelude;
mod receive_message;
pub mod test_actor;
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
/// async fn root<C, R>(mut cell: C, mut receiver: R, mut me: ActorRef<u64>) -> (C, ())
/// where
///     C: CreateChild,
///     R: ReceiveMessage<u64>,
/// {
///     let m = receiver.recv().await.into_message().unwrap();
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
///         let m = receiver.recv().await.into_message().unwrap();
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
/// async fn root<C, R>(mut cell: C, mut receiver: R, me: ActorRef<u64>, mut vec: Vec<u64>) -> (C, ())
/// where
///     C: CreateChild,
///     R: ReceiveMessage<u64>,
/// {
///     let m = receiver.recv().await.into_message().unwrap();
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
pub fn actum<M, F, Fut, Ret>(f: F) -> ActorToSpawn<M, ExtensibleActorTask<M, F, Fut, Ret, (), (), ()>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<()>, ExtendableMessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let (actor_ref, receiver) = create_actor_ref_and_message_receiver::<M>();

    let cell = ActorCell {
        tracker: None,
        dependency: (),
    };

    let task = ExtensibleActorTask::new(f, cell, receiver, actor_ref.clone(), (), None);

    ActorToSpawn::new(task, actor_ref)
}
