use actum::prelude::*;
use tracing::info;

#[derive(Debug)]
pub struct Stop;

pub fn child() -> Receive<u32, u32> {
    let mut total = 0;

    receive_message(move |context, message| {
        total += 1;
        if message > 0 {
            receive::Next::Same
        } else {
            stop_with_output(total).into()
        }
    })
}

pub fn parent() -> Setup<(), (), u32> {
    setup(move |context| {
        let child = context.spawn("child1", child()).unwrap();
        context.watch(&child);

        child.send(1);
        child.send(2);
        child.send(3);
        child.send(0);

        receive_supervision(move |context, path, result| {
            //
            match result.0 {
                Some(Ok(output)) => {
                    info!("Child output is: {output}");
                    receive::Next::Empty
                }
                Some(Err(error)) => receive::Next::Unhandled,
                None => receive::Next::Unhandled,
            }
        })
        .into()
    })
}

fn main() {
    tracing_subscriber::fmt()
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let system = ActorSystem::new("example", ActorSystemConfig::default()).unwrap();
    actum(system, parent(), |_| {}).0.unwrap().unwrap();
}
