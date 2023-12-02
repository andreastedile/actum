use actum::prelude::*;
use std::error::Error;
use tracing::info;

fn child1() -> Setup<()> {
    setup(move |context| None.unwrap())
}

fn child2(dependency: ActorRef<()>) -> Setup<()> {
    setup(move |context| {
        context.watch(&dependency);
        receive_supervision(move |context, path, result| {
            info!("{path:?} failed");
            receive::Next::Same
        }).into()
    })
}

fn parent() -> Setup<()> {
    setup(move |context| {
        let child1 = context.spawn("child1", child1()).unwrap();
        context.watch(&child1);

        context.spawn("child2", child2(child1.clone())).unwrap();

        receive_supervision(move |context, path, result| {
            if let Some(Err(ActorError::Panic(cause))) = result.0 {
                match cause.downcast_ref::<&'static str>() {
                    Some(as_str) => info!("Child {path:?} failed: {as_str}"),
                    None => info!("Child {path:?} failed"),
                }
            }
            receive::Next::Same
        }).into()
    })
}

fn main() {
    tracing_subscriber::fmt()
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    let output = actum(system, parent(), |guardian| {
        // nothing
    });
}
