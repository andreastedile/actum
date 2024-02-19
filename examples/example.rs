use actum::prelude::*;
use tracing::info;

struct ParentMessage {
    n: u32,
}

struct ChildMessage {
    n: u32,
    reply_to: ActorRef<ParentMessage>,
}

async fn child(mut cell: ActorCell<ChildMessage>) {
    while let Some(ActorInput::Message(message)) = (&mut cell).await {
        info!("Received message from {:?}", message.reply_to.path);

        message.reply_to.send(ParentMessage { n: message.n * 2 });
    }

    info!("Stopped by parent");

    assert!((&mut cell).await.is_none());
}

async fn guardian(mut cell: ActorCell<ParentMessage>) {
    let child = cell.spawn("child", child);

    info!("Send 1 to child");

    child.send(ChildMessage {
        n: 1,
        reply_to: cell.me().clone(),
    });

    if let Some(ActorInput::Message(message)) = (&mut cell).await {
        info!("Received {} from child", message.n);
    }

    info!("Stopping child");

    cell.stop(&child.path);

    if let Some(ActorInput::Supervision { path, panic }) = (&mut cell).await {
        info!(?path, ?panic, "Supervise");
    }

    info!("Done");
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

    let result = actum("testing", guardian);
    println!("{:?}", result);
}
