// TRACE parent: 41: new
// TRACE parent:child: 11: new
// TRACE parent: 47: join children
// INFO parent:child: 23: sleeping for 1 second
// TRACE parent:child: 11: close time.busy=93.4µs time.idle=1.00s
// TRACE parent: 41: close time.busy=51.2µs time.idle=1.00s

use std::time::Duration;
use tracing::Instrument;

use actum::prelude::*;

async fn parent<A>(mut cell: A, _receiver: MessageReceiver<()>, _me: ActorRef<()>) -> (A, ())
where
    A: Actor<()>,
{
    let child = cell.create_child(child).await;
    let span = tracing::trace_span!("child");
    tokio::spawn(child.task.run_task().instrument(span));

    // we return immediately after having spawned the child actor.
    // even though the child actor is sleeping, it will not outlive us.
    (cell, ())
}

async fn child<A>(cell: A, _receiver: MessageReceiver<()>, _me: ActorRef<()>) -> (A, ())
where
    A: Actor<()>,
{
    tracing::info!("sleeping for 1 second");
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
    let span = tracing::trace_span!("parent");
    parent.task.run_task().instrument(span).await;
}
