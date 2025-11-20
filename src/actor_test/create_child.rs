use crate::actor_test::effect::create_child_effect::{
    CreateChildEffectFromTestkitToActor, UntypedCreateChildEffectFromActorToTestkit,
};
use crate::actor_test::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::actor_test::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::actor_test::receive_message::MessageReceiver;
use crate::actor_test::run_task::ActorTask;
use crate::core::children_tracker::ChildrenTracker;
use crate::prelude::{ActorRef, CreateActorResult, CreateChild, Testkit};
use either::Either;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};

pub struct ActorCell {
    pub(crate) tracker: ChildrenTracker,
    create_child_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedCreateChildEffectFromActorToTestkit>,
    create_child_effect_from_testkit_to_actor_receiver: mpsc::Receiver<CreateChildEffectFromTestkitToActor>,
}

impl ActorCell {
    pub(crate) fn new(
        create_child_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedCreateChildEffectFromActorToTestkit>,
        create_child_effect_from_testkit_to_actor_receiver: mpsc::Receiver<CreateChildEffectFromTestkitToActor>,
    ) -> Self {
        Self {
            tracker: ChildrenTracker::new(),
            create_child_effect_from_actor_to_testkit_sender,
            create_child_effect_from_testkit_to_actor_receiver,
        }
    }
}

impl CreateChild for ActorCell {
    type ReceiveMessageT<M>
        = MessageReceiver<M>
    where
        M: Send + 'static;

    type RunTaskT<M, F, Fut, Ret>
        = ActorTask<M, F, Fut, Ret>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;

    async fn create_child<M, F, Fut, Ret>(&mut self, f: F) -> CreateActorResult<M, Self::RunTaskT<M, F, Fut, Ret>>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
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
            recv_effect_from_actor_to_testkit_channel.0,
            recv_effect_from_testkit_to_actor_channel.1,
        );

        let cell = Self::new(
            create_child_effect_from_actor_to_testkit_channel.0,
            create_child_effect_from_testkit_to_actor_channel.1,
        );

        let testkit = Testkit::new(
            recv_effect_from_actor_to_testkit_channel.1,
            recv_effect_from_testkit_to_actor_channel.0,
            create_child_effect_from_actor_to_testkit_channel.1,
            create_child_effect_from_testkit_to_actor_channel.0,
            returned_effect_from_actor_to_testkit_channel.1,
            returned_effect_from_testkit_to_actor_channel.0,
        );

        let create_child_effect_from_actor_to_testkit = UntypedCreateChildEffectFromActorToTestkit {
            untyped_testkit: testkit.into(),
        };

        self.create_child_effect_from_actor_to_testkit_sender
            .try_send(create_child_effect_from_actor_to_testkit)
            .expect("could not send the effect to the testkit");

        let create_child_effect_from_testkit_to_actor = self
            .create_child_effect_from_testkit_to_actor_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        let inner = if let Some(injected) = create_child_effect_from_testkit_to_actor.injected {
            Either::Right(injected.downcast_unwrap::<M, Ret>())
        } else {
            Either::Left(f)
        };

        let task = ActorTask::new(
            inner,
            cell,
            receiver,
            actor_ref.clone(),
            Some(self.tracker.make_child()),
            returned_effect_from_actor_to_testkit_channel.0,
            returned_effect_from_testkit_to_actor_channel.1,
        );

        CreateActorResult::new(task, actor_ref)
    }
}
