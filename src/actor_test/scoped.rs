use crate::actor_test::create_child::ActorCell;
use crate::actor_test::effect::completed_effect::{CompletedEffectToActor, CompletedEffectToTestkit};
use crate::actor_test::receive_message::MessageReceiver;
use crate::core::children_tracker::{ChildrenTracker, WakeParentOnDrop};
use crate::prelude::ActorRef;
use either::Either;
use futures::FutureExt;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomPinned;
use std::mem;
use std::mem::ManuallyDrop;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

pub struct ScopedActorTask<M, F, Fut, Output> {
    state: TaskState<M, F, Fut, Output>,
    /// None if there is no parent (thus, the actor is the root of the tree).
    _waker: Option<WakeParentOnDrop>,
    _pin: PhantomPinned,
}

enum TaskState<M, F, Fut, Output> {
    State0 {
        either: Either<F, BoxTestActor<M, Output>>,
        receiver: MessageReceiver<M>,
        cell: ActorCell,
        actor_ref: ActorRef<M>,
        completed_effect_to_testkit_sender: oneshot::Sender<CompletedEffectToTestkit<Output>>,
        completed_effect_to_actor_receiver: oneshot::Receiver<CompletedEffectToActor<Output>>,
    },
    State1 {
        either: Either<ManuallyDrop<Fut>, BoxFuture<'static, (ActorCell, Output)>>,
        completed_effect_to_testkit_sender: oneshot::Sender<CompletedEffectToTestkit<Output>>,
        completed_effect_to_actor_receiver: oneshot::Receiver<CompletedEffectToActor<Output>>,
    },
    State2 {
        tracker: ChildrenTracker,
        output: Output,
        completed_effect_to_testkit_sender: oneshot::Sender<CompletedEffectToTestkit<Output>>,
        completed_effect_to_actor_receiver: oneshot::Receiver<CompletedEffectToActor<Output>>,
    },
    State3 {
        completed_effect_to_actor_receiver: oneshot::Receiver<CompletedEffectToActor<Output>>,
    },
    Completed,
}

impl<M, F, Fut, Output> ScopedActorTask<M, F, Fut, Output> {
    pub(crate) const fn new(
        either: Either<F, BoxTestActor<M, Output>>,
        cell: ActorCell,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
        completed_effect_to_testkit_sender: oneshot::Sender<CompletedEffectToTestkit<Output>>,
        completed_effect_to_actor_receiver: oneshot::Receiver<CompletedEffectToActor<Output>>,
    ) -> Self {
        Self {
            state: TaskState::State0 {
                either,
                receiver,
                cell,
                actor_ref,
                completed_effect_to_testkit_sender,
                completed_effect_to_actor_receiver,
            },
            _waker: waker,
            _pin: PhantomPinned,
        }
    }
}

