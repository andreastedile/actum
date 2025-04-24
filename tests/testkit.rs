use actum::prelude::*;
use futures::FutureExt;
use std::future::poll_fn;
use std::task::Poll;
use std::time::Duration;
use tracing::{Instrument, info_span};

/// Non-cloneable type.
/// If the actor_cell receives it, it certainly could not have been cloned by Actum.
struct NonClone;

#[tokio::test]
async fn test_slow_testkit() {
    let _ = tracing_subscriber::fmt()
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .try_init();

    let ActumWithTestkit {
        task,
        mut actor_ref,
        mut testkit,
    } = actum_with_testkit::<NonClone, _, _, ()>(|cell, mut receiver, _| async move {
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let mut recv_future = receiver.recv();
        poll_fn(|cx| match recv_future.poll_unpin(cx) {
            Poll::Ready(_) => panic!("the testkit should be slow"),
            Poll::Pending => Poll::Ready(()),
        })
        .await;
        drop(recv_future);

        let _ = receiver.recv().await.into_message().unwrap();
        tracing::info!("received NonClone");

        (cell, ())
    });

    let root_handle = tokio::spawn(task.run_task().instrument(info_span!("root")));

    // Immediately send the NonClone.
    assert!(actor_ref.try_send(NonClone).is_ok());

    let _ = testkit
        .expect_recv_effect(async |_| {
            tracing::info!("effect received; sleeping");
            tokio::time::sleep(Duration::from_millis(1000)).await;
        })
        .instrument(info_span!("testkit"))
        .await;

    let _ = testkit
        .expect_recv_effect(async |_| {
            tracing::info!("effect received");
        })
        .instrument(info_span!("testkit"))
        .await;

    let _ = testkit
        .expect_returned_effect(async |_| {})
        .instrument(info_span!("testkit"))
        .await;

    root_handle.await.unwrap();
}

#[tokio::test]
async fn test_recv_effect_discard() {
    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .try_init();

    let ActumWithTestkit {
        task,
        mut actor_ref,
        mut testkit,
    } = actum_with_testkit::<u32, _, _, ()>(|cell, mut receiver, _| async move {
        let m = receiver.recv().await.into_message().unwrap();
        assert_eq!(m, 2);
        tracing::info!("received 2");

        (cell, ())
    });

    let root_handle = tokio::spawn(task.run_task().instrument(info_span!("root")));

    // Send two messages and discard the first. Only the second can be received.

    tracing::info!("sending 1 to actor_cell");
    let _ = actor_ref.try_send(1);

    tracing::info!("sending 2 to actor_cell");
    let _ = actor_ref.try_send(2);

    let _ = testkit
        .expect_recv_effect(async |mut effect| {
            let m = effect.recv.as_ref().into_message().unwrap();
            assert_eq!(*m, 1);
            tracing::info!("discarding 1");
            effect.discard();
        })
        .instrument(info_span!("testkit"))
        .await;

    let _ = testkit
        .expect_recv_effect(async |effect| {
            let m = effect.recv.as_ref().into_message().unwrap();
            assert_eq!(*m, 2);
        })
        .instrument(info_span!("testkit"))
        .await;

    let _ = testkit
        .expect_returned_effect(async |_| {})
        .instrument(info_span!("testkit"))
        .await;

    root_handle.await.unwrap();
}
