use crate::actor::Actor;
use crate::actor_bounds::{ActorBounds, Recv};
use crate::actor_cell::actor_task::ActorTask;
use crate::actor_cell::ActorCell;
use crate::actor_cell::Stop;
use crate::actor_ref::ActorRef;
use crate::drop_guard::ActorDropGuard;
use crate::resolve_when_one::ResolveWhenOne;
use crate::testkit::{AnyTestkit, Testkit};
use futures::channel::{mpsc, oneshot};
use futures::future::{Either, FusedFuture};
use futures::stream::FusedStream;
use futures::{future, SinkExt, StreamExt};
use std::future::Future;

pub struct TestBounds<M> {
    /// Used to send a [Recv] to the [Testkit].
    /// See [ActorBounds::recv].
    recv_m_sender: mpsc::Sender<Recv<M>>,
    /// Used to receive the [Recv] back from the [Testkit] to which it was sent.
    recv_m_receiver: mpsc::Receiver<Recv<M>>,
    /// Used to send the optional [Testkit] of a child actor to the [Testkit].
    /// See [ActorBounds::spawn].
    testkit_sender: mpsc::Sender<Option<AnyTestkit>>,
    /// Used to receive a confirmation back from the [Testkit] to which the [Testkit] of the child actor was sent.
    testkit_receiver: mpsc::Receiver<()>,
    awaiting_testkit: bool,
}

impl<M> TestBounds<M> {
    pub const fn new(
        recv_m_sender: mpsc::Sender<Recv<M>>,
        recv_m_receiver: mpsc::Receiver<Recv<M>>,
        testkit_sender: mpsc::Sender<Option<AnyTestkit>>,
        testkit_receiver: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            recv_m_sender,
            recv_m_receiver,
            testkit_sender,
            testkit_receiver,
            awaiting_testkit: false,
        }
    }
}

impl<M> ActorBounds<M> for ActorCell<M, TestBounds<M>>
where
    M: Send + 'static,
{
    type ChildActorBoundsType<M2: Send + 'static> = TestBounds<M2>;
    type ChildActorBounds<M2: Send + 'static> = ActorCell<M2, TestBounds<M2>>;
    type SpawnOut<M2, F, Fut, Ret>
        = ActorTask<M2, F, Fut, Ret, TestBounds<M2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, TestBounds<M2>>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    async fn recv(&mut self) -> Recv<M> {
        assert!(!self.bounds.recv_m_sender.is_closed());

        if !self.bounds.awaiting_testkit {
            let select = future::select(&mut self.stop_receiver, self.m_receiver.next()).await;

            let recv = match select {
                Either::Left(_) /* (Result<Stop, Canceled>, Next<Receiver<M>>) */ => {
                    self.m_receiver.close();
                    let m = self.m_receiver.try_next().expect("the message receiver should stay open as long as the stop receiver");
                    Recv::Stopped(m)
                }
                Either::Right((Some(m), _)) => Recv::Message(m),
                Either::Right((None, _)) => Recv::NoMoreSenders,
            };

            self.bounds
                .recv_m_sender
                .try_send(recv)
                .expect("could not send the Recv to the testkit");

            // there is an await next
            self.bounds.awaiting_testkit = true; // now the future is cancel safe
        }

        let recv = self
            .bounds
            .recv_m_receiver
            .next()
            .await
            .expect("could not receive the Recv back from the testkit");

        self.bounds.awaiting_testkit = false;

        recv
    }

    async fn spawn<M2, F, Fut, Ret>(&mut self, f: F) -> Option<Actor<M2, ActorTask<M2, F, Fut, Ret, TestBounds<M2>>>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<M2, TestBounds<M2>>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<M2, TestBounds<M2>>, Ret)> + Send + 'static,
    {
        assert!(!self.bounds.testkit_sender.is_closed());

        if self.stop_receiver.is_terminated() || self.m_receiver.is_terminated() {
            self.bounds
                .testkit_sender
                .send(None)
                .await
                .expect("could not send to the testkit");

            let _ = self
                .bounds
                .testkit_receiver
                .next()
                .await
                .expect("cold not receive back from the testkit");

            return None;
        }

        let stop_channel = oneshot::channel::<Stop>();
        let m2_channel = mpsc::channel::<M2>(100);

        let guard = ActorDropGuard::new(stop_channel.0);
        let recv_m_channel_out = mpsc::channel::<Recv<M2>>(1);
        let recv_m_channel_in = mpsc::channel::<Recv<M2>>(1);
        let testkit_channel_out = mpsc::channel::<Option<AnyTestkit>>(1);
        let testkit_channel_in = mpsc::channel::<()>(1);
        let bounds = TestBounds::new(
            recv_m_channel_out.0,
            recv_m_channel_in.1,
            testkit_channel_out.0,
            testkit_channel_in.1,
        );
        let cell = ActorCell::new(stop_channel.1, m2_channel.1, bounds);

        let m2_ref = ActorRef::new(m2_channel.0);
        let subtree = self.subtree.get_or_insert(ResolveWhenOne::new());
        let task = ActorTask::new(f, cell, m2_ref.clone(), Some(subtree.clone()));

        let testkit = Testkit::new(
            recv_m_channel_out.1,
            recv_m_channel_in.0,
            testkit_channel_out.1,
            testkit_channel_in.0,
        );

        self.bounds
            .testkit_sender
            .try_send(Some(AnyTestkit::from(testkit)))
            .expect("could not send the child actor testkit to the testkit");

        self.bounds
            .testkit_receiver
            .next()
            .await
            .expect("cold not receive back from the testkit");

        Some(Actor::new(task, guard, m2_ref))
    }
}
