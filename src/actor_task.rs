mod interpreter;
mod stop_actor;

use std::any::{Any, TypeId};
use std::fmt::{Debug, Formatter, Write};
use std::panic;
use std::panic::panic_any;

use tokio::select;
use tracing::trace;

use crate::actor_context::ActorContext;
use crate::actor_task::interpreter::interpret_message;
use crate::actor_task::interpreter::interpret_supervision;
use crate::actor_task::interpreter::Interpreter;
use crate::behavior::initial::Initial;
use crate::behavior::setup;

pub struct ActorResult<O>(pub Option<Result<O, ActorError>>);

impl<O> Debug for ActorResult<O> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(Ok(_)) => f.write_str(std::any::type_name::<O>()),
            Some(Err(error)) => f.write_fmt(format_args!("{error:?}")),
            None => f.write_str("forced to stop"),
        }
    }
}

unsafe impl<O> Sync for ActorResult<O> {}

#[derive(Debug)]
pub enum ActorError {
    UnhandledSupervision,
    Panic(Box<dyn Any + Send>),
}

impl ActorError {
    fn unwrap_panic(self) -> Box<dyn Any + Send> {
        match self {
            ActorError::Panic(cause) => cause,
            ActorError::UnhandledSupervision => {
                panic_any("called `ActorError::unwrap_panic()` on an `UnhandledSupervision` value")
            }
        }
    }
}

pub async fn run_actor<M, O, S>(mut context: ActorContext<M, S>, behavior: Initial<M, O, S>) -> ActorResult<O>
where
    M: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    let mut interpreter = match behavior {
        Initial::Receive(receive) => Interpreter::Receive(receive),
        Initial::Setup(setup) => {
            trace!("Execute setup");

            match panic::catch_unwind(panic::AssertUnwindSafe(|| setup.0(&mut context))) {
                Ok(setup::Next::Receive(receive)) => {
                    //
                    Interpreter::Receive(receive)
                }
                Ok(setup::Next::Stopped(stopped)) => {
                    return stop_actor::stop_voluntarily(context, Interpreter::Empty, stopped).await;
                }
                Ok(setup::Next::Ignore) => {
                    //
                    Interpreter::Ignore
                }
                Ok(setup::Next::Empty) => {
                    //
                    Interpreter::Empty
                }
                Err(cause) => {
                    //
                    return stop_actor::stop_involuntarily(context, Interpreter::Empty, Some(ActorError::Panic(cause)))
                        .await;
                }
            }
        }
    };

    loop {
        select! {
            biased;
            _ = context.cancellation.cancelled() => {
                trace!("Forced to stop");
                break stop_actor::stop_involuntarily(context, interpreter, None).await;
            }
            Some((path, Ok(supervision))) = context.children_tasks.join_next() => {
                if !context.watches.abort(&path) {
                    trace!("Child actor stopped, child was not watched");
                    continue;
                }
                trace!("Child actor stopped, supervision");
                match interpret_supervision(&mut interpreter, &mut context, path, supervision) {
                    Ok(None) => {},
                    Ok(Some(stopped)) => {
                        //
                        break stop_actor::stop_voluntarily(context, Interpreter::Empty, stopped).await;
                    },
                    Err(error) => {
                        //
                        break stop_actor::stop_involuntarily(context, Interpreter::Empty, Some(error)).await;
                    }
                }
            },
            Some((path, Ok(()))) = context.watches.join_next() => {
                trace!("Actor stopped, supervision");
                match interpret_supervision(&mut interpreter, &mut context, path, ActorResult(None)) {
                    Ok(None) => {},
                    Ok(Some(stopped)) => {
                        //
                        break stop_actor::stop_voluntarily(context, interpreter, stopped).await;
                    },
                    Err(error) => {
                        break stop_actor::stop_involuntarily(context, interpreter, Some(error)).await;
                    }
                }
            },
            Some(result) = context.futures.join_next() => {
                let Ok(message) = result else { continue; };

                match interpret_message(&mut interpreter, &mut context, message) {
                    Ok(None) => {}
                    Ok(Some(stopped)) => {
                        break stop_actor::stop_voluntarily(context, interpreter, stopped).await;
                    },
                    Err(error) => {
                        break stop_actor::stop_involuntarily(context, interpreter, Some(error)).await;
                    }
                }
            },
            Some(message) = context.receiver.recv() => {
                trace!("Received {}", std::any::type_name::<M>());
                match interpret_message(&mut interpreter, &mut context, message) {
                    Ok(None) => {}
                    Ok(Some(stopped)) => {
                        break stop_actor::stop_voluntarily(context, interpreter, stopped).await;
                    },
                    Err(error) => {
                        break stop_actor::stop_involuntarily(context, interpreter, Some(error)).await;
                    }
                }
            },
        }
    }
}
