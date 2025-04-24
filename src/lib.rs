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

/// Creates the root actor of the actor tree hierarchy.
/// The actor should then be spawned onto the runtime of choice.
///
/// Compared to using the [testkit](actum_with_testkit) function, this function does not incur any overhead.
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

pub struct ActumWithTestkit<M, RT, Ret> {
    pub task: RT,
    pub actor_ref: ActorRef<M>,
    pub testkit: Testkit<M, Ret>,
}
