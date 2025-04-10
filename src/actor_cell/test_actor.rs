use crate::actor::Actor;
use crate::actor_cell::ActorCell;
use crate::actor_ref::{ActorRef, MessageReceiverWithTestkitExtension};
use crate::actor_task::{ActorInner, ActorTask};
use crate::actor_to_spawn::ActorToSpawn;
use crate::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::effect::spawn_effect::{SpawnEffectFromTestkitToActor, UntypedSpawnEffectFromActorToTestkit};
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use std::future::Future;
use std::marker::PhantomData;

pub struct TestExtension<Ret> {
    /// used to send spawn effects from the actor under test to the corresponding testkit.
    spawn_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedSpawnEffectFromActorToTestkit>,
    /// used to receive spawn effects from the testkit to the actor.
    spawn_effect_from_testkit_to_actor_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,

    /// used to send returned effects from the actor under test to the corresponding testkit.
    pub(crate) returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
    /// used to receive returned effects from the testkit to the actor.
    pub(crate) returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,

    _ret: PhantomData<Ret>,
}

impl<Ret> TestExtension<Ret> {
    pub(crate) const fn new(
        spawn_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedSpawnEffectFromActorToTestkit>,
        spawn_effect_from_testkit_to_actor_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,
        returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            spawn_effect_from_actor_to_testkit_sender,
            spawn_effect_from_testkit_to_actor_receiver,
            returned_effect_from_actor_to_testkit_sender,
            returned_effect_from_testkit_to_actor_receiver,
            _ret: PhantomData,
        }
    }
}

impl<M, Ret> Actor<M, Ret> for ActorCell<TestExtension<Ret>, Ret>
where
    M: Send + 'static,
    Ret: Send + 'static,
{
    type ChildActor<M2: Send + 'static, Ret2: Send + 'static> = ActorCell<TestExtension<Ret2>, Ret2>;
    type HasRunTask<M2, F, Fut, Ret2>
        =
        ActorTask<M2, ActorInner<F, M2, Ret2>, Fut, Ret2, TestExtension<Ret2>, MessageReceiverWithTestkitExtension<M2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<TestExtension<Ret2>, Ret2>, MessageReceiverWithTestkitExtension<M2>, ActorRef<M2>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = (ActorCell<TestExtension<Ret2>, Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    type MessageReceiverT<M2>
        = MessageReceiverWithTestkitExtension<M2>
    where
        M2: Send + 'static;

    async fn create_child<M2, F, Fut, Ret2>(
        &mut self,
        f: F,
    ) -> ActorToSpawn<
        M2,
        ActorTask<M2, ActorInner<F, M2, Ret2>, Fut, Ret2, TestExtension<Ret2>, MessageReceiverWithTestkitExtension<M2>>,
    >
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<TestExtension<Ret2>, Ret2>, MessageReceiverWithTestkitExtension<M2>, ActorRef<M2>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = (ActorCell<TestExtension<Ret2>, Ret2>, Ret2)> + Send + 'static,
        Ret2: Send + 'static,
    {
        assert!(!self.dependency.spawn_effect_from_actor_to_testkit_sender.is_closed());

        let m_channel = mpsc::channel::<M2>(100);
        let actor_ref = ActorRef::<M2>::new(m_channel.0);

        let recv_effect_from_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M2>>(1);
        let recv_effect_from_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M2>>(1);
        let spawn_effect_from_actor_to_testkit_channel = mpsc::channel::<UntypedSpawnEffectFromActorToTestkit>(1);
        let spawn_effect_from_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor>(1);
        let returned_effect_from_actor_to_testkit_channel =
            oneshot::channel::<ReturnedEffectFromActorToTestkit<Ret2>>();
        let returned_effect_from_testkit_to_actor_channel =
            oneshot::channel::<ReturnedEffectFromTestkitToActor<Ret2>>();

        let receiver = MessageReceiverWithTestkitExtension::<M2>::new(
            m_channel.1,
            recv_effect_from_actor_to_testkit_channel.0,
            recv_effect_from_testkit_to_actor_channel.1,
        );

        let extension = TestExtension::new(
            spawn_effect_from_actor_to_testkit_channel.0,
            spawn_effect_from_testkit_to_actor_channel.1,
            returned_effect_from_actor_to_testkit_channel.0,
            returned_effect_from_testkit_to_actor_channel.1,
        );

        let testkit = Testkit::new(
            recv_effect_from_actor_to_testkit_channel.1,
            recv_effect_from_testkit_to_actor_channel.0,
            spawn_effect_from_actor_to_testkit_channel.1,
            spawn_effect_from_testkit_to_actor_channel.0,
            returned_effect_from_actor_to_testkit_channel.1,
            returned_effect_from_testkit_to_actor_channel.0,
        );

        let cell = ActorCell::new(extension);

        let tracker = self.tracker.get_or_insert_default();
        let mut task = ActorTask::new(
            ActorInner::Unboxed(f),
            cell,
            receiver,
            actor_ref.clone(),
            Some(tracker.make_child()),
        );

        let spawn_effect_to_testkit = UntypedSpawnEffectFromActorToTestkit {
            untyped_testkit: testkit.into(),
        };

        self.dependency
            .spawn_effect_from_actor_to_testkit_sender
            .try_send(spawn_effect_to_testkit)
            .expect("could not send the effect to the testkit");

        let spawn_effect_from_actor = self
            .dependency
            .spawn_effect_from_testkit_to_actor_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        if let Some(inner) = spawn_effect_from_actor.injected {
            let actor = inner.downcast_unwrap::<M2, Ret2>();
            task.f = ActorInner::Boxed(actor);
        };

        ActorToSpawn::new(task, actor_ref)
    }
}
