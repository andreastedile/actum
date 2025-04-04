use crate::actor_cell::test_actor::TestExtension;
use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::actor_ref::MessageReceiver;
use crate::children_tracker::WakeParentOnDrop;
use crate::effect::returned_effect::ReturnedEffectFromActorToTestkit;
use futures::future::BoxFuture;
use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;

pub trait RunTask<Ret>: Send + 'static {
    fn run_task(self) -> impl Future<Output = Ret> + Send + 'static;
}

pub struct ActorTask<M, F, Fut, Ret, D> {
    pub(crate) f: F,
    ret: PhantomData<Ret>,
    fut: PhantomData<Fut>,
    pub(crate) cell: ActorCell<D, Ret>,
    pub(crate) receiver: MessageReceiver<M>,
    actor_ref: ActorRef<M>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
}

impl<M, F, Fut, Ret, D> ActorTask<M, F, Fut, Ret, D> {
    pub const fn new(
        f: F,
        cell: ActorCell<D, Ret>,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
    ) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            cell,
            receiver,
            actor_ref,
            _waker: waker,
        }
    }
}

impl<M, F, Fut, Ret> RunTask<Ret> for ActorTask<M, F, Fut, Ret, ()>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<(), Ret>, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<(), Ret>, Ret)> + Send + 'static,
    Ret: Send + 'static,
{
    async fn run_task(self) -> Ret {
        let f = self.f;
        let fut = f(self.cell, self.receiver, self.actor_ref);
        let (mut cell, ret) = fut.await;

        if let Some(tracker) = cell.tracker.take() {
            tracing::trace!("joining children");
            tracker.join_all().await;
        }

        ret
    }
}

pub enum ActorInner<F, M, Ret> {
    Unboxed(F),
    Boxed(BoxTestActor<M, Ret>),
}

impl<M, F, Fut, Ret> RunTask<Ret> for ActorTask<M, ActorInner<F, M, Ret>, Fut, Ret, TestExtension<M, Ret>>
where
    M: Send + 'static,
    F: FnOnce(ActorCell<TestExtension<M, Ret>, Ret>, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell<TestExtension<M, Ret>, Ret>, Ret)> + Send + 'static,
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

        let extension = cell.dependency;

        let effect_out = ReturnedEffectFromActorToTestkit { ret };
        extension
            .returned_effect_sender
            .send(effect_out)
            .expect("could not send the effect to the testkit");

        let effect_in = extension
            .returned_effect_receiver
            .await
            .expect("could not receive effect back from the testkit");

        effect_in.ret
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Ret> =
    Box<dyn FnOnce(ActorCell<TestExtension<M, Ret>, Ret>, MessageReceiver<M>, ActorRef<M>) -> BoxFuture<'static, (ActorCell<TestExtension<M, Ret>, Ret>, Ret)> + Send + 'static>;

pub struct UntypedBoxTestActor(Box<dyn Any + Send>);

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
