use crate::core::receive_message::ReceiveMessage;
use crate::core::receive_message::Recv;
use futures::future::Either;
use futures::pin_mut;

pub enum Command<RegistrantType> {
    Register { who: RegistrantType },
    Unregister { who: RegistrantType },
}

/// For registering actors which are interested in receiving messages from remote sources and
/// unregistering those which are no more interested.
pub trait Registrar<RegistrantType> {
    fn register(&mut self, who: RegistrantType);
    fn unregister(&mut self, who: RegistrantType);
}

async fn actum_remoting<RegistrantType, LetterType, RegistrarType, DispatchLetterType, R>(
    mut receiver: R,
    mut registrar: RegistrarType,
    mut receive_letter: impl AsyncFnMut() -> LetterType,
    mut dispatch_letter: DispatchLetterType,
) where
    RegistrantType: Send + 'static,
    LetterType: Send + 'static,
    RegistrarType: Registrar<RegistrantType>,
    DispatchLetterType: FnMut(&mut RegistrarType, LetterType),
    R: ReceiveMessage<Command<RegistrantType>>,
{
    'outer: loop {
        let letter_future = receive_letter();
        pin_mut!(letter_future);

        'inner: loop {
            let select = futures::future::select(receiver.recv(), &mut letter_future).await;

            match select {
                Either::Left((Recv::Message(command), ..)) => {
                    match command {
                        Command::Register { who } => {
                            registrar.register(who);
                        }
                        Command::Unregister { who } => {
                            registrar.unregister(who);
                        }
                    }
                    //
                }
                Either::Left((Recv::NoMoreSenders, ..)) => {
                    break 'outer;
                }
                Either::Right((letter, ..)) => {
                    dispatch_letter(&mut registrar, letter);

                    break 'inner;
                }
            }
        }
    }
}

pub mod kv {
    use super::*;
    use crate::prelude::MappedActorRef;
    use std::collections::HashMap;
    use std::fmt::Debug;
    use std::hash::Hash;

    pub struct Letter<ActorIdentifier, MessageTypes> {
        pub(crate) recipient: ActorIdentifier,
        pub(crate) message: MessageTypes,
    }

    pub struct IdentifiedMappedActorRef<ActorIdentifier, MessageTypes> {
        pub id: ActorIdentifier,
        pub mapped_actor_ref: MappedActorRef<MessageTypes>,
    }

    impl<ActorIdentifier, MessageTypes> Registrar<IdentifiedMappedActorRef<ActorIdentifier, MessageTypes>>
        for HashMap<ActorIdentifier, MappedActorRef<MessageTypes>>
    where
        ActorIdentifier: Eq + Hash,
    {
        fn register(&mut self, who: IdentifiedMappedActorRef<ActorIdentifier, MessageTypes>) {
            self.insert(who.id, who.mapped_actor_ref);
        }
        fn unregister(&mut self, who: IdentifiedMappedActorRef<ActorIdentifier, MessageTypes>) {
            self.remove(&who.id);
        }
    }

    pub async fn actum_remoting_kv<ActorIdentifier, MessageTypes, R>(
        receiver: R,
        registrar: HashMap<ActorIdentifier, MappedActorRef<MessageTypes>>,
        receive_letter: impl AsyncFnMut() -> Letter<ActorIdentifier, MessageTypes>,
    ) where
        ActorIdentifier: Debug + Eq + Hash + Send + 'static,
        MessageTypes: Send + 'static,
        R: ReceiveMessage<Command<IdentifiedMappedActorRef<ActorIdentifier, MessageTypes>>>,
    {
        actum_remoting(receiver, registrar, receive_letter, |registrar, letter| {
            let Some(mapped_actor_ref) = registrar.get_mut(&letter.recipient) else {
                tracing::warn!("cannot dispatch message to recipient = {:?}", letter.recipient);
                return;
            };
            let _ = mapped_actor_ref.try_send(letter.message);
        })
        .await;
    }
}

#[cfg(test)]
mod remoting_tests {
    use super::*;
    use crate::prelude::*;
    use tokio::net::UdpSocket;

    type MyLetterType = String;

    type MyRegistrantType = ActorRef<String>;

    struct MyRegistrarType {
        registered: Option<ActorRef<String>>,
    }

    impl Registrar<ActorRef<String>> for MyRegistrarType {
        fn register(&mut self, _: ActorRef<String>) {
            unreachable!()
        }

        fn unregister(&mut self, _: ActorRef<String>) {
            unreachable!()
        }
    }

