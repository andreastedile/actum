use crate::actor_path::{ActorPath, ActorPathInner};
use crate::actor_ref::{ActorRef, ActorRefInner};
use crate::actor_task::{run_actor, ActorResult};
use crate::behavior::initial::Initial;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::iter::FusedIterator;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::task::{AbortHandle, JoinSet};
use tokio_util::sync::{CancellationToken, DropGuard};
use tokio_util::task::JoinMap;

use crate::actor_system::ActorSystem;

/// Errors that can occur using [`ActorContext::spawn`].
#[derive(Debug)]
pub enum SpawnError {
    EmptyName,
    NonAlphanumeric,
    NonUniqueName,
}

#[derive(Debug)]
pub struct WatchError;
#[derive(Debug)]
pub struct UnwatchError;
#[derive(Debug)]
pub struct StopError;

pub struct ActorContext<M, S> {
    pub me: ActorRef<M>,
    pub system: ActorSystem,
    pub(crate) cancellation: CancellationToken,
    pub(crate) children_tasks: JoinMap<ActorPath, ActorResult<S>>,
    pub(crate) children_tokens: HashMap<ActorPath, CancellationToken>,
    pub(crate) watches: JoinMap<ActorPath, ()>,
    pub(crate) futures: JoinSet<M>,
    pub(crate) receiver: Receiver<M>,
    pub(crate) _drop_guard: DropGuard,
}

impl<M, S> ActorContext<M, S>
where
    M: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    pub fn spawn<M1, O1, S1>(
        &mut self,
        name: &str,
        behavior: impl Into<Initial<M1, O1, S1>> + Send + 'static,
    ) -> Result<ActorRef<M1>, SpawnError>
    where
        M1: Send + Sync + 'static,
        S1: Send + Sync + 'static,
        O1: Send + Sync + 'static,
        O1: Into<S>,
    {
        if name.is_empty() {
            return Err(SpawnError::EmptyName);
        }

        if name.chars().any(|char| !char.is_ascii_alphanumeric()) {
            return Err(SpawnError::NonAlphanumeric);
        }

        if self.children_tasks.keys().any(|path| name == path.name) {
            return Err(SpawnError::NonUniqueName);
        }

        let path = ActorPath(Arc::new(ActorPathInner {
            parent: Some(self.me.path.clone()),
            name: name.to_string(),
        }));

        let (sender, receiver) = mpsc::channel::<M1>(16);

        let child_token = self.cancellation.child_token();

        // Used to watch the actor.
        let cancellation = CancellationToken::new();

        let me = ActorRef(Arc::new(ActorRefInner {
            path,
            cancellation: cancellation.clone(),
            send_fn: Box::new(move |message| sender.try_send(message).is_ok()),
        }));

        let actor_context = ActorContext {
            system: self.system.clone(),
            me: me.clone(),
            cancellation: child_token.clone(),
            children_tasks: Default::default(),
            children_tokens: Default::default(),
            watches: Default::default(),
            futures: Default::default(),
            receiver,
            _drop_guard: cancellation.drop_guard(),
        };

        self.children_tasks.spawn(me.path.clone(), async {
            run_actor(actor_context, behavior.into())
                .await
                .map(|result| result.map(|output| output.into()))
        });

        self.children_tokens.insert(me.path.clone(), child_token);

        Ok(me)
    }

    pub fn children(&self) -> impl Iterator<Item = &ActorPath> + ExactSizeIterator + FusedIterator {
        self.children_tasks.keys()
    }

    pub fn watch<M1>(&mut self, actor: &ActorRef<M1>) -> Result<(), WatchError> {
        if actor.path.is_descendant_of(&self.me.path) || !actor.path.is_ancestor_of(&self.me.path) {
            self.watches
                .spawn(actor.path.clone(), actor.cancellation.clone().cancelled_owned());
            Ok(())
        } else {
            Err(WatchError)
        }
    }

    pub fn unwatch<M1>(&mut self, actor: &ActorRef<M1>) -> Result<(), UnwatchError> {
        if actor.path.is_descendant_of(&self.me.path) || !actor.path.is_ancestor_of(&self.me.path) {
            self.watches.abort(&actor.path);
            Ok(())
        } else {
            Err(UnwatchError)
        }
    }

    pub fn stop<M1>(&mut self, actor: &ActorRef<M1>) -> Result<(), StopError> {
        if actor.path.is_child_of(&self.me.path) {
            if let Some(token) = self.children_tokens.get(&actor.path) {
                token.cancel();
            }
            Ok(())
        } else {
            Err(StopError)
        }
    }

    pub fn pipe_to_self(&mut self, future: impl Future<Output = M> + Send + 'static) -> AbortHandle {
        self.futures.spawn(future)
    }
}
