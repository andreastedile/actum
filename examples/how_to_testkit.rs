use actum::prelude::*;
use tracing::Instrument;

async fn generic_parent<AB>(mut cell: AB, me: ActorRef<u64>) -> (AB, ())
where
    AB: ActorBounds<u64>,
{
    let m1 = cell.recv().await.message().unwrap();
    tracing::info!(recv = m1);

    let parent = me.clone();
    let child = cell
        .create_child(move |cell, me| async move { generic_child(cell, me, parent, m1).await })
        .await
        .unwrap_left();
    let span = tracing::trace_span!("child");
    tokio::spawn(child.task.run_task().instrument(span));

    let m2 = cell.recv().await.message().unwrap();
    tracing::info!(recv = m2);

    assert_eq!(m2, m1 * 2);

    (cell, ())
}

async fn generic_child<AB>(mut cell: AB, mut me: ActorRef<u64>, mut parent: ActorRef<u64>, m: u64) -> (AB, ())
where
    AB: ActorBounds<u64>,
{
    tracing::info!(try_send = m * 2);
    me.try_send(m * 2).unwrap();

    let m = cell.recv().await.message().unwrap();
    tracing::info!(recv = m);

    tracing::info!(try_send = m);
    parent.try_send(m).unwrap();

    (cell, ())
}

#[tokio::test]
async fn test() {
    use actum::testkit::testkit;

    tracing_subscriber::fmt()
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let (mut parent, mut parent_testkit) = testkit(generic_parent);
    let span = tracing::trace_span!("parent");
    let handle = tokio::spawn(parent.task.run_task().instrument(span));

    parent.m_ref.try_send(1).unwrap();

    let _ = parent_testkit
        .test_next_effect(async |effect| {
            let effect = effect.unwrap();
            let recv = effect.unwrap_recv();
            let m = recv.recv.unwrap_message();
            assert_eq!(*m, 1);
        })
        .await;

    let mut child_testkit = parent_testkit
        .test_next_effect(async |effect| {
            let effect = effect.unwrap();
            let spawn = effect.unwrap_spawn();
            let testkit = spawn.testkit_or_message.unwrap_left().downcast_unwrap::<u64>();
            testkit
        })
        .await;

    let _ = child_testkit
        .test_next_effect(async |effect| {
            let effect = effect.unwrap();
            let recv = effect.unwrap_recv();
            let m = recv.recv.unwrap_message();
            assert_eq!(*m, 2);
        })
        .await;

    let _ = parent_testkit
        .test_next_effect(async |effect| {
            let effect = effect.unwrap();
            let recv = effect.unwrap_recv();
            let m = recv.recv.unwrap_message();
            assert_eq!(*m, 2);
        })
        .await;

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

    parent.m_ref.try_send(1).unwrap();

    handle.await.unwrap();
}
