use crate::actor_cell::ActorCell;
use crate::actor_path::ActorPath;
use crate::actor_ref::ActorRef;
use crate::actor_system::{ActorSystem, NameError};
use futures::FutureExt;
use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{instrument, trace_span, Instrument};

#[derive(Debug)]
pub enum ActumError {
    Name(NameError),
    Panic(Box<dyn Any + Send>),
}

#[instrument(level = "trace", skip(actor_fn), ret)]
pub fn actum<M, Fut>(
    name: &str,
    actor_fn: impl FnOnce(ActorCell<M>) -> Fut,
) -> Result<(), ActumError>
where
    M: Send + 'static,
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let system = match ActorSystem::new(name, runtime.handle().clone()) {
        Ok(system) => system,
        Err(error) => return Err(ActumError::Name(error)),
    };

    let path = ActorPath::new("guardian");

    let (message_sender, message_receiver) = mpsc::channel::<M>(42);
    let me = ActorRef::<M>::new(path.clone(), message_sender);

    let (_stop_sender, stop_receiver) = mpsc::unbounded_channel::<()>();

    let drop = CancellationToken::new();

    let cell = ActorCell::<M>::new(system, me, message_receiver, stop_receiver, drop.clone());

    let future = actor_fn(cell);

    let span = trace_span!(parent: None, "actor", path = ?path);
    let panic = runtime.block_on(
        async move {
            let panic: Option<Box<dyn Any + Send>> =
                AssertUnwindSafe(future).catch_unwind().await.err();

            drop.cancelled_owned().await;

            panic
        }
        .instrument(span),
    );

    panic.map_or(Ok(()), |panic| Err(ActumError::Panic(panic)))
}
