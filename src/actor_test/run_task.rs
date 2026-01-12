use crate::actor_test::create_child::ActorCell;
use crate::actor_test::effect::returned_effect::{ReturnedEffectToActor, ReturnedEffectToTestkit};
use crate::actor_test::receive_message::MessageReceiver;
use crate::core::children_tracker::WakeParentOnDrop;
use crate::prelude::{ActorRef, RunTask};
use either::Either;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

pub struct ActorTask<M, F, Fut, Output> {
    f: Either<F, BoxTestActor<M, Output>>,
    ret: PhantomData<Output>,
    fut: PhantomData<Fut>,
    receiver: MessageReceiver<M>,
    cell: ActorCell,
    actor_ref: ActorRef<M>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
    returned_effect_to_testkit_sender: oneshot::Sender<ReturnedEffectToTestkit<Output>>,
    returned_effect_to_actor_receiver: oneshot::Receiver<ReturnedEffectToActor<Output>>,
}

impl<M, F, Fut, Output> ActorTask<M, F, Fut, Output> {
    pub(crate) const fn new(
        f: Either<F, BoxTestActor<M, Output>>,
        cell: ActorCell,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
        returned_effect_to_testkit_sender: oneshot::Sender<ReturnedEffectToTestkit<Output>>,
        returned_effect_to_actor_receiver: oneshot::Receiver<ReturnedEffectToActor<Output>>,
    ) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            receiver,
            cell,
            actor_ref,
            _waker: waker,
            returned_effect_to_testkit_sender,
            returned_effect_to_actor_receiver,
        }
    }
}

impl<M, F, Fut, Output> RunTask<Output> for ActorTask<M, F, Fut, Output>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Output)> + Send + 'static,
    Output: Send + 'static,
{
    async fn run_task(self) -> Output {
        let f = self.f;
        let (mut cell, output) = match f {
            Either::Left(f) => {
                let fut = f(self.cell, self.receiver, self.actor_ref);
                fut.await
            }
            Either::Right(f) => {
                let fut = f(self.cell, self.receiver, self.actor_ref);
                fut.await
            }
        };

        if cell.tracker.has_children() {
            tracing::trace!("joining children");
            cell.tracker.join_all().await;
        }

        let returned_effect_to_testkit = ReturnedEffectToTestkit { output };
        self.returned_effect_to_testkit_sender
            .send(returned_effect_to_testkit)
            .expect("could not send the effect to the testkit");

        let returned_effect_to_actor = self
            .returned_effect_to_actor_receiver
            .await
            .expect("could not receive effect back from the testkit");

        returned_effect_to_actor.output
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Output> =
    Box<dyn FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> BoxFuture<'static, (ActorCell, Output)> + Send + 'static>;

pub(crate) struct UntypedBoxTestActor(Box<dyn Any + Send>);

impl Debug for UntypedBoxTestActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("UntypedBoxTestActor")
    }
}

impl<M, Output> From<BoxTestActor<M, Output>> for UntypedBoxTestActor
where
    M: 'static,
    Output: 'static,
{
    fn from(actor: BoxTestActor<M, Output>) -> Self {
        Self(Box::new(actor))
    }
}

impl UntypedBoxTestActor {
    pub fn downcast_unwrap<M: 'static, Output: 'static>(self) -> BoxTestActor<M, Output> {
        self.0.downcast::<BoxTestActor<M, Output>>().unwrap()
    }
}
