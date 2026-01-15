use crate::actor_test::effect::completed_effect::{CompletedEffectToActor, CompletedEffectToTestkit};
use crate::actor_test::effect::create_child_effect::{CreateChildEffectToActor, UntypedCreateChildEffectToTestkit};
use crate::actor_test::effect::recv_effect::{RecvEffectToActor, RecvEffectToTestkit};
use crate::actor_test::receive_message::MessageReceiver;
use crate::actor_test::scoped::ScopedActorTask;
use crate::core::children_tracker::ChildrenTracker;
use crate::prelude::{ActorRef, CreateActorResult, CreateChild, Testkit};
use either::Either;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};

pub struct ActorCell {
    pub(crate) tracker: ChildrenTracker,
    create_child_effect_sender: mpsc::Sender<UntypedCreateChildEffectToTestkit>,
    create_child_effect_receiver: mpsc::Receiver<CreateChildEffectToActor>,
}

impl ActorCell {
    pub(crate) fn new(
        create_child_effect_sender: mpsc::Sender<UntypedCreateChildEffectToTestkit>,
        create_child_effect_receiver: mpsc::Receiver<CreateChildEffectToActor>,
    ) -> Self {
        Self {
            tracker: ChildrenTracker::new(),
            create_child_effect_sender,
            create_child_effect_receiver,
        }
    }
}

impl CreateChild for ActorCell {
    type ReceiveMessageT<M>
        = MessageReceiver<M>
    where
        M: Send + 'static;

    type ScopedActorTaskT<M, F, Fut, Output>
        = ScopedActorTask<M, F, Fut, Output>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Output)> + Send + 'static,
        Output: Send + 'static;

    async fn create_child<M, F, Fut, Output>(
        &mut self,
        f: F,
    ) -> CreateActorResult<M, Self::ScopedActorTaskT<M, F, Fut, Output>>
    where
        M: Send + 'static,
        F: FnOnce(Self, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Output)> + Send + 'static,
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

        let cell = Self::new(
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

        let create_child_to_testkit = UntypedCreateChildEffectToTestkit {
            untyped_testkit: testkit.into(),
        };

        self.create_child_effect_sender
            .try_send(create_child_to_testkit)
            .expect("could not send the effect to the testkit");

        let create_child_effect_to_actor = self
            .create_child_effect_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        let either = if let Some(injected) = create_child_effect_to_actor.injected {
            Either::Right(injected.downcast_unwrap::<M, Output>())
        } else {
            Either::Left(f)
        };

        let task = ScopedActorTask::new(
            either,
            cell,
            receiver,
            actor_ref.clone(),
            Some(self.tracker.make_child()),
            completed_effect_to_testkit_channel.0,
            completed_effect_to_actor_channel.1,
        );

        CreateActorResult::new(task, actor_ref)
    }
}
