use crate::actor_ref::{ActorRef, MessageReceiverTestkitExtension, MessageReceiverWithTestkitExtension};
use crate::actor_task::{ExtensibleActorTask, RunTask};
use crate::actor_to_spawn::ActorToSpawn;
use crate::create_child::{ActorCell, CreateChild};
use crate::effect::recv_effect::{RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor};
use crate::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::effect::spawn_effect::{SpawnEffectFromTestkitToActor, UntypedSpawnEffectFromActorToTestkit};
use crate::testkit::Testkit;
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::StreamExt;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::marker::PhantomData;

pub struct ActorCellTestkitExtension {
    /// used to send spawn effects from the actor under test to the corresponding testkit.
    pub(crate) spawn_effect_from_actor_to_testkit_sender: mpsc::Sender<UntypedSpawnEffectFromActorToTestkit>,
    /// used to receive spawn effects from the testkit to the actor.
    pub(crate) spawn_effect_from_testkit_to_actor_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,
}

impl CreateChild for ActorCell<ActorCellTestkitExtension> {
    type MessageReceiverT<M2>
        = MessageReceiverWithTestkitExtension<M2>
    where
        M2: Send + 'static;
    type HasRunTask<M2, F, Fut, Ret2>
        = ExtensibleActorTask<
        M2,
        ActorInner<F, M2, Ret2>,
        Fut,
        Ret2,
        ActorCellTestkitExtension,
        MessageReceiverTestkitExtension<M2>,
        ActorTaskTestkitExtension<Ret2>,
    >
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiverWithTestkitExtension<M2>, ActorRef<M2>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = (ActorCell<ActorCellTestkitExtension>, Ret2)> + Send + 'static,
        Ret2: Send + 'static;

    async fn create_child<M2, F, Fut, Ret2>(&mut self, f: F) -> ActorToSpawn<M2, Self::HasRunTask<M2, F, Fut, Ret2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiverWithTestkitExtension<M2>, ActorRef<M2>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = (ActorCell<ActorCellTestkitExtension>, Ret2)> + Send + 'static,
        Ret2: Send + 'static,
    {
        assert!(!self.dependency.spawn_effect_from_actor_to_testkit_sender.is_closed());

        let recv_effect_from_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M2>>(1);
        let recv_effect_from_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M2>>(1);
        let spawn_effect_from_actor_to_testkit_channel = mpsc::channel::<UntypedSpawnEffectFromActorToTestkit>(1);
        let spawn_effect_from_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor>(1);
        let returned_effect_from_actor_to_testkit_channel =
            oneshot::channel::<ReturnedEffectFromActorToTestkit<Ret2>>();
        let returned_effect_from_testkit_to_actor_channel =
            oneshot::channel::<ReturnedEffectFromTestkitToActor<Ret2>>();

        let cell = ActorCell {
            tracker: None,
            dependency: ActorCellTestkitExtension {
                spawn_effect_from_actor_to_testkit_sender: spawn_effect_from_actor_to_testkit_channel.0,
                spawn_effect_from_testkit_to_actor_receiver: spawn_effect_from_testkit_to_actor_channel.1,
            },
        };

        let m_channel = mpsc::channel::<M2>(100);
        let actor_ref = ActorRef::<M2>::new(m_channel.0);

        let receiver = MessageReceiverWithTestkitExtension::<M2>::new(
            m_channel.1,
            recv_effect_from_actor_to_testkit_channel.0,
            recv_effect_from_testkit_to_actor_channel.1,
        );

        let extension = ActorTaskTestkitExtension::new(
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

        let tracker = self.tracker.get_or_insert_default();
        let mut task = ExtensibleActorTask::new(
            ActorInner::Unboxed(f),
            cell,
            receiver,
            actor_ref.clone(),
            extension,
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

pub enum ActorInner<F, M, Ret> {
    Unboxed(F),
    Boxed(BoxTestActor<M, Ret>),
}

impl<M, F, Fut, Ret> RunTask<Ret>
    for ExtensibleActorTask<
        M,
        ActorInner<F, M, Ret>,
        Fut,
        Ret,
        ActorCellTestkitExtension,
        MessageReceiverTestkitExtension<M>,
        ActorTaskTestkitExtension<Ret>,
    >
where
    M: Send + 'static,
    F: FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiverWithTestkitExtension<M>, ActorRef<M>) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = (ActorCell<ActorCellTestkitExtension>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let (mut cell, ret) = match f {
            ActorInner::Unboxed(f) => {
                //
                let fut = f(self.cell, self.receiver, self.actor_ref);
                fut.await
            }
            ActorInner::Boxed(f) => {
                //
                let fut = f(self.cell, self.receiver, self.actor_ref);
                fut.await
            }
        };

        if let Some(tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        let effect_out = ReturnedEffectFromActorToTestkit { ret };
        self.dependency
            .returned_effect_from_actor_to_testkit_sender
            .send(effect_out)
            .expect("could not send the effect to the testkit");

        let effect_in = self
            .dependency
            .returned_effect_from_testkit_to_actor_receiver
            .await
            .expect("could not receive effect back from the testkit");

        effect_in.ret
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Ret> =
    Box<dyn FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiverWithTestkitExtension<M>, ActorRef<M>) -> BoxFuture<'static, (ActorCell<ActorCellTestkitExtension>, Ret)> + Send + 'static>;

pub(crate) struct UntypedBoxTestActor(Box<dyn Any + Send>);

impl Debug for UntypedBoxTestActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("UntypedBoxTestActor")
    }
}

impl<M, Ret> From<BoxTestActor<M, Ret>> for UntypedBoxTestActor
where
    M: 'static,
    Ret: 'static,
{
    fn from(actor: BoxTestActor<M, Ret>) -> Self {
        Self(Box::new(actor))
    }
}

impl UntypedBoxTestActor {
    pub fn downcast_unwrap<M: 'static, Ret: 'static>(self) -> BoxTestActor<M, Ret> {
        self.0.downcast::<BoxTestActor<M, Ret>>().unwrap()
    }
}

pub struct ActorTaskTestkitExtension<Ret> {
    /// used to send returned effects from the actor under test to the corresponding testkit.
    pub(crate) returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
    /// used to receive returned effects from the testkit to the actor.
    pub(crate) returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,

    _ret: PhantomData<Ret>,
}

impl<Ret> ActorTaskTestkitExtension<Ret> {
    pub(crate) const fn new(
        returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            returned_effect_from_actor_to_testkit_sender,
            returned_effect_from_testkit_to_actor_receiver,
            _ret: PhantomData,
        }
    }
}
