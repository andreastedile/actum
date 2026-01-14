use crate::core::actor_ref::ActorRef;
use crate::core::actor_to_spawn::CreateActorResult;
use crate::core::receive_message::ReceiveMessage;
use std::future::Future;

/// Trait to create a child actor.
pub trait CreateChild: Sized + Send + 'static {
    type ReceiveMessageT<M>: ReceiveMessage<M> + Send + 'static
    where
        M: Send + 'static;

    /// An actor's scoped task is a future that:
    /// 1. awaits the actor's future, obtaining its output;
    /// 2. awaits the completion of the control tasks of its child actors (if any);
    /// 3. returns the output of the actor's future.
    ///
    /// This ensures that actors in a tree hierarchy complete in a bottom-up order.
    type ScopedActorTaskT<M, F, Fut, Output>: Future<Output = Output> + Send + 'static
    where
        M: Send + 'static,
        F: FnOnce(Self, Self::ReceiveMessageT<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Output)> + Send + 'static,
        Output: Send + 'static;

    /// Creates a child actor of future `Fut` that can receive messages of type `M`.
    ///
    /// Returns a struct containing the [ActorRef] and the [scoped task](Self::ScopedActorTaskT) of the
    /// newly created actor.
    ///
    /// # Running the actor
    ///
    /// This method does not run the actor.
    /// To do so:
    /// - await the scoped task directly, or
    /// - spawn the scoped task into an async runtime of choice.
    ///
    /// For this reason, you can send messages to the actor before running it.
    ///
    /// # Hierarchy and lifetime
    ///
    /// From within the actor, you can create new child actors of its own.
    ///
    /// The scoped task of a parent actor only resolves after the actor's future completes and all
    /// scoped tasks of its descendant child actors, if any, have resolved as well.
    /// Therefore, an actor's scoped task never outlives the one of its parent.
    ///
    /// By awaiting an actor's scoped task, you are guaranteed that the entire subtree hierarchy of
    /// actors rooted in the actor has completed.
    ///
    /// # Obtaining the output
    ///
    /// The scoped task returns the output of the actor's future.
    /// If you want to obtain the output, await the scoped task.
    fn create_child<M, F, Fut, Output>(
        &mut self,
        f: F,
    ) -> impl Future<Output = CreateActorResult<M, Self::ScopedActorTaskT<M, F, Fut, Output>>> + Send + '_
    where
        M: Send + 'static,
        F: FnOnce(Self, Self::ReceiveMessageT<M>, ActorRef<M>) -> Fut + Send + 'static,
        Fut: Future<Output = (Self, Output)> + Send + 'static,
        Output: Send + 'static;
}
