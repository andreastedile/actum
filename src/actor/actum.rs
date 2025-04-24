use crate::core::actor_cell::ActorCell;
use crate::core::actor_ref::ActorRef;
use crate::core::actor_task::ActorTask;
use crate::core::actor_to_spawn::CreateActorResult;
use crate::core::message_receiver::MessageReceiver;
use futures::channel::mpsc;

/// Instantiates the actor tree hierarchy.
///
/// The argument of this function is the root actor of the actor tree hierarchy.
/// From within the root actor, you can [create](prelude::CreateChild::create_child) new child actors.
/// Therefore, this function effectively instantiates the whole actor tree hierarchy.
///
/// Returns a struct containing the [ActorRef] of the root actor (generic over parameter `M`) and a special control structure.
/// The control structure has a method that returns a future which runs the root actor, joins its child actors (if any), and resolves with the value returned by the root actor (generic over parameter `Ret`).
/// Once this future resolves, the entire actor tree hierarchy has returned.
/// This future is **runtime agnostic**.
///
/// Note that you can send messages to the root actor before awaiting the future.
///
/// # Examples
///
/// You can pass a closure. In this way, you can also pass values to the actor.
///
/// ```rust
/// use actum::prelude::*;
///
/// #[tokio::main]
/// async fn main() {
///     let numbers = vec![1, 2, 3]; // gets moved into the closure
///     let CreateActorResult { task, mut actor_ref } = actum::<String, _, _, u64>(|cell, mut receiver, _me| async move {
///         let m = receiver.recv().await.into_message().unwrap();
///         println!("received: {}", m);
///
///         let sum = numbers.iter().sum();
///         (cell, sum)
///     });
///     actor_ref.try_send("hello from main!".to_string()).unwrap();
///     let sum = task.run_task().await;
///     println!("sum = {}", sum);
/// }
/// ```
///
/// You can pass a function pointer.
///
/// ```rust
/// use actum::prelude::*;
///
/// async fn root_actor<C, R>(cell: C, mut receiver: R, _me: ActorRef<u64>) -> (C, u64)
/// where
///     C: CreateChild,
///     R: ReceiveMessage<u64>,
/// {
///     let mut sum = 0;
///     for _ in 0..3 {
///         let m = receiver.recv().await.into_message().unwrap();
///         println!("received: {}", m);
///         sum += m;
///     }
///     (cell, sum)
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let CreateActorResult { task, mut actor_ref } = actum(root_actor);
///     actor_ref.try_send(1).unwrap();
///     actor_ref.try_send(2).unwrap();
///     actor_ref.try_send(3).unwrap();
///     let sum = task.run_task().await;
///     println!("sum = {}", sum);
/// }
/// ```
pub fn actum<M, F, Fut, Ret>(f: F) -> CreateActorResult<M, ActorTask<M, F, Fut, Ret, (), (), ()>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<()>, MessageReceiver<M, ()>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<()>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let m_channel = mpsc::channel::<M>(100);
    let actor_ref = ActorRef::new(m_channel.0);
    let receiver = MessageReceiver::new(m_channel.1, ());

    let cell = ActorCell::new(());

    let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), (), None);

    CreateActorResult::new(task, actor_ref)
}
