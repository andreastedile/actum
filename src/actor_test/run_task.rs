use crate::actor_test::create_child::ActorCell;
use crate::actor_test::effect::returned_effect::{ReturnedEffectFromActorToTestkit, ReturnedEffectFromTestkitToActor};
use crate::actor_test::receive_message::MessageReceiver;
use crate::core::children_tracker::WakeParentOnDrop;
use crate::prelude::{ActorRef, RunTask};
use futures::channel::oneshot;
use futures::future::BoxFuture;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

pub struct ActorTask<M, F, Fut, Ret> {
    f: F,
    ret: PhantomData<Ret>,
    fut: PhantomData<Fut>,
    receiver: MessageReceiver<M>,
    cell: ActorCell,
    actor_ref: ActorRef<M>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
    returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
    returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,
}

impl<M, F, Fut, Ret> ActorTask<M, F, Fut, Ret> {
    pub(crate) const fn new(
        f: F,
        cell: ActorCell,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
        returned_effect_from_actor_to_testkit_sender: oneshot::Sender<ReturnedEffectFromActorToTestkit<Ret>>,
        returned_effect_from_testkit_to_actor_receiver: oneshot::Receiver<ReturnedEffectFromTestkitToActor<Ret>>,
    ) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            receiver,
            cell,
            actor_ref,
            _waker: waker,
            returned_effect_from_actor_to_testkit_sender,
            returned_effect_from_testkit_to_actor_receiver,
        }
    }
}

impl<M, F, Fut, Ret> RunTask<Ret> for ActorTask<M, ActorInner<F, M, Ret>, Fut, Ret>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Ret)> + Send + 'static,
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

        if cell.tracker.has_children() {
            tracing::trace!("joining children");
            cell.tracker.join_all().await;
        }

        let returned_effect_from_actor_to_testkit = ReturnedEffectFromActorToTestkit { ret };
        self.returned_effect_from_actor_to_testkit_sender
            .send(returned_effect_from_actor_to_testkit)
            .expect("could not send the effect to the testkit");

        let returned_effect_from_testkit_to_actor = self
            .returned_effect_from_testkit_to_actor_receiver
            .await
            .expect("could not receive effect back from the testkit");

        returned_effect_from_testkit_to_actor.ret
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Ret> =
    Box<dyn FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> BoxFuture<'static, (ActorCell, Ret)> + Send + 'static>;

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

pub enum ActorInner<F, M, Ret> {
    Unboxed(F),
    Boxed(BoxTestActor<M, Ret>),
}
