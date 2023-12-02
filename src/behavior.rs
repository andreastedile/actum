use crate::actor_context::ActorContext;
use crate::actor_input::ActorInput;
use crate::actor_path::ActorPath;
use crate::actor_task::{ActorError, ActorResult};
use std::panic::panic_any;
use tracing::trace;

pub mod initial;
pub mod receive;
pub mod setup;
pub mod stopped;

pub fn receive<M, O, S>(
    handler: impl FnMut(ActorInput<M, S>) -> receive::Next<M, O, S> + Send + 'static,
) -> receive::Receive<M, O, S> {
    receive::Receive(Box::new(handler))
}
pub fn receive_message<M, O, S>(
    mut handler: impl FnMut(&mut ActorContext<M, S>, M) -> receive::Next<M, O, S> + Send + 'static,
) -> receive::Receive<M, O, S> {
    receive::Receive(Box::new(move |input| {
        if let ActorInput::Message { context, message } = input {
            handler(context, message)
        } else {
            receive::Next::Unhandled
        }
    }))
}
pub fn receive_supervision<M, O, S>(
    mut handler: impl FnMut(&mut ActorContext<M, S>, ActorPath, ActorResult<S>) -> receive::Next<M, O, S> + Send + 'static,
) -> receive::Receive<M, O, S> {
    receive::Receive(Box::new(move |input| {
        if let ActorInput::Supervision { context, path, result } = input {
            handler(context, path, result)
        } else {
            receive::Next::Unhandled
        }
    }))
}
pub fn receive_post_stop<M, O, S>(
    mut handler: impl FnMut(&ActorContext<M, S>) -> O + Send + 'static,
) -> receive::Receive<M, O, S> {
    receive::Receive(Box::new(move |input| {
        if let ActorInput::PostStop { context } = input {
            let output = handler(context);
            receive::Next::Stopped(stopped::Stopped(either::Either::Left(output)))
        } else {
            receive::Next::Unhandled
        }
    }))
}
pub fn setup<M, O, S>(
    handler: impl FnOnce(&mut ActorContext<M, S>) -> setup::Next<M, O, S> + Send + 'static,
) -> setup::Setup<M, O, S> {
    setup::Setup(Box::new(handler))
}
pub fn stop_with_output<M, O, S>(output: O) -> stopped::Stopped<M, O, S> {
    stopped::Stopped(either::Left(output))
}
pub fn stop_with_callback<M, O, S>(
    callback: impl FnOnce(&ActorContext<M, S>) -> O + Send + 'static,
) -> stopped::Stopped<M, O, S> {
    stopped::Stopped(either::Right(Box::new(callback)))
}

///
///
/// # Arguments
///
/// * `behavior`:
///
/// returns: Setup<Pub, ActorResult<O>, O>
///
/// # Panics
///
/// ```
///
/// ```
pub fn narrow<Pub, PubPriv, O, S>(behavior: impl Into<initial::Initial<PubPriv, O, S>> + Send + 'static) -> setup::Setup<Pub, ActorResult<O>, O>
where
    PubPriv: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
    Pub: Into<PubPriv> + Send + Sync + 'static,
{
    setup::Setup(Box::new(move |context| {
        let child = context.spawn("wrapped", behavior).unwrap();
        context.watch(&child);

        setup::Next::Receive(receive::Receive(Box::new(move |input| match input {
            ActorInput::Message { message, .. } => {
                child.send(message.into());
                receive::Next::Same
            }
            ActorInput::Supervision { result, .. } => receive::Next::Stopped(stopped::Stopped(either::Left(result))),
            ActorInput::PostStop { .. } => receive::Next::Same,
        })))
    }))
}
