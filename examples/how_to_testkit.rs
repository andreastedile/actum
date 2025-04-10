use actum::prelude::*;
use tracing::Instrument;

async fn generic_parent<A, R>(mut cell: A, mut receiver: R, _me: ActorRef<u64>) -> (A, ())
where
    A: Actor<u64, ()>,
    R: ReceiveMessage<u64>,
{
    let m = receiver.recv().await.into_message().unwrap();
    tracing::info!(recv = m);

    let child = cell
        .create_child(move |cell, receiver, me| async move { generic_child(cell, receiver, me, m).await })
        .await;
    let span = tracing::trace_span!("child");
    let m_times_two = tokio::spawn(child.task.run_task().instrument(span)).await.unwrap();

    assert_eq!(m_times_two, m * 2);

    (cell, ())
}

async fn generic_child<A, R>(cell: A, mut receiver: R, mut me: ActorRef<u64>, m: u64) -> (A, u64)
where
    A: Actor<u64, u64>,
    R: ReceiveMessage<u64>,
{
    tracing::info!(try_send = m * 2);
    me.try_send(m * 2).unwrap();

    let m_times_two = receiver.recv().await.into_message().unwrap();
    tracing::info!(recv = m_times_two);

    (cell, m_times_two)
}

#[tokio::test]
async fn test() {
    use actum::testkit::actum_with_testkit;
    use actum::testkit::ActumWithTestkit;

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
        mut actor_ref,
        testkit: mut parent_tk,
    } = actum_with_testkit(generic_parent);
    let span = tracing::trace_span!("parent");
    let handle = tokio::spawn(task.run_task().instrument(span));

    actor_ref.try_send(1).unwrap();

    let _ = parent_tk
        .expect_recv_effect(async |effect| {
            let m = effect.recv.as_ref().into_message().unwrap();
            assert_eq!(*m, 1);
        })
        .await;

    let mut child_tk = parent_tk
        .expect_spawn_effect(async |effect| {
            let effect = effect.downcast_unwrap::<u64, u64>();
            effect.testkit
        })
        .await;

    let _ = child_tk
        .expect_recv_effect(async |effect| {
            let m = effect.recv.as_ref().into_message().unwrap();
            assert_eq!(*m, 2);
        })
        .await;

    let _ = child_tk
        .expect_returned_effect(async |effect| {
            assert_eq!(*effect.ret, 2);
        })
        .await;

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

    let mut parent = actum(generic_parent);
    let span = tracing::trace_span!("parent");
    let handle = tokio::spawn(parent.task.run_task().instrument(span));

    parent.actor_ref.try_send(1).unwrap();

    handle.await.unwrap();
}
