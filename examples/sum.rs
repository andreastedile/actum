use actum::prelude::*;
use tracing::{info, Level};

fn behavior1() -> receive::ReceiveBehavior<i32, (), i32> {
    let mut received = vec![];

    receive::receive_message(move |_, message| {
        received.push(*message);

        if *message > 0 {
            info!(
                "[behavior1] n: {}, received: {:?} -> Same",
                *message, received
            );
            receive::NextBehavior::Same
        } else {
            info!(
                "[behavior1] n: {}, received: {:?} -> behavior2",
                *message, received
            );
            // cannot move out of `received`, a captured variable in an `FnMut` closure
            behavior2(received.clone()).into()
        }
    })
}

fn behavior2(mut received: Vec<i32>) -> receive::ReceiveBehavior<i32, (), i32> {
    receive::receive_message(move |_, message| {
        received.push(*message);

        if *message > 0 {
            info!("[behavior2] n: {} -> behavior2({:?})", *message, received);
            // cannot move out of `received`, a captured variable in an `FnMut` closure
            behavior2(received.clone()).into()
        } else {
            info!("[behavior2] n: 0 -> Stopped");

            let sum = received.iter().sum();
            receive::stop(sum)
        }
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_target(false)
        .with_line_number(false)
        .with_max_level(Level::TRACE)
        .init();

    let system = ActorSystem::new("sum").unwrap();

    let sum = actum(system, behavior1(), |system, guardian| {
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
