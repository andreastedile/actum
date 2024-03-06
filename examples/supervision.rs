use actum::prelude::*;
use log::info;

async fn parent(mut cell: ActorCell<()>) {
    info!("Spawn child");
    cell.spawn("child", child);

    if let Some(ActorInput::Supervision { path, panic }) = (&mut cell).await {
        if let Some(reason) = panic {
            if let Some(reason) = reason.downcast_ref::<&str>() {
                info!("{:?} stopped with reason {:?}", path, reason);
            }
        }
    }
}

async fn child(_cell: ActorCell<()>) {
    panic!("Surprise");
}

fn main() {
    tracing_subscriber::fmt()
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let result = actum("testing", parent);
    println!("{:?}", result);
}
