use crate::actorcontext::ActorContext;
use crate::actorpath::{ActorPath, ActorPathInner};
use crate::actorref::ActorRef;
use crate::actorsystem::ActorSystem;
use crate::actortask;
use crate::behavior::initial::InitialBehavior;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info_span, instrument, trace_span, Instrument};

#[instrument(level = "trace", skip(behavior, app), ret)]
pub fn actum<M, S, O>(
    actor_system: ActorSystem,
    behavior: impl Into<InitialBehavior<M, S, O>>,
    app: impl FnOnce(ActorSystem, ActorRef<M>),
) -> Result<Option<O>, actortask::ActorError>
where
    M: Debug + Send + Sync + 'static,
    S: Debug + Send + Sync + 'static,
    O: Debug + Send + 'static,
{
    let actor_path = ActorPath {
        inner: Arc::new(ActorPathInner {
            parent: None,
            name: "user".to_string(),
            full_name: "/user".to_string(),
        }),
    };

    let (sender, receiver) = mpsc::channel::<M>(16);

    let child_token = actor_system.cancellation.clone();
    let cancellation = CancellationToken::new();

    let actorref = ActorRef {
        actor_path: actor_path.clone(),
        sender,
        cancellation: cancellation.clone(),
    };

    let actor_context = ActorContext {
        actor_system: actor_system.clone(),
        actor_ref: actorref.clone(),
        children_tasks: Default::default(),
        children_cancellations: Default::default(),
        children_watches: Default::default(),
        watches: Default::default(),
        receiver,
        cancellation: child_token,
        _drop_guard: cancellation.drop_guard(),
    };

    let span = trace_span!(parent: None, "actor", path = ?actor_path);
    // let span = trace_span!("actor", path = ?actor_path);

    let handle = actor_system
        .runtime
        .spawn(actortask::run_actor(actor_context, behavior.into()).instrument(span));

    let span = info_span!(parent: None, "app");
    span.in_scope(|| {
        app(actor_system.clone(), actorref);
    });

    actor_system.runtime.block_on(handle).unwrap()
}
