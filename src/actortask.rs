use std::fmt::{Debug, Formatter};

use either::Either;
use tokio::select;
use tracing::{debug, instrument, trace};

use crate::actorcontext::ActorContext;
use crate::actorinput::ActorInput;
use crate::behavior::initial::InitialBehavior;
use crate::behavior::stopped::StoppedBehavior;
use crate::behavior::{receive, setup};

/// Behaviors that are suitable for passing a message or signal.
pub enum Interpreter<M, S = (), O = ()> {
    Receive(receive::ReceiveBehavior<M, S, O>),
    Empty,
    Ignore,
}

impl<M, S, O> Debug for Interpreter<M, S, O> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Interpreter::Receive(_) => write!(f, "Receive"),
            Interpreter::Empty => write!(f, "Empty"),
            Interpreter::Ignore => write!(f, "Ignore"),
        }
    }
}

#[derive(Debug)]
pub enum ActorError {
    DeathPact,
}

#[instrument(level = "trace", skip(actor_context, interpreter, stopped), ret)]
pub async fn stop_actor<M, S, O>(
    mut actor_context: ActorContext<M, S>,
    interpreter: Interpreter<M, S, O>,
    stopped: Option<StoppedBehavior<M, S, O>>,
) -> Option<O>
where
    M: Debug + Send + 'static,
    S: Debug + Send + 'static,
    O: Debug + Send + 'static,
{
    actor_context.receiver.close();

    actor_context.watches.abort_all();
    while actor_context.watches.join_next().await.is_some() {}

    match actor_context.children_tasks.len() {
        0 => trace!("No children to wait..."),
        1 => trace!("Waiting for 1 child to finish..."),
        n => trace!("Waiting for {n} children to finish..."),
    }

    // for (path, cancellation) in actor_context.children_cancellations.drain() {
    //     trace!(stopping = ?path.name);
    //     cancellation.cancel();
    // }
    actor_context.cancellation.cancel();
    while let Some((path, Ok(_))) = actor_context.children_tasks.join_next().await {
        trace!(finished = path.name);
    }

    if let Some(output) = stopped.map(|stopped| match stopped {
        Either::Left(output) => output,
        Either::Right(post_stop) => {
            trace!("Executing post stop");

            post_stop(&actor_context)
        }
    }) {
        return Some(output);
    }

    let output = if let Interpreter::Receive(mut receive) = interpreter {
        trace!("Executing post stop");

        let actor_input = ActorInput::PostStop {
            actor_context: &actor_context,
        };

        let next_behavior = receive(actor_input);
        trace!(next_behavior = ?next_behavior);

        match next_behavior {
            receive::NextBehavior::Receive(_) => {
                trace!("Ignoring Receive behavior");
                None
            }
            receive::NextBehavior::Setup(_) => {
                trace!("Ignoring Setup behavior");
                None
            }
            receive::NextBehavior::Same => {
                trace!("Ignoring Same behavior");
                None
            }
            receive::NextBehavior::Stopped(stopped) => {
                match stopped {
                    Either::Left(output) => Some(output),
                    Either::Right(_) => {
                        trace!("Not applicable");
                        None
                    }
                }
            }
            receive::NextBehavior::Unhandled => {
                trace!("Ignoring Unhandled behavior");
                None
            }
            receive::NextBehavior::Empty => {
                trace!("Ignoring Empty behavior");
                None
            }
            receive::NextBehavior::Ignore => {
                trace!("Ignoring Ignore behavior");
                None
            }
        }
    } else {
        None
    };

    output
}

