use crate::actor_test::create_child::ActorCellTestkitExtension;
use crate::actor_test::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::actor_test::receive_message::MessageReceiverTestkitExtension;
use crate::core::actor_cell::ActorCell;
use crate::core::actor_task::ActorTask;
use crate::core::message_receiver::MessageReceiver;
use crate::prelude::{ActorRef, RunTask};
use futures::channel::oneshot;
use futures::future::BoxFuture;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

impl<M, F, Fut, Ret> RunTask<Ret>
    for ActorTask<
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

        if let Some(mut tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        let returned_effect_from_actor_to_testkit = ReturnedEffectFromActorToTestkit { ret };
        self.dependency
            .returned_effect_from_actor_to_testkit_sender
            .send(returned_effect_from_actor_to_testkit)
            .expect("could not send the effect to the testkit");

        let returned_effect_from_testkit_to_actor = self
            .dependency
            .returned_effect_from_testkit_to_actor_receiver
            .await
            .expect("could not receive effect back from the testkit");

        returned_effect_from_testkit_to_actor.ret
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Ret> =
    Box<dyn FnOnce(ActorCell<ActorCellTestkitExtension>, MessageReceiver<M, MessageReceiverTestkitExtension<M>>, ActorRef<M>) -> BoxFuture<'static, (ActorCell<ActorCellTestkitExtension>, Ret)> + Send + 'static>;

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
    returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
    /// used to receive returned effects from the testkit to the actor.
    returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,

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

pub enum ActorInner<F, M, Ret> {
    Unboxed(F),
    Boxed(BoxTestActor<M, Ret>),
}
