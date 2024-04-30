use crate::actor::Actor;
use crate::actor_cell::actor_task::RunTask;
use crate::actor_cell::Stop;
use crate::actor_ref::ActorRef;
use std::future::Future;

pub trait ActorBounds<M>: Send + 'static
where
    M: Send + 'static,
{
    type ChildActorBoundsType<M2>: Send + 'static
    where
        M2: Send + 'static;
    type ChildActorBounds<M2>: ActorBounds<M2>
    where
        M2: Send + 'static;
    type SpawnOut<M2, F, Fut>: RunTask
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActorBounds<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static;

    /// Asynchronously receive the next message.
    ///
    /// This method returns `None` when the actor has been stopped by its parent or when all its [ActorRef]s have been dropped.
    /// After that this method has returned `None`, subsequent calls to the method will continue to do so.
    fn recv(&mut self) -> impl Future<Output = Option<M>> + Send + '_;

    /// Define a child actor.
    ///
    /// This method returns `Err(Stop)` when the actor has been stopped by its parent or when all its [ActorRef]s have been dropped.
    /// After that this method has returned `Err(Stop)`, subsequent calls to the method will continue to do so.
    fn spawn<M2, F, Fut>(
        &mut self,
        f: F,
    ) -> impl Future<Output = Result<Actor<M2, Self::SpawnOut<M2, F, Fut>>, Stop>> + Send + '_
    where
        M2: Send + 'static,
        F: FnOnce(Self::ChildActorBounds<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static;
}
