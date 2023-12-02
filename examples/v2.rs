use actum::prelude::*;
use std::collections::HashMap;

enum Command {
    Insert {
        key: u32,
        value: u32,
        reply_to: ActorRef<Option<u32>>,
    },
    Get {
        key: u32,
        reply_to: ActorRef<Option<u32>>,
    },
}

fn concurrent_hashmap() -> Receive<Command> {
    let mut hashmap = HashMap::new();
    receive(move |input| match input {
        ActorInput::Message { context, message } => {
            match message {
                Command::Insert { key, value, reply_to } => {
                    let previous = hashmap.insert(key, value);
                    reply_to.send(previous);
                }
                Command::Get { key, reply_to } => {
                    let current = hashmap.get(&key).copied();
                    reply_to.send(current);
                }
            };
            receive::Next::Same
        }
        ActorInput::Supervision { context, path, result } => receive::Next::Unhandled,
        ActorInput::PostStop { context } => receive::Next::Same,
    })
}

fn concurrent_hashmap2() -> Receive<Command> {
    let mut hashmap = HashMap::new();

    receive_message(move |context, message| {
        match message {
            Command::Insert { key, value, reply_to } => {
                let previous = hashmap.insert(key, value);
                reply_to.send(previous);
            }
            Command::Get { key, reply_to } => {
                let current = hashmap.get(&key).copied();
                reply_to.send(current);
            }
        }
        receive::Next::Same
    })
}

fn main() {}
