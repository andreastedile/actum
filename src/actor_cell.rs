use crate::actor_input::ActorInput;
use crate::actor_path::ActorPath;
use crate::actor_ref::ActorRef;
use crate::actor_system::ActorSystem;
use futures::FutureExt;
use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_util::sync::CancellationToken;
use tracing::{instrument, trace, trace_span, Instrument};

pub struct ActorCell<M> {
    pub system: ActorSystem,
    pub me: ActorRef<M>,
    message_receiver: mpsc::Receiver<M>,
    stop_receiver: mpsc::UnboundedReceiver<()>,
    children: Vec<ChildActor>,
    drop: CancellationToken,
}

struct ChildActor {
    path: ActorPath,
    stop_sender: mpsc::UnboundedSender<()>,
    result_receiver: mpsc::UnboundedReceiver<Option<Box<dyn Any + Send>>>,
}

impl<M> Drop for ActorCell<M> {
    fn drop(&mut self) {
        trace!("Dropping");

        self.stop_receiver.close();
        self.message_receiver.close();

        while self.stop_receiver.try_recv().is_ok() {}
        while self.message_receiver.try_recv().is_ok() {}

        let children = std::mem::take(&mut self.children);
        let drop = std::mem::take(&mut self.drop);

        self.system.runtime.spawn(
            async move {
                for child in &children {
                    let _ = child.stop_sender.send(());
                }

                for mut child in children {
                    child
                        .result_receiver
                        .recv()
                        .await
                        .expect("Could not receive the result from child");
                }

                drop.cancel();
            }
            .in_current_span(),
        );
    }
}

impl<M> Future for ActorCell<M> {
    type Output = Option<ActorInput<M>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.stop_receiver.poll_recv(cx).is_ready() {
            trace!("Stopping");

            self.stop_receiver.close();
            self.message_receiver.close();

            while self.stop_receiver.poll_recv(cx).is_ready() {}
            while self.message_receiver.poll_recv(cx).is_ready() {}

            for child in &self.children {
                let _ = child.stop_sender.send(());
            }

            self.children
                .retain_mut(|child| child.result_receiver.poll_recv(cx).is_pending());

            if self.children.is_empty() {
                return Poll::Ready(None);
            }

            return Poll::Pending;
        }

        for i in 0..self.children.len() {
            if let Poll::Ready(Some(panic)) = self.children[i].result_receiver.poll_recv(cx) {
                let path = self.children.remove(i).path;
                let input = ActorInput::Supervision { path, panic };
                return Poll::Ready(Some(input));
            }
        }

        if let Poll::Ready(Some(message)) = self.message_receiver.poll_recv(cx) {
            let input = ActorInput::Message(message);
            return Poll::Ready(Some(input));
        }

        Poll::Pending
    }
}

impl<M> Stream for ActorCell<M> {
    type Item = ActorInput<M>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll(cx)
    }
}

impl<M> ActorCell<M> {
    pub fn new(
        system: ActorSystem,
        me: ActorRef<M>,
        message_receiver: mpsc::Receiver<M>,
        stop_receiver: mpsc::UnboundedReceiver<()>,
        drop: CancellationToken,
    ) -> Self {
        Self {
            system,
            me,
            message_receiver,
            stop_receiver,
            children: Vec::new(),
            drop,
        }
    }

    #[instrument(level = "trace", skip(self, actor_fn), ret)]
    pub fn spawn<M2, Fut>(
        &mut self,
        name: &str,
        actor_fn: impl FnOnce(ActorCell<M2>) -> Fut,
    ) -> Option<ActorRef<M2>>
    where
        M2: Send + 'static,
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        if self.stop_receiver.try_recv().is_ok() {
            trace!("Stopping");

            self.stop_receiver.close();
            self.message_receiver.close();

            while self.stop_receiver.try_recv().is_ok() {}
            while self.message_receiver.try_recv().is_ok() {}

            for child in &self.children {
                let _ = child.stop_sender.send(());
            }

            return None;
        }

        let path = self.me.path.make_child(name);

        let (message_sender, message_receiver) = mpsc::channel::<M2>(42);
        let me = ActorRef::<M2>::new(path.clone(), message_sender);

        let (stop_sender, stop_receiver) = mpsc::unbounded_channel::<()>();

        let drop = CancellationToken::new();

        let (result_sender, result_receiver) =
            mpsc::unbounded_channel::<Option<Box<dyn Any + Send>>>();

        let cell = ActorCell::<M2>::new(
            self.system.clone(),
            me.clone(),
            message_receiver,
            stop_receiver,
            drop.clone(),
        );

        let child = ChildActor {
            path: path.clone(),
            stop_sender,
            result_receiver,
        };

        self.children.push(child);

        let future = actor_fn(cell);

        let span = trace_span!(parent: None, "actor", path = ?path);
        self.system.runtime.spawn(
            async move {
                let panic: Option<Box<dyn Any + Send>> =
                    AssertUnwindSafe(future).catch_unwind().await.err();

                drop.cancelled_owned().await;

                result_sender
                    .send(panic)
                    .expect("Could not send the result to parent");
            }
            .instrument(span),
        );

        Some(me)
    }

    pub fn stop(&self, child: &ActorPath) {
        if child.is_child_of(&self.me.path) {
            if let Some(child) = self
                .children
                .iter()
                .find(|other| other.path.is_same_as(child))
            {
                let _ = child.stop_sender.send(());
            }
        }
    }
}
