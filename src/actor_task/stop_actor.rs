use crate::actor_context::ActorContext;
use crate::actor_input::ActorInput;
use crate::actor_task::interpreter::Interpreter;
use crate::actor_task::{ActorError, ActorResult};
use crate::behavior::stopped;
use either::{Left, Right};
use std::panic;

pub async fn stop_voluntarily<M, O, S>(
    mut context: ActorContext<M, S>,
    interpreter: Interpreter<M, O, S>,
    stopped: stopped::Stopped<M, O, S>,
) -> ActorResult<O>
where
    M: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    context.watches.abort_all();
    while context.watches.join_next().await.is_some() {}

    context.futures.abort_all();
    while context.futures.join_next().await.is_some() {}

    context.cancellation.cancel();
    while context.children_tasks.join_next().await.is_some() {}

    match stopped.0 {
        Left(output) => {
            ActorResult(Some(Ok(output)))
            // if let Interpreter::Receive(mut receive) = interpreter {
            //     if let Err(cause) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            //         //
            //         let input = ActorInput::PostStop { context: &context };
            //         let _ignored = receive.0(input);
            //     })) {
            //         ActorResult(Some(Err(ActorError::Panic(cause))))
            //     } else {
            //         ActorResult(Some(Ok(output)))
            //     }
            // } else {
            //     ActorResult(Some(Ok(output)))
            // }
        }
        Right(callback) => match panic::catch_unwind(panic::AssertUnwindSafe(|| callback(&context))) {
            Ok(output) => ActorResult(Some(Ok(output))),
            Err(cause) => ActorResult(Some(Err(ActorError::Panic(cause)))),
        },
    }
}

pub async fn stop_involuntarily<M, O, S>(
    mut context: ActorContext<M, S>,
    interpreter: Interpreter<M, O, S>,
    error: Option<ActorError>,
) -> ActorResult<O>
where
    M: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    context.watches.abort_all();
    while context.watches.join_next().await.is_some() {}

    context.futures.abort_all();
    while context.futures.join_next().await.is_some() {}

    if error.is_none() {
        context.cancellation.cancel();
    }
    while context.children_tasks.join_next().await.is_some() {}

    if let Interpreter::Receive(mut receive) = interpreter {
        let _ignored = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            //
            let input = ActorInput::PostStop { context: &context };
            let _ignored = receive.0(input);
        }));
    }

    ActorResult(error.map(|error| Err(error)))
}
