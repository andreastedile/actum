use futures::task::AtomicWaker;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Default)]
pub struct ChildrenTracker(Option<Arc<ChildrenState>>);

impl ChildrenTracker {
    pub fn make_child(&mut self) -> WakeParentOnDrop {
        if let Some(state) = self.0.as_mut() {
            state.children_count.fetch_add(1, Ordering::Relaxed);
            WakeParentOnDrop(Arc::clone(state))
        } else {
            // This is the first child.
            let state = self.0.get_or_insert(Arc::new(ChildrenState {
                waker: AtomicWaker::new(),
                children_count: AtomicUsize::new(1),
            }));
            WakeParentOnDrop(Arc::clone(state))
        }
    }

    pub fn join_all(self) -> impl Future<Output = ()> + Send + 'static {
        self.into_future()
    }
}

pub struct WakeParentOnDrop(Arc<ChildrenState>);

impl Drop for WakeParentOnDrop {
    fn drop(&mut self) {
        let previous = self.0.children_count.fetch_sub(1, Ordering::Relaxed);
        let remaining = previous - 1;
        if remaining == 0 {
            // Wake the parent.
            self.0.waker.wake();
        }
    }
}

struct ChildrenState {
    waker: AtomicWaker,
    children_count: AtomicUsize,
}

impl Future for ChildrenTracker {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(state) = self.0.as_ref() else {
            return Poll::Ready(());
        };
        let current = state.children_count.load(Ordering::Relaxed);
        if current == 0 {
            self.0 = None;
            Poll::Ready(())
        } else {
            state.waker.register(cx.waker());

            let current = state.children_count.load(Ordering::Relaxed);
            if current == 0 {
                self.0 = None;
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_1() {
        let parent = ChildrenTracker::default();
        parent.join_all().await;
    }

    #[tokio::test]
    async fn test_2() {
        let mut parent = ChildrenTracker::default();
        let child_1 = parent.make_child();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(child_1);
        });

        parent.join_all().await;
    }

    #[tokio::test]
    async fn test_3() {
        let mut parent = ChildrenTracker::default();
        let child_1 = parent.make_child();
        let child_2 = parent.make_child();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(child_1);
        });

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            drop(child_2);
        });

        parent.join_all().await;
    }
}
