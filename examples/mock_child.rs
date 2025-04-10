use actum::prelude::*;
use tracing::Instrument;

async fn generic_parent<C, R>(mut cell: C, _receiver: R, _me: ActorRef<u64>) -> (C, ())
where
    C: CreateChild,
    R: ReceiveMessage<u64>,
{
    let child = cell
        .create_child(move |cell, receiver, me| async move { generic_child(cell, receiver, me).await })
        .await;
    let span = tracing::trace_span!("child");
    let _child_handle = tokio::spawn(child.task.run_task().instrument(span));

    (cell, ())
}

async fn generic_child<C, R>(cell: C, _receiver: R, _me: ActorRef<u64>) -> (C, ())
where
    C: CreateChild,
    R: ReceiveMessage<u64>,
{
    tracing::info!("hello from child!");
    (cell, ())
}

#[tokio::test]
async fn test() {
    use futures::FutureExt;

    tracing_subscriber::fmt()
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let ActumWithTestkit {
        task,
        testkit: mut parent_tk,
        ..
    } = actum_with_testkit(generic_parent);
    let span = tracing::trace_span!("parent");
    let handle = tokio::spawn(task.run_task().instrument(span));

    let mut child_tk = parent_tk
        .expect_spawn_effect(async |effect| {
            let effect = effect.downcast_unwrap::<u64, ()>();
            let child_tk = effect.inject_actor(Box::new(|cell, _receiver, _me| {
                async {
                    tracing::info!("Hello from mock!");
                    (cell, ())
                }
                .boxed()
            }));
            child_tk
        })
        .await;

    let _ = child_tk.expect_returned_effect(async |_| {}).await;
    let _ = parent_tk.expect_returned_effect(async |_| {}).await;

    handle.await.unwrap();
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

    let parent = actum(generic_parent);
    let span = tracing::trace_span!("parent");
    let handle = tokio::spawn(parent.task.run_task().instrument(span));

    handle.await.unwrap();
}
