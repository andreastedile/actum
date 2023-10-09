use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::iter::FusedIterator;

use tokio::sync::mpsc;
use tokio_util::sync::{CancellationToken, DropGuard};
use tokio_util::task::JoinMap;
use tracing::{debug, instrument, trace_span, Instrument};

use crate::actorpath::ActorPath;
use crate::actorref::ActorRef;
use crate::actorsystem::ActorSystem;
use crate::actortask;
use crate::behavior::initial::InitialBehavior;

pub struct ActorContext<M, S = ()> {
    pub(crate) actor_system: ActorSystem,
    pub(crate) actor_ref: ActorRef<M>,

    pub(crate) children_tasks: JoinMap<ActorPath, Result<Option<S>, actortask::ActorError>>,
    pub(crate) children_cancellations: HashMap<ActorPath, CancellationToken>,
    pub(crate) children_watches: HashSet<ActorPath>,

    // WaitForCancellationFutureOwned
    pub(crate) watches: JoinMap<ActorPath, ()>,

    pub(crate) receiver: mpsc::Receiver<M>,

    pub(crate) cancellation: CancellationToken,
    pub(crate) _drop_guard: DropGuard,
}

impl<M, S> ActorContext<M, S>
where
    M: Debug + Send + 'static,
    S: Debug + Send + 'static,
{
    pub fn actor_system(&self) -> &ActorSystem {
        &self.actor_system
    }

    pub fn me(&self) -> &ActorRef<M> {
        &self.actor_ref
    }

    pub fn actor_path(&self) -> &ActorPath {
        &self.actor_ref.actor_path
    }

    pub fn children(&self) -> impl Iterator<Item = &ActorPath> + ExactSizeIterator + FusedIterator {
        self.children_cancellations.keys()
    }

    pub fn watches(&self) -> impl Iterator<Item = &ActorPath> + FusedIterator {
        // + ExactSizeIterator
        // https://github.com/rust-lang/rust/issues/34433
        self.children_watches
            .iter()
            .chain(self.children_cancellations.keys())
    }

    #[instrument(level = "trace", skip(self, behavior), ret)]
    pub fn spawn<M1, S1, O1>(
        &mut self,
        name: &str,
        behavior: impl Into<InitialBehavior<M1, S1, O1>> + Send + 'static,
    ) -> ActorRef<M1>
    where
        M1: Debug + Send + Sync + 'static,
        S1: Debug + Send + Sync + 'static,
        O1: Debug + Send + 'static,
        O1: Into<S>,
    {
        assert!(!name.is_empty(), "Child name is empty");
        assert!(name.is_ascii(), "Child name is not ASCII");

        let actor_path = self.actor_path().make_child_path(name);

        let (sender, receiver) = mpsc::channel::<M1>(16);

        let child_token = self.cancellation.child_token();
        let cancellation = CancellationToken::new();

        let actor_ref = ActorRef {
            actor_path: actor_path.clone(),
            sender,
            cancellation: cancellation.clone(),
        };

        let actor_context = ActorContext {
            actor_system: self.actor_system.clone(),
            actor_ref: actor_ref.clone(),
            children_tasks: Default::default(),
            children_cancellations: Default::default(),
            children_watches: Default::default(),
            watches: Default::default(),
            receiver,
            cancellation: child_token.clone(),
            _drop_guard: cancellation.drop_guard(),
        };

        let span = trace_span!(parent: None, "actor", path = ?actor_path);

        self.children_tasks.spawn(
            actor_path.clone(),
            async {
                let result = actortask::run_actor(actor_context, behavior.into()).await;
                result.map(|output| output.map(|output| output.into()))
            }
            .instrument(span),
        );

        self.children_cancellations.insert(actor_path, child_token);

        actor_ref
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub fn watch<M1>(&mut self, other: &ActorRef<M1>) -> bool
    where
        M1: Debug,
    {
        if other.actor_path == self.actor_ref.actor_path {
            panic!("Attempted to watch self");
        }

        if other.actor_path.is_ancestor_of(&self.actor_ref.actor_path) {
            panic!("Attempted to watch ancestor");
        }

        if other.cancellation.is_cancelled() {
            return false;
        }

        if other.actor_path.is_child_of(self.actor_path()) {
            //
            if !self.children_watches.insert(other.actor_path.clone()) {
                debug!("Attempted to watch a child actor that was already being watched");
            }
        } else {
            //
            if self.watches.contains_key(&other.actor_path) {
                debug!("Attempted to watch a non-ancestor, non-child actor that was already being watched");
            } else {
                self.watches.spawn(
                    other.actor_path.clone(),
                    other.cancellation.clone().cancelled_owned(),
                );
            }
        }

        true
    }

    /// Revoke the registration established by [`watch`](Self::watch).
    /// A [`PostStop`](actorinput::ActorInput) input will not subsequently be received for the referenced actor.
    #[instrument(level = "trace", skip(self))]
    pub fn unwatch<M1>(&mut self, other: &ActorRef<M1>)
    where
        M1: Debug,
    {
        if other.actor_path == self.actor_ref.actor_path {
            panic!("Attempted to unwatch self");
        }

        if other.actor_path.is_ancestor_of(&self.actor_ref.actor_path) {
            panic!("Attempted to unwatch ancestor");
        }

        if self.actor_ref.actor_path.is_parent_of(&other.actor_path) {
            if !self.children_watches.remove(&other.actor_path) {
                //
                debug!("Attempted to unwatch a child actor that was not being watched");
            }
        } else if !self.watches.abort(&other.actor_path) {
            debug!("Attempted to unwatch a non-ancestor, non-child that was not being watched");
        }
    }

    /// Force the child actor under the given name to stop after it finishes processing its current [`ActorInput`](actorinput::ActorInput) (if any).
    /// Nothing happens if the `ActorRef` is a child that is already stopped.
    #[instrument(level = "trace", skip(self))]
    pub fn stop<M1>(&mut self, child: &ActorRef<M1>) -> bool
    where
        M1: Debug,
    {
        if !child.actor_path.is_child_of(self.actor_path()) {
            panic!("Attempted to stop non-child");
        }

        if let Some(cancellation) = self.children_cancellations.remove(&child.actor_path) {
            // Don't care if it is already cancelled
            // cancellation.is_cancelled();

            cancellation.cancel();

            true
        } else {
            debug!(child = ?child, "Attempted to stop a child actor that is already stopped");

            false
        }
    }
}