pub async fn run_actor<M, S, O>(
    mut actor_context: ActorContext<M, S>,
    behavior: InitialBehavior<M, S, O>,
) -> Result<Option<O>, ActorError>
where
    M: Debug + Send + 'static,
    S: Debug + Send + 'static,
    O: Debug + Send + 'static,
{
    let mut interpreter = match behavior {
        InitialBehavior::Receive(receive) => Interpreter::Receive(receive),
        InitialBehavior::Setup(mut setup) => {
            trace!("Materializing setup behavior");

            let next_behavior = setup(&mut actor_context);
            trace!(next_behavior = ?next_behavior);

            match next_behavior {
                setup::NextBehavior::Receive(receive) => Interpreter::Receive(receive),
                setup::NextBehavior::Stopped(stopped) => {
                    trace!("Stopping actor...");

                    return Ok(stop_actor(actor_context, Interpreter::Empty, Some(stopped)).await);
                }
                setup::NextBehavior::Empty => Interpreter::Empty,
                setup::NextBehavior::Ignore => Interpreter::Empty,
            }
        }
        InitialBehavior::Empty => Interpreter::Empty,
        InitialBehavior::Ignore => Interpreter::Ignore,
    };

    trace!(initial = ?interpreter);

    loop {
        select! {
            biased;
            _ = actor_context.cancellation.cancelled() => {
                trace!("Stopped by ancestor actor: stopping actor...");

                return Ok(stop_actor(actor_context, interpreter, None).await);
            }
            Some((actor_path, Ok(actor_result))) = actor_context.children_tasks.join_next() => {
                trace!(interpreter = ?interpreter, actor_path = ?actor_path, actor_result = ?actor_result);

                if actor_context.children_watches.remove(&actor_path) {
                    match &mut interpreter {
                        Interpreter::Receive(receive) => {
                            let actor_input = ActorInput::Stopped {
                                actor_context: &mut actor_context,
                                actor_path: &actor_path,
                                actor_result: Some(actor_result.as_ref().map(|a| a.as_ref())),
                            };

                            let next_behavior = receive(actor_input);
                            trace!(next_behavior = ?next_behavior);

                            match next_behavior {
                                receive::NextBehavior::Receive(receive) => {
                                    interpreter = Interpreter::Receive(receive);
                                },
                                receive::NextBehavior::Setup(mut setup) => {
                                    trace!("Materializing setup behavior");

                                    let next_behavior = setup(&mut actor_context);
                                    trace!(next_behavior = ?next_behavior);

                                    match next_behavior {
                                        setup::NextBehavior::Receive(receive) => {
                                            interpreter = Interpreter::Receive(receive);
                                        }
                                        setup::NextBehavior::Stopped(stopped) => {
                                            trace!("Stopping actor...");

                                            return Ok(stop_actor(actor_context, Interpreter::Empty, Some(stopped)).await);
                                        }
                                        setup::NextBehavior::Empty => {
                                            interpreter = Interpreter::Empty;
                                        }
                                        setup::NextBehavior::Ignore => {
                                            interpreter = Interpreter::Ignore;
                                        }
                                    }
                                },
                                receive::NextBehavior::Same => { /* Maintain same behavior */ },
                                receive::NextBehavior::Stopped(stopped) => {
                                    trace!("Stopping actor...");

                                    return Ok(stop_actor(actor_context, interpreter, Some(stopped)).await);
                                },
                                receive::NextBehavior::Unhandled => {
                                    debug!(actor_path = ?actor_path, actor_result = ?actor_result, "Death pact triggered: stopping actor...");

                                    stop_actor::<M, S, O>(actor_context, Interpreter::Empty, None).await;
                                    return Err(ActorError::DeathPact);
                                },
                                receive::NextBehavior::Empty => {
                                    interpreter = Interpreter::Empty;
                                },
                                receive::NextBehavior::Ignore => {
                                    interpreter = Interpreter::Ignore;
                                },
                            }
                        },
                        Interpreter::Empty => {
                            debug!(actor_path = ?actor_path, actor_result = ?actor_result, "Death pact triggered: stopping actor...");

                            stop_actor(actor_context, interpreter, None).await;
                            return Err(ActorError::DeathPact);
                        },
                        Interpreter::Ignore => { /* Ignore input and maintain same behavior */ },
                    }
                } else {
                    // Ignore input and maintain same behavior
                }
            },
            Some((actor_path, Ok(()))) = actor_context.watches.join_next() => {
                trace!(interpreter = ?interpreter, actor_path = ?actor_path);

                match &mut interpreter {
                    Interpreter::Receive(receive) => {
                        let actor_input = ActorInput::Stopped {
                            actor_context: &mut actor_context,
                            actor_path: &actor_path,
                            actor_result: None,
                        };

                        let next_behavior = receive(actor_input);
                        trace!(next_behavior = ?next_behavior);

                        match next_behavior {
                            receive::NextBehavior::Receive(receive) => {
                                interpreter = Interpreter::Receive(receive);
                            },
                            receive::NextBehavior::Setup(mut setup) => {
                                trace!("Materializing setup behavior");

                                let next_behavior = setup(&mut actor_context);
                                trace!(next_behavior = ?next_behavior);

                                match next_behavior {
                                    setup::NextBehavior::Receive(receive) => {
                                        interpreter = Interpreter::Receive(receive);
                                    }
                                    setup::NextBehavior::Stopped(stopped) => {
                                        trace!("Stopping actor...");

                                        return Ok(stop_actor(actor_context, Interpreter::Empty, Some(stopped)).await);
                                    }
                                    setup::NextBehavior::Empty => {
                                        interpreter = Interpreter::Empty;
                                    }
                                    setup::NextBehavior::Ignore => {
                                        interpreter = Interpreter::Ignore;
                                    }
                                }
                            },
                            receive::NextBehavior::Same => { /* Maintain same behavior */ },
                            receive::NextBehavior::Stopped(stopped) => {
                                trace!("Stopping actor...");

                                return Ok(stop_actor(actor_context, interpreter, Some(stopped)).await);
                            },
                            receive::NextBehavior::Unhandled => {
                                debug!(actor_path = ?actor_path, "Death pact triggered: stopping actor...");

                                stop_actor::<M, S, O>(actor_context, Interpreter::Empty, None).await;
                                return Err(ActorError::DeathPact);
                            },
                            receive::NextBehavior::Empty => {
                                interpreter = Interpreter::Empty;
                            },
                            receive::NextBehavior::Ignore => {
                                interpreter = Interpreter::Ignore;
                            },
                        }
                    },
                    Interpreter::Empty => {
                        debug!(actor_path = ?actor_path, "Death pact triggered: stopping actor...");

                        stop_actor(actor_context, interpreter, None).await;
                        return Err(ActorError::DeathPact);
                    },
                    Interpreter::Ignore => { /* Ignore input and maintain same behavior */ },
                }
            },
            Some(message) = actor_context.receiver.recv() => {
                trace!(interpreter = ?interpreter, message = ?message);

                match &mut interpreter {
                    Interpreter::Receive(receive) => {
                        let actor_input = ActorInput::Message {
                            actor_context: &mut actor_context,
                            message: &message,
                        };

                        let next_behavior = receive(actor_input);
                        trace!(next_behavior = ?next_behavior);

                        match next_behavior {
                            receive::NextBehavior::Receive(receive) => {
                                interpreter = Interpreter::Receive(receive);
                            },
                            receive::NextBehavior::Setup(mut setup) => {
                                trace!("Materializing setup behavior");

                                let next_behavior = setup(&mut actor_context);
                                trace!(next_behavior = ?next_behavior);

                                match next_behavior {
                                    setup::NextBehavior::Receive(receive) => {
                                        interpreter = Interpreter::Receive(receive);
                                    },
                                    setup::NextBehavior::Stopped(stopped) => {
                                        trace!("Stopping actor...");

                                        return Ok(stop_actor(actor_context, Interpreter::Empty, Some(stopped)).await);
                                    },
                                    setup::NextBehavior::Empty => {
                                        interpreter = Interpreter::Empty;
                                    },
                                    setup::NextBehavior::Ignore => {
                                        interpreter = Interpreter::Ignore;
                                    },
                                }
                            },
                            receive::NextBehavior::Same => {  /* Maintain same behavior */ },
                            receive::NextBehavior::Stopped(stopped) => {
                                trace!("Stopping actor...");

                                return Ok(stop_actor(actor_context, interpreter, Some(stopped)).await);
                            },
                            receive::NextBehavior::Unhandled => {
                                debug!(unhandled = ?message);
                                // Maintain same behavior
                            },
                            receive::NextBehavior::Empty => {
                                interpreter = Interpreter::Empty;
                            },
                            receive::NextBehavior::Ignore => {
                                interpreter = Interpreter::Ignore;
                            },
                        }
                    },
                    Interpreter::Empty => {
                        debug!(unhandled = ?message);
                        // Maintain same behavior
                    },
                    Interpreter::Ignore => {  /* Ignore input and maintain same behavior */ },
                }
            },
        }
    }
}