impl<M, F, Fut, Output> Future for ScopedActorTask<M, F, Fut, Output>
where
    M: Send + 'static,
    F: FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> Fut + Send + 'static,
    Fut: Future<Output = (ActorCell, Output)> + Send + 'static,
    Output: Send + 'static,
{
    type Output = Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY:
        // Field "either" can be a Left variant containing a pinned future.
        let this = unsafe { self.get_unchecked_mut() };

        loop {
            match &mut this.state {
                TaskState::State0 { .. } => {
                    // SAFETY:
                    // since this state does not contain a pinned Future yet,
                    // we can safely move out the field.
                    let TaskState::State0 {
                        either: f,
                        receiver,
                        cell,
                        actor_ref,
                        completed_effect_to_testkit_sender,
                        completed_effect_to_actor_receiver,
                    } = mem::replace(&mut this.state, TaskState::Completed)
                    else {
                        unreachable!();
                    };

                    let either = match f {
                        Either::Left(f) => {
                            let fut = f(cell, receiver, actor_ref);
                            let md = ManuallyDrop::new(fut);
                            Either::Left(md)
                        }
                        Either::Right(box_test_actor) => {
                            let box_future = box_test_actor(cell, receiver, actor_ref);
                            Either::Right(box_future)
                        }
                    };

                    this.state = TaskState::State1 {
                        either,
                        completed_effect_to_testkit_sender,
                        completed_effect_to_actor_receiver,
                    };
                    continue;
                }
                TaskState::State1 { either, .. } => {
                    let (cell, output) = match either {
                        Either::Left(fut) => {
                            // SAFETY:
                            // we poll the future in place.
                            let pin_fut = unsafe { Pin::new_unchecked(fut.deref_mut()) };
                            ready!(pin_fut.poll(cx))
                        }
                        Either::Right(box_future) => {
                            ready!(box_future.poll_unpin(cx))
                        }
                    };

                    if let Either::Left(fut) = either {
                        unsafe {
                            // SAFETY:
                            // since we will never return to this state,
                            // we effectively manually drop the future dropped once and for all.
                            ManuallyDrop::drop(fut);
                            // Field "either" does not contain a pinned future anymore.
                        }
                    };

                    // https://doc.rust-lang.org/reference/expressions/operator-expr.html?utm_source=chatgpt.com#r-expr.assign.basic
                    // It then has the effect of first dropping the value at the assigned place [...].
                    // Next it either copies or moves the assigned value to the assigned place.
                    //
                    // SAFETY:
                    // Field "either" can be a Left variant containing a pinned future,
                    // which we have manually dropped.
                    // Therefore, we can safely move the field.
                    let TaskState::State1 {
                        completed_effect_to_testkit_sender,
                        completed_effect_to_actor_receiver,
                        either,
                    } = mem::replace(&mut this.state, TaskState::Completed)
                    else {
                        unreachable!()
                    };
                    // with Either::Right<ManuallyDrop<..>>, nothing happens.
                    // with Either::Right<BoxFuture<..>>, BoxFuture is dropped.
                    drop(either);

                    if cell.tracker.has_children() {
                        tracing::trace!("joining children");

                        this.state = TaskState::State2 {
                            tracker: cell.tracker,
                            output,
                            completed_effect_to_testkit_sender,
                            completed_effect_to_actor_receiver,
                        };
                        continue;
                    } else {
                        let completed_effect_to_testkit = CompletedEffectToTestkit { output };
                        completed_effect_to_testkit_sender
                            .send(completed_effect_to_testkit)
                            .expect("could not send the effect to the testkit");

                        this.state = TaskState::State3 {
                            completed_effect_to_actor_receiver,
                        };
                        continue;
                    }
                }
                TaskState::State2 { tracker, .. } => {
                    ready!(tracker.poll(cx));

                    let TaskState::State2 {
                        output,
                        completed_effect_to_testkit_sender,
                        completed_effect_to_actor_receiver,
                        ..
                    } = mem::replace(&mut this.state, TaskState::Completed)
                    else {
                        unreachable!();
                    };

                    let completed_effect_to_testkit = CompletedEffectToTestkit { output };
                    completed_effect_to_testkit_sender
                        .send(completed_effect_to_testkit)
                        .expect("could not send the effect to the testkit");

                    this.state = TaskState::State3 {
                        completed_effect_to_actor_receiver,
                    };
                    continue;
                }
                TaskState::State3 {
                    completed_effect_to_actor_receiver,
                } => {
                    let completed_effect_to_actor = ready!(completed_effect_to_actor_receiver.poll_unpin(cx))
                        .expect("could not receive effect back from the testkit");

                    this.state = TaskState::Completed;

                    return Poll::Ready(completed_effect_to_actor.output);
                }
                TaskState::Completed => {
                    panic!("ControlTask polled after completion");
                }
            }
        }
    }
}

#[rustfmt::skip]
pub type BoxTestActor<M, Output> =
    Box<dyn FnOnce(ActorCell, MessageReceiver<M>, ActorRef<M>) -> BoxFuture<'static, (ActorCell, Output)> + Send + 'static>;

pub(crate) struct UntypedBoxTestActor(Box<dyn Any + Send>);

impl Debug for UntypedBoxTestActor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("UntypedBoxTestActor")
    }
}

impl<M, Output> From<BoxTestActor<M, Output>> for UntypedBoxTestActor
where
    M: 'static,
    Output: 'static,
{
    fn from(actor: BoxTestActor<M, Output>) -> Self {
        Self(Box::new(actor))
    }
}

impl UntypedBoxTestActor {
    pub fn downcast_unwrap<M: 'static, Output: 'static>(self) -> BoxTestActor<M, Output> {
        self.0.downcast::<BoxTestActor<M, Output>>().unwrap()
    }
}
