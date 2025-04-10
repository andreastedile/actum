use crate::actor_ref::ActorRef;
use crate::actor_ref::ExtendableMessageReceiver;
use crate::children_tracker::WakeParentOnDrop;
use crate::create_child::ActorCell;
use std::future::Future;
use std::marker::PhantomData;

pub trait RunTask<Ret>: Send + 'static {
    fn run_task(self) -> impl Future<Output = Ret> + Send + 'static;
}

pub struct ExtensibleActorTask<M, F, Fut, Ret, C, R, D> {
    pub(crate) f: F,
    ret: PhantomData<Ret>,
    fut: PhantomData<Fut>,
    pub(crate) receiver: ExtendableMessageReceiver<M, R>,
    pub(crate) cell: ActorCell<C>,
    pub(crate) actor_ref: ActorRef<M>,
    pub(crate) dependency: D,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
}

impl<M, F, Fut, Ret, C, R, D> ExtensibleActorTask<M, F, Fut, Ret, C, R, D> {
    pub(crate) const fn new(
        f: F,
        cell: ActorCell<C>,
        receiver: ExtendableMessageReceiver<M, R>,
        actor_ref: ActorRef<M>,
        dependency: D,
        waker: Option<WakeParentOnDrop>,
    ) -> Self {
        Self {
            f,
            ret: PhantomData,
            fut: PhantomData,
            receiver,
            cell,
            actor_ref,
            dependency,
            _waker: waker,
        }
    }
}
