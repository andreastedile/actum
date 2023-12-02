use crate::actor_context::ActorContext;
use crate::actor_path::{ActorPath, ActorPathInner};
use crate::actor_ref::{ActorRef, ActorRefInner};
use crate::actor_system::ActorSystem;
use crate::actor_task::{run_actor, ActorResult};
use crate::behavior::initial::Initial;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info_span, instrument, trace_span, Instrument};

#[instrument(level = "trace", skip(behavior, app), ret)]
pub fn actum<M, O, S>(
    system: ActorSystem,
    behavior: impl Into<Initial<M, O, S>>,
    app: impl FnOnce(ActorRef<M>),
) -> ActorResult<O>
where
    M: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    let path = ActorPath(Arc::new(ActorPathInner {
        parent: None,
        name: "user".to_string(),
    }));

    let (sender, receiver) = mpsc::channel::<M>(16);

    let child_token = system.cancellation.child_token();

    // Used to watch the actor.
    let cancellation = CancellationToken::new();

    let me = ActorRef(Arc::new(ActorRefInner {
        cancellation: cancellation.clone(),
        path: path.clone(),
        send_fn: Box::new(move |message| sender.try_send(message).is_ok()),
    }));

    let context = ActorContext {
        system: system.clone(),
        me: me.clone(),
        cancellation: child_token,
        children_tasks: Default::default(),
        children_tokens: Default::default(),
        watches: Default::default(),
        futures: Default::default(),
        receiver,
        _drop_guard: cancellation.drop_guard(),
    };

    let span = trace_span!(parent: None, "actor", path = ?me.path);
    let handle = system
        .runtime
        .spawn(run_actor(context, behavior.into()).instrument(span));

    let span = info_span!(parent: None, "app");
    span.in_scope(|| {
        app(me);
    });

    system.runtime.block_on(handle).unwrap()
}
