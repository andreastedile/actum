use crate::actor_context::ActorContext;
use crate::actor_input::ActorInput;
use crate::actor_path::ActorPath;
use crate::actor_task::ActorError;
use crate::behavior::{receive, setup, stopped};
use std::panic;

pub enum Interpreter<M, O = (), S = ()> {
    Receive(receive::Receive<M, O, S>),
    Empty,
    Ignore,
}

impl<M, O, S> std::fmt::Debug for Interpreter<M, O, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Interpreter::Receive(_) => write!(f, "Receive"),
            Interpreter::Empty => write!(f, "Empty"),
            Interpreter::Ignore => write!(f, "Ignore"),
        }
    }
}

pub fn interpret_supervision<M, O, S>(
    interpreter: &mut Interpreter<M, O, S>,
    context: &mut ActorContext<M, S>,
    path: ActorPath,
    supervision: Option<Result<S, ActorError>>,
) -> Result<Option<stopped::Stopped<M, O, S>>, ActorError>
where
    M: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    match panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let input = ActorInput::Supervision {
            context,
            path,
            supervision,
        };

        match interpreter {
            Interpreter::Receive(receive) => match receive.0(input) {
                receive::Next::Receive(receive) => {
                    *interpreter = Interpreter::Receive(receive);
                    Ok(None)
                }
                receive::Next::Same => Ok(None),
                receive::Next::Stopped(stopped) => Ok(Some(stopped)),
                receive::Next::Setup(setup) => {
                    // Materialize the setup behavior.

                    match setup.0(context) {
                        setup::Next::Receive(receive) => {
                            *interpreter = Interpreter::Receive(receive);
                            Ok(None)
                        }
                        setup::Next::Stopped(stopped) => Ok(Some(stopped)),
                        setup::Next::Ignore => {
                            *interpreter = Interpreter::Ignore;
                            Ok(None)
                        }
                        setup::Next::Empty => {
                            *interpreter = Interpreter::Empty;
                            Ok(None)
                        }
                    }
                }
                receive::Next::Unhandled => Err(ActorError::UnhandledSupervision),
                receive::Next::Ignore => {
                    *interpreter = Interpreter::Ignore;
                    Ok(None)
                }
                receive::Next::Empty => {
                    *interpreter = Interpreter::Empty;
                    Ok(None)
                }
            },
            Interpreter::Empty => Err(ActorError::UnhandledSupervision),
            Interpreter::Ignore => Ok(None),
        }
    })) {
        Ok(inner) => inner,
        Err(cause) => Err(ActorError::Panic(cause)),
    }
}

pub fn interpret_message<M, O, S>(
    interpreter: &mut Interpreter<M, O, S>,
    context: &mut ActorContext<M, S>,
    message: M,
) -> Result<Option<stopped::Stopped<M, O, S>>, ActorError>
where
    M: Send + Sync + 'static,
    O: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    match panic::catch_unwind(panic::AssertUnwindSafe(|| match interpreter {
        Interpreter::Receive(receive) => {
            let input = ActorInput::Message { context, message };

            match receive.0(input) {
                receive::Next::Receive(receive) => {
                    *interpreter = Interpreter::Receive(receive);
                    Ok(None)
                }
                receive::Next::Same => Ok(None),
                receive::Next::Stopped(stopped) => Ok(Some(stopped)),
                receive::Next::Setup(setup) => {
                    // Materialize the setup behavior.

                    match setup.0(context) {
                        setup::Next::Receive(receive) => {
                            *interpreter = Interpreter::Receive(receive);
                            Ok(None)
                        }
                        setup::Next::Stopped(stopped) => Ok(Some(stopped)),
                        setup::Next::Ignore => {
                            *interpreter = Interpreter::Ignore;
                            Ok(None)
                        }
                        setup::Next::Empty => {
                            *interpreter = Interpreter::Empty;
                            Ok(None)
                        }
                    }
                }
                receive::Next::Unhandled => Ok(None),
                receive::Next::Ignore => {
                    *interpreter = Interpreter::Ignore;
                    Ok(None)
                }
                receive::Next::Empty => {
                    *interpreter = Interpreter::Empty;
                    Ok(None)
                }
            }
        }
        Interpreter::Empty => {
            *interpreter = Interpreter::Empty;
            Ok(None)
        }
        Interpreter::Ignore => {
            *interpreter = Interpreter::Ignore;
            Ok(None)
        }
    })) {
        Ok(inner) => inner,
        Err(cause) => Err(ActorError::Panic(cause)),
    }
}
