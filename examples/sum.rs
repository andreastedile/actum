use actum::prelude::*;

fn behavior1() -> Receive<i32, i32> {
    let mut received = vec![];

    receive_message(move |_context, message| {
        received.push(message);

        if message > 0 {
            receive::Next::Same
        } else {
            // cannot move out of `received`, a captured variable in an `FnMut` closure
            behavior2(std::mem::take(&mut received)).into()
        }
    })
}

fn behavior2(mut received: Vec<i32>) -> Receive<i32, i32> {
    receive_message(move |_context, message| {
        received.push(message);

        if message > 0 {
            // cannot move out of `received`, a captured variable in an `FnMut` closure
            behavior2(std::mem::take(&mut received)).into()
        } else {
            let sum = received.iter().sum();

            // stop_with_output(received.iter().sum()).into()
            stop_with_callback(move |_context| sum).into()
        }
    })
}

fn main() {
    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    let sum = actum(system, behavior1(), |guardian| {
        // behavior1
        guardian.send(3);
        guardian.send(2);
        guardian.send(1);
        guardian.send(0);
        // behavior2
        guardian.send(3);
        guardian.send(2);
        guardian.send(1);
        guardian.send(0);
    })
    .unwrap()
    .unwrap();
    assert_eq!(sum, 3 + 2 + 1 + 3 + 2 + 1);
}
