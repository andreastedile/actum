use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use crate::create_child::ActorCell;
use crate::effect::create_child_effect::{
    CreateChildEffectFromTestkitToActor, UntypedCreateChildEffectFromActorToTestkit,
};
use crate::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::receive_message::MessageReceiver;
use crate::test_actor::{
    ActorCellTestkitExtension, ActorInner, ActorTaskTestkitExtension, MessageReceiverTestkitExtension,
};
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
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

/// Instantiates the actor tree hierarchy.
///
/// The argument of this function is the root actor of the actor tree hierarchy.
/// From within the root actor, you can [create](create_child::CreateChild::create_child) new child actors.
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
///     let ActorToSpawn { task, mut actor_ref } = actum::<String, _, _, u64>(|cell, mut receiver, _me| async move {
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
///     let ActorToSpawn { task, mut actor_ref } = actum(root_actor);
///     actor_ref.try_send(1).unwrap();
///     actor_ref.try_send(2).unwrap();
///     actor_ref.try_send(3).unwrap();
///     let sum = task.run_task().await;
///     println!("sum = {}", sum);
/// }
/// ```
pub fn actum<M, F, Fut, Ret>(f: F) -> ActorToSpawn<M, ActorTask<M, F, Fut, Ret, (), (), ()>>
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

    ActorToSpawn::new(task, actor_ref)
}

/// Instantiates the actor tree hierarchy with instrumentation for testing the behavior of the actors in the tree hierarchy.
///
/// The documentation of [actum] applies to this function as well, with the difference that the
/// returned struct also contains the [Testkit] for testing the behavior of the root actor.
///
/// For any child actor created from within the root actor, you can obtain its corresponding testkit
/// through the testkit of the root actor (see [expect_create_child_effect](Testkit::expect_create_child_effect)).
///
/// # Examples
///
/// See the documentation of [Testkit] and its methods.
pub fn actum_with_testkit<M, F, Fut, Ret>(
    f: F,
) -> ActumWithTestkit<
    M,
    ActorTask<
        M,
        ActorInner<F, M, Ret>,
        Fut,
        Ret,
        ActorCellTestkitExtension,
        MessageReceiverTestkitExtension<M>,
        ActorTaskTestkitExtension<Ret>,
    >,
    Ret,
>
where
    M: Send + 'static,
    F: FnOnce(
            ActorCell<ActorCellTestkitExtension>,
            MessageReceiver<M, MessageReceiverTestkitExtension<M>>,
            ActorRef<M>,
        ) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = (ActorCell<ActorCellTestkitExtension>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    let recv_effect_from_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M>>(1);
    let recv_effect_from_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M>>(1);
    let create_child_effect_from_actor_to_testkit_channel =
        mpsc::channel::<UntypedCreateChildEffectFromActorToTestkit>(1);
    let create_child_effect_from_testkit_to_actor_channel = mpsc::channel::<CreateChildEffectFromTestkitToActor>(1);
    let returned_effect_from_actor_to_testkit_channel = oneshot::channel::<ReturnedEffectFromActorToTestkit<Ret>>();
    let returned_effect_from_testkit_to_actor_channel = oneshot::channel::<ReturnedEffectFromTestkitToActor<Ret>>();

    let m_channel = mpsc::channel::<M>(100);
    let actor_ref = ActorRef::new(m_channel.0);
    let receiver = MessageReceiver::new(
        m_channel.1,
        MessageReceiverTestkitExtension::new(
            recv_effect_from_actor_to_testkit_channel.0,
            recv_effect_from_testkit_to_actor_channel.1,
        ),
    );

    let cell = ActorCell::new(ActorCellTestkitExtension::new(
        create_child_effect_from_actor_to_testkit_channel.0,
        create_child_effect_from_testkit_to_actor_channel.1,
    ));

    let extension = ActorTaskTestkitExtension::new(
        returned_effect_from_actor_to_testkit_channel.0,
        returned_effect_from_testkit_to_actor_channel.1,
    );

    let testkit = Testkit::new(
        recv_effect_from_actor_to_testkit_channel.1,
        recv_effect_from_testkit_to_actor_channel.0,
        create_child_effect_from_actor_to_testkit_channel.1,
        create_child_effect_from_testkit_to_actor_channel.0,
        returned_effect_from_actor_to_testkit_channel.1,
        returned_effect_from_testkit_to_actor_channel.0,
    );

    let task = ActorTask::new(
        ActorInner::Unboxed(f),
        cell,
        receiver,
        actor_ref.clone(),
        extension,
        None,
    );

    ActumWithTestkit {
        task,
        actor_ref,
        testkit,
    }
}

/// Returned by [actum_with_testkit].
pub struct ActumWithTestkit<M, RT, Ret> {
    pub task: RT,
    pub actor_ref: ActorRef<M>,
    pub testkit: Testkit<M, Ret>,
}
