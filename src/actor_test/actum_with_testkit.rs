use crate::actor_test::create_child::ActorCellTestkitExtension;
use crate::actor_test::effect::create_child_effect::{
    CreateChildEffectFromTestkitToActor, UntypedCreateChildEffectFromActorToTestkit,
};
use crate::actor_test::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::actor_test::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::actor_test::receive_message::MessageReceiverTestkitExtension;
use crate::actor_test::run_task::ActorInner;
use crate::actor_test::run_task::ActorTaskTestkitExtension;
use crate::core::actor_cell::ActorCell;
use crate::core::actor_task::ActorTask;
use crate::core::message_receiver::MessageReceiver;
use crate::prelude::{ActorRef, Testkit};
use futures::channel::{mpsc, oneshot};

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
