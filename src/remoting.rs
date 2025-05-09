use crate::core::actor_ref::MappedActorRef;
use crate::prelude::{ReceiveMessage, Recv};
use futures::future::Either;
use futures::pin_mut;
use std::collections::HashMap;

pub enum Command<K, MessageTypes> {
    Register {
        who: K,
        actor_ref: MappedActorRef<MessageTypes>,
    },
    Unregister {
        id: K,
    },
}

struct Letter<K, MessageTypes> {
    id: K,
    message: MessageTypes,
}

async fn actum_remoting<K, MessageTypes, R>(
    mut receiver: R,
    mut receive_letter: impl AsyncFnMut() -> Letter<K, MessageTypes>,
) where
    K: Eq + std::hash::Hash + Send + 'static,
    MessageTypes: Send + 'static,
    R: ReceiveMessage<Command<K, MessageTypes>>,
{
    let mut actor_map = HashMap::<K, MappedActorRef<MessageTypes>>::new();

    'outer: loop {
        let letter_future = receive_letter();
        pin_mut!(letter_future);

        'inner: loop {
            let select = futures::future::select(receiver.recv(), &mut letter_future).await;

            match select {
                Either::Left((Recv::Message(command), ..)) => {
                    match command {
                        Command::Register {
                            who: id,
                            actor_ref: who,
                        } => {
                            actor_map.insert(id, who);
                        }
                        Command::Unregister { id } => {
                            actor_map.remove(&id);
                        }
                    }
                    //
                }
                Either::Left((Recv::NoMoreSenders, ..)) => {
                    break 'outer;
                }
                Either::Right((letter, ..)) => {
                    if let Some(actor_ref) = actor_map.get_mut(&letter.id) {
                        let _ = actor_ref.try_send(letter.message);
                    } else {
                        // Todo: is the message lost? Should we resend it?
                    }

                    break 'inner;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use either::Either;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_it() {
        let sock = UdpSocket::bind("0.0.0.0:8080").await.unwrap();
        let mut buf = [0; 1024];
        let f = async move || {
            let (len, ..) = sock.recv_from(&mut buf).await.unwrap();
            Letter::<u32, Either<String, u32>> {
                id: 0,
                message: Either::Left(String::from_utf8_lossy(&buf[..len]).to_string()),
            }
        };

        // echo -n "hello, world!" >/dev/udp/0.0.0.0/8080
        // echo -n "what's up?" >/dev/udp/0.0.0.0/8080

        let CreateActorResult { task, .. } = actum::<(), _, _, ()>(|mut cell, receiver, me| async move {
            drop(receiver);
            drop(me);

            let CreateActorResult {
                task,
                actor_ref: mut receptionist_ref,
            } = cell
                .create_child::<Command<u32, Either<String, u32>>, _, _, ()>(|cell, receiver, me| async move {
                    drop(me);

                    actum_remoting(receiver, f).await;

                    (cell, ())
                })
                .await;
            tokio::spawn(task.run_task());

            let CreateActorResult { task, .. } = cell
                .create_child::<String, _, _, ()>(|cell, mut receiver, me| async move {
                    let _ = receptionist_ref.try_send(Command::Register {
                        who: 0,
                        actor_ref: me.widen(Either::unwrap_left),
                    });

                    while let Recv::Message(m) = receiver.recv().await {
                        println!("child: received {}", m);
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
