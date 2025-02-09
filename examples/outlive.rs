use std::time::Duration;
use tracing::Instrument;

use actum::prelude::*;

async fn parent<AB>(mut cell: AB, _me: ActorRef<()>) -> (AB, ())
where
    AB: ActorBounds<()>,
{
    let child = cell.spawn(child).await.unwrap_left();
    let span = tracing::info_span!("child");
    tokio::spawn(child.task.run_task().instrument(span));

    (cell, ()) // the child is sleeping; try to return immediately and see what happens.
}

async fn child<AB>(cell: AB, _me: ActorRef<()>) -> (AB, ())
where
    AB: ActorBounds<()>,
{
    tokio::time::sleep(Duration::from_secs(1)).await;

    (cell, ())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let parent = actum(parent);
    let span = tracing::info_span!("parent");
    parent.task.run_task().instrument(span).await;
}
