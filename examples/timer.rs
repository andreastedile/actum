use std::time::Duration;
use tracing::Instrument;

use actum::prelude::*;

async fn an_actor<AB>(mut cell: AB, _me: ActorRef<u32>) -> (AB, ())
where
    AB: Actor<u32>,
{
    tokio::select! {
        Recv::Message(m) = cell.recv() => {
            tracing::info!(m);
        }
        _ = tokio::time::sleep(Duration::from_secs(1)) => {
            tracing::info!("timeout");
        }
    }

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

    let root = actum(an_actor);
    let span = tracing::trace_span!("root");
    let handle = tokio::spawn(root.task.run_task().instrument(span));

    // let _ = root.m_ref.try_send(1);

    handle.await.unwrap();
}
