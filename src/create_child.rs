use crate::actor_ref::ActorRef;
use crate::actor_task::RunTask;
use crate::actor_to_spawn::ActorToSpawn;
use crate::children_tracker::ChildrenTracker;
use crate::receive_message::ReceiveMessage;
use std::future::Future;

pub struct ActorCell<D> {
    pub(crate) tracker: Option<ChildrenTracker>,
    pub(crate) dependency: D,
}

impl<D> ActorCell<D> {
    pub const fn new(dependency: D) -> Self {
        Self {
            tracker: None,
            dependency,
        }
    }
}

pub trait CreateChild: Sized + Send + 'static {
    type ReceiveMessageT<M>: ReceiveMessage<M> + Send + 'static
    where
        M: Send + 'static;

    type RunTaskT<M, F, Fut, Ret>: RunTask<Ret>
    where
        M: Send + 'static,
        F: FnOnce(Self, Self::ReceiveMessageT<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;

    /// Creates a child actor.
    /// The documentation of [actum](crate::actum) applies to this function as well.
    ///
    /// From within the child actor, you can [create](CreateChild::create_child) new child actors of its own.
    /// Therefore, this function effectively instantiates an actor tree hierarchy rooted in the child actor.
    ///
    /// Like the actum function, it returns a struct containing the [ActorRef] of the root actor (generic over parameter `M`) and a special control structure.
    /// The control structure has a method that returns a future which runs the child actor, joins the child actors of its own (if any), and resolves with the value returned by the child actor (generic over parameter `Ret`).
    /// Once this future resolves, the entire actor tree hierarchy rooted in the child actor has returned.
    ///
    /// You can await the future to obtain the value returned by the child actor.
    /// Otherwise, if your actor needs to do other work while the child actor is running, you can spawn the future in the background using your runtime of choice.
    /// In the case of Tokio, this can be done with the [spawn](https://docs.rs/tokio/latest/tokio/task/fn.spawn.html) function:
    /// you can await the [JoinHandle](https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html) to obtain the value returned by the child actor.
    /// If you do not do so and your actor returns, Actum will still join the child actor and the child actor of its own (if any).
    /// Therefore, your actor will never be outlived by a child actor in any case.
    ///
    /// # Example
    ///
    /// ```rust
    /// use actum::prelude::*;
    ///
    /// #[derive(Debug)]
    /// pub struct Request {
    ///     n: u32,
    ///     reply_to: ActorRef<Response>,
    /// }
    ///
    /// #[derive(Debug)]
    /// pub struct Response {
    ///     n: u32,
    /// }
    ///
    /// async fn root_actor<C, R>(mut cell: C, mut receiver: R, me: ActorRef<Response>) -> (C, ())
    /// where
    ///     C: CreateChild,
    ///     R: ReceiveMessage<Response>,
    /// {
    ///     let ActorToSpawn { task, mut actor_ref } = cell.create_child(child_actor).await;
    ///     let _handle = tokio::spawn(task.run_task());
    ///
    ///     // do other work...
    ///
    ///     let request = Request { n: 1, reply_to: me };
    ///     actor_ref.try_send(request).unwrap();
    ///
    ///     let response = receiver.recv().await.into_message().unwrap();
    ///     println!("received response: {:?}", response.n);
    ///
    ///     // handle.await.unwrap(); // not necessary
    ///     (cell, ())
    /// }
    ///
    /// async fn child_actor<C, R>(cell: C, mut receiver: R, _me: ActorRef<Request>) -> (C, ())
    /// where
    ///     C: CreateChild,
    ///     R: ReceiveMessage<Request>,
    /// {
    ///     let mut request = receiver.recv().await.into_message().unwrap();
    ///     println!("received request: {:?}", request.n);
    ///     let response = Response { n: request.n * 2 };
    ///     request.reply_to.try_send(response).unwrap();
    ///
    ///     (cell, ())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let ActorToSpawn { task, .. } = actum(root_actor);
    ///     task.run_task().await;
    /// }
    /// ```
    fn create_child<M, F, Fut, Ret>(
        &mut self,
        f: F,
    ) -> impl Future<Output = ActorToSpawn<M, Self::RunTaskT<M, F, Fut, Ret>>> + Send + '_
    where
        M: Send + 'static,
        F: FnOnce(Self, Self::ReceiveMessageT<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Ret)> + Send + 'static,
        Ret: Send + 'static;
}
