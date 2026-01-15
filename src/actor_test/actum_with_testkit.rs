use crate::actor_test::create_child::ActorCell;
use crate::actor_test::effect::completed_effect::{CompletedEffectToActor, CompletedEffectToTestkit};
use crate::actor_test::effect::create_child_effect::{CreateChildEffectToActor, UntypedCreateChildEffectToTestkit};
use crate::actor_test::effect::recv_effect::{RecvEffectToActor, RecvEffectToTestkit};
use crate::actor_test::receive_message::MessageReceiver;
use crate::actor_test::scoped::ScopedActorTask;
use crate::prelude::{ActorRef, Testkit};
use either::Either;
use futures::channel::{mpsc, oneshot};

pub fn actum_with_testkit<M, F, Fut, Output>(f: F) -> ActumWithTestkit<M, ScopedActorTask<M, F, Fut, Output>, Output>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Output)> + Send + 'static,
    Output: Send + 'static,
{
    let recv_effect_to_testkit_channel = mpsc::channel::<RecvEffectToTestkit<M>>(1);
    let recv_effect_to_actor_channel = mpsc::channel::<RecvEffectToActor<M>>(1);
    let create_child_effect_to_testkit_channel = mpsc::channel::<UntypedCreateChildEffectToTestkit>(1);
    let create_child_effect_to_actor_channel = mpsc::channel::<CreateChildEffectToActor>(1);
    let completed_effect_to_testkit_channel = oneshot::channel::<CompletedEffectToTestkit<Output>>();
    let completed_effect_to_actor_channel = oneshot::channel::<CompletedEffectToActor<Output>>();

    let m_channel = mpsc::channel::<M>(100);
    let actor_ref = ActorRef::new(m_channel.0);
    let receiver = MessageReceiver::new(
        m_channel.1,
        recv_effect_to_testkit_channel.0,
        recv_effect_to_actor_channel.1,
    );

    let cell = ActorCell::new(
        create_child_effect_to_testkit_channel.0,
        create_child_effect_to_actor_channel.1,
    );

    let testkit = Testkit::new(
        recv_effect_to_testkit_channel.1,
        recv_effect_to_actor_channel.0,
        create_child_effect_to_testkit_channel.1,
        create_child_effect_to_actor_channel.0,
        completed_effect_to_testkit_channel.1,
        completed_effect_to_actor_channel.0,
    );

    let task = ScopedActorTask::new(
        Either::Left(f),
        cell,
        receiver,
        actor_ref.clone(),
        None,
        completed_effect_to_testkit_channel.0,
        completed_effect_to_actor_channel.1,
    );

    ActumWithTestkit {
        task,
        actor_ref,
        testkit,
    }
}

/// Returned by [actum_with_testkit].
pub struct ActumWithTestkit<M, Scoped, Output> {
    pub task: Scoped,
    pub actor_ref: ActorRef<M>,
    pub testkit: Testkit<M, Output>,
}