    fn my_dispatch_letter(registrar: &mut MyRegistrarType, letter: String) {
        if let Some(registered) = &mut registrar.registered {
            let _ = registered.try_send(letter);
        }
    }

    #[tokio::test]
    async fn test_it() {
        let sock = UdpSocket::bind("0.0.0.0:8080").await.unwrap();
        let mut buf = [0; 1024];
        let my_receive_letter = async move || {
            let (len, ..) = sock.recv_from(&mut buf).await.unwrap();
            String::from_utf8_lossy(&buf[..len]).to_string()
        };

        // echo -n "hello, world!" >/dev/udp/0.0.0.0/8080
        // echo -n "what's up?" >/dev/udp/0.0.0.0/8080
        // echo -n "stop" >/dev/udp/0.0.0.0/8080

        let CreateActorResult { task, .. } = actum::<(), _, _, ()>(|mut cell, receiver, me| async move {
            drop(receiver);
            drop(me);

            // instantiate the actor which uses the remoting service
            let CreateActorResult {
                task,
                actor_ref: registrant,
            } = cell
                .create_child::<String, _, _, ()>(|cell, mut receiver, me| async move {
                    drop(me);
                    while let Recv::Message(m) = receiver.recv().await {
                        println!("child: received {}", m);
                        if m == "stop" {
                            println!("stopping");
                            break;
                        }
                    }

                    (cell, ())
                })
                .await;
            let registrant_handle = tokio::spawn(task.run_task());

            // instantiate the actum remoting service
            let CreateActorResult {
                task,
                actor_ref: receptionist_ref,
            } = cell
                .create_child::<Command<MyRegistrantType>, _, _, ()>(|cell, receiver, me| async move {
                    drop(me);

                    // preload the registrar
                    let registrar = MyRegistrarType {
                        registered: Some(registrant),
                    };

                    actum_remoting::<MyRegistrantType, MyLetterType, MyRegistrarType, _, _>(
                        receiver,
                        registrar,
                        my_receive_letter,
                        my_dispatch_letter,
                    )
                    .await;

                    (cell, ())
                })
                .await;
            let receptionist_handle = tokio::spawn(task.run_task());

            // to prevent the remoting service actor from returning due to "no more senders",
            // we must keep a reference in scope up until the users of the service are done with it.
            registrant_handle.await.unwrap();
            drop(receptionist_ref);

            (cell, ())
        });

        task.run_task().await;
    }
}

#[cfg(test)]
mod remoting_kv_tests {
    use super::*;
    use crate::prelude::*;
    use crate::remoting::kv::{IdentifiedMappedActorRef, Letter, actum_remoting_kv};
    use either::Either;
    use std::collections::HashMap;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_it() {
        let sock = UdpSocket::bind("0.0.0.0:8080").await.unwrap();
        let mut buf = [0; 1024];
        let my_receive_letter = async move || {
            let (len, ..) = sock.recv_from(&mut buf).await.unwrap();
            Letter::<u32, Either<String, u32>> {
                recipient: 0,
                message: Either::Left(String::from_utf8_lossy(&buf[..len]).to_string()),
            }
        };

        // echo -n "hello, world!" >/dev/udp/0.0.0.0/8080
        // echo -n "what's up?" >/dev/udp/0.0.0.0/8080
        // echo -n "stop" >/dev/udp/0.0.0.0/8080

        let CreateActorResult { task, .. } = actum::<(), _, _, ()>(|mut cell, receiver, me| async move {
            drop(receiver);
            drop(me);

            // instantiate the actum remoting service
            let CreateActorResult {
                task,
                actor_ref: mut receptionist_ref,
            } = cell
                .create_child::<_, _, _, ()>(|cell, receiver, me| async move {
                    drop(me);

                    let registrar = HashMap::new();

                    actum_remoting_kv::<u32, Either<String, u32>, _>(receiver, registrar, my_receive_letter).await;

                    (cell, ())
                })
                .await;
            tokio::spawn(task.run_task());

            // instantiate the actor which uses the remoting service
            let CreateActorResult { task, .. } = cell
                .create_child::<_, _, _, ()>(|cell, mut receiver, me| async move {
                    let _ = receptionist_ref.try_send(Command::Register {
                        who: IdentifiedMappedActorRef {
                            id: 0,
                            mapped_actor_ref: me.widen(Either::unwrap_left),
                        },
                    });

                    while let Recv::Message(m) = receiver.recv().await {
                        println!("child: received {}", m);
                        if m == "stop" {
                            break;
                        }
                    }

                    (cell, ())
                })
                .await;
            tokio::spawn(task.run_task());

            (cell, ())
        });

        task.run_task().await;
    }
}
