use crate::actor::{Actor, Recv};
use crate::actor_cell::ActorCell;
use crate::actor_ref::ActorRef;
use crate::actor_task::ActorTask;
use crate::actor_to_spawn::ActorToSpawn;
use crate::effect::{
    RecvEffectFromActorToTestkit, RecvEffectFromTestkitToActor, SpawnEffectFromActorToTestkit,
    SpawnEffectFromTestkitToActor,
};
use crate::message_receiver::MessageReceiver;
use crate::testkit::Testkit;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use std::future::{poll_fn, Future};
use std::task::{ready, Poll};

pub struct TestExtension<M> {
    recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
    recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
    spawn_effect_sender: mpsc::Sender<SpawnEffectFromActorToTestkit>,
    spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,
    state: RecvFutureStateMachine,
}

impl<M> TestExtension<M> {
    pub const fn new(
        recv_effect_sender: mpsc::Sender<RecvEffectFromActorToTestkit<M>>,
        recv_effect_receiver: mpsc::Receiver<RecvEffectFromTestkitToActor<M>>,
        spawn_effect_sender: mpsc::Sender<SpawnEffectFromActorToTestkit>,
        spawn_effect_receiver: mpsc::Receiver<SpawnEffectFromTestkitToActor>,
    ) -> Self {
        Self {
            recv_effect_sender,
            recv_effect_receiver,
            spawn_effect_sender,
            spawn_effect_receiver,
            state: RecvFutureStateMachine::S0,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum RecvFutureStateMachine {
    S0,
    S1,
    S2,
}

impl<M> Actor<M> for ActorCell<TestExtension<M>>
where
    M: Send + 'static,
{
    type ChildActorDependency<M2: Send + 'static> = TestExtension<M2>;
    type ChildActor<M2: Send + 'static> = ActorCell<TestExtension<M2>>;
    type HasRunTask<M2, F, Fut, Ret>
        = ActorTask<M2, F, Fut, Ret, TestExtension<M2>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<TestExtension<M2>>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<TestExtension<M2>>, Ret)> + Send + 'static,
        Ret: Send + 'static;

    fn recv<'a>(&'a mut self, receiver: &'a mut MessageReceiver<M>) -> impl Future<Output = Recv<M>> + Send + 'a {
        if self.dependency.state == RecvFutureStateMachine::S1 {
            // If the state is S1, it means that the previous future was dropped while it was waiting for the effect to
            // come back from the testkit.
            self.dependency.state = RecvFutureStateMachine::S2;
        }

        poll_fn(|cx| {
            if self.dependency.recv_effect_sender.is_closed() {
                panic!("the testkit has dropped");
            }

            loop {
                match self.dependency.state {
                    RecvFutureStateMachine::S0 => {
                        let recv = ready!(receiver.recv().poll_unpin(cx));

                        let effect_out = RecvEffectFromActorToTestkit(recv);

                        self.dependency
                            .recv_effect_sender
                            .try_send(effect_out)
                            .expect("could not send the effect to the testkit");

                        self.dependency.state = RecvFutureStateMachine::S1;
                    }
                    RecvFutureStateMachine::S1 => {
                        let effect_in = match self.dependency.recv_effect_receiver.poll_next_unpin(cx) {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(effect_in)) => effect_in,
                            Poll::Pending => return Poll::Pending,
                        };

                        self.dependency.state = RecvFutureStateMachine::S0;

                        if !effect_in.discarded {
                            return Poll::Ready(effect_in.recv);
                        } // else: poll the channels in the next iteration
                    }
                    RecvFutureStateMachine::S2 => {
                        let effect_in = match self.dependency.recv_effect_receiver.poll_next_unpin(cx) {
                            Poll::Ready(None) => panic!("could not receive effect back from the testkit"),
                            Poll::Ready(Some(effect_in)) => effect_in,
                            Poll::Pending => return Poll::Pending,
                        };

                        let effect_out = RecvEffectFromActorToTestkit(effect_in.recv);

                        self.dependency
                            .recv_effect_sender
                            .try_send(effect_out)
                            .expect("could not send the effect to the testkit");

                        self.dependency.state = RecvFutureStateMachine::S1;
                    }
                }
            }
        })
    }

    async fn create_child<M2, F, Fut, Ret>(
        &mut self,
        f: F,
    ) -> ActorToSpawn<M2, ActorTask<M2, F, Fut, Ret, TestExtension<M2>>>
    where
        M2: Send + 'static,
        F: FnOnce(ActorCell<TestExtension<M2>>, MessageReceiver<M2>, ActorRef<M2>) -> Fut + Send + 'static,
        Fut: Future<Output = (ActorCell<TestExtension<M2>>, Ret)> + Send + 'static,
        Ret: Send + 'static,
    {
        assert!(!self.dependency.spawn_effect_sender.is_closed());

        let m2_channel = mpsc::channel::<M2>(100);
        let receiver = MessageReceiver::<M2>::new(m2_channel.1);
        let actor_ref = ActorRef::<M2>::new(m2_channel.0);

        let recv_effect_actor_to_testkit_channel = mpsc::channel::<RecvEffectFromActorToTestkit<M2>>(1);
        let recv_effect_testkit_to_actor_channel = mpsc::channel::<RecvEffectFromTestkitToActor<M2>>(1);
        let spawn_effect_actor_to_testkit_channel = mpsc::channel::<SpawnEffectFromActorToTestkit>(1);
        let spawn_effect_testkit_to_actor_channel = mpsc::channel::<SpawnEffectFromTestkitToActor>(1);
        let extension = TestExtension::new(
            recv_effect_actor_to_testkit_channel.0,
            recv_effect_testkit_to_actor_channel.1,
            spawn_effect_actor_to_testkit_channel.0,
            spawn_effect_testkit_to_actor_channel.1,
        );

        let cell = ActorCell::new(extension);

        let tracker = self.tracker.get_or_insert_default();
        let task = ActorTask::new(f, cell, receiver, actor_ref.clone(), Some(tracker.make_child()));

        let testkit = Testkit::new(
            recv_effect_actor_to_testkit_channel.1,
            recv_effect_testkit_to_actor_channel.0,
            spawn_effect_actor_to_testkit_channel.1,
            spawn_effect_testkit_to_actor_channel.0,
        );
        let spawn_effect_to_testkit = SpawnEffectFromActorToTestkit(Some(testkit.into()));

        self.dependency
            .spawn_effect_sender
            .try_send(spawn_effect_to_testkit)
            .expect("could not send the effect to the testkit");

        let _ = self
            .dependency
            .spawn_effect_receiver
            .next()
            .await
            .expect("could not receive the effect back from the testkit");

        ActorToSpawn::new(task, actor_ref)
    }
}
