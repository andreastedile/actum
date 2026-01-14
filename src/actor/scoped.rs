use crate::actor::create_child::ActorCell;
use crate::actor::receive_message::MessageReceiver;
use crate::core::children_tracker::{ChildrenTracker, WakeParentOnDrop};
use crate::prelude::ActorRef;
use std::marker::PhantomPinned;
use std::mem;
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
        f: F,
        receiver: MessageReceiver<M>,
        cell: ActorCell,
        actor_ref: ActorRef<M>,
    },
    State1 {
        fut: Fut,
    },
    State2 {
        tracker: ChildrenTracker,
        output: Output,
    },
    Completed,
}

impl<M, F, Fut, Output> ScopedActorTask<M, F, Fut, Output> {
    pub(crate) const fn new(
        f: F,
        cell: ActorCell,
        receiver: MessageReceiver<M>,
        actor_ref: ActorRef<M>,
        waker: Option<WakeParentOnDrop>,
    ) -> Self {
        Self {
            state: TaskState::State0 {
                f,
                receiver,
                cell,
                actor_ref,
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

        'polling: loop {
            match &mut this.state {
                TaskState::State0 { .. } => {
                    // SAFETY:
                    // since this state does not contain a pinned Future yet,
                    // we can safely move out the field.
                    let TaskState::State0 {
                        f,
                        receiver,
                        cell,
                        actor_ref,
                    } = mem::replace(&mut this.state, TaskState::Completed)
                    else {
                        unreachable!();
                    };

                    let fut = f(cell, receiver, actor_ref);

                    this.state = TaskState::State1 { fut };
                    continue 'polling;
                }
                TaskState::State1 { fut } => {
                    // SAFETY:
                    // we poll the future in place.
                    let pin_fut = unsafe { Pin::new_unchecked(fut) };

                    let (cell, output) = ready!(pin_fut.poll(cx));

                    if cell.tracker.has_children() {
                        tracing::trace!("joining children");

                        // https://doc.rust-lang.org/reference/expressions/operator-expr.html?utm_source=chatgpt.com#r-expr.assign.basic
                        // It then has the effect of first dropping the value at the assigned place [...].
                        // Next it either copies or moves the assigned value to the assigned place.
                        //
                        // SAFETY:
                        // since we will never return to this state,
                        // we effectively drop the future dropped once and for all.
                        this.state = TaskState::State2 {
                            tracker: cell.tracker,
                            output,
                        };
                        continue 'polling;
                    } else {
                        // SAFETY:
                        // see comment above.
                        this.state = TaskState::Completed;

                        return Poll::Ready(output);
                    }
                }
                TaskState::State2 { tracker, .. } => {
                    ready!(tracker.poll(cx));

                    let TaskState::State2 { output, .. } = std::mem::replace(&mut this.state, TaskState::Completed)
                    else {
                        unreachable!();
                    };

                    this.state = TaskState::Completed;

                    return Poll::Ready(output);
                }
                TaskState::Completed => {
                    panic!("polled after completion");
                }
            }
        }
    }
}
