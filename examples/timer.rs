use actum::prelude::*;
use std::time::Duration;
use tracing::Instrument;

async fn an_actor<C, R>(cell: C, mut receiver: R, _me: ActorRef<u32>) -> (C, ())
where
    C: CreateChild,
    R: ReceiveMessage<u32>,
{
    tokio::select! {
        Recv::Message(m) = receiver.recv() => {
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

    // let _ = root.actor_ref.try_send(1);

    handle.await.unwrap();
}
