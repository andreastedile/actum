use futures::task::AtomicWaker;
use std::future::{Future, poll_fn};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

#[derive(Default)]
pub(crate) struct ChildrenTracker {
    inner: Option<Arc<ChildrenCountWithParentWaker>>,
}

impl ChildrenTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_children(&self) -> bool {
        self.children_count() > 0
    }

    pub fn children_count(&self) -> usize {
        if let Some(counter) = &self.inner {
            counter.children_count.load(Ordering::Relaxed)
        } else {
            0
        }
    }

    pub fn make_child(&mut self) -> WakeParentOnDrop {
        if let Some(state) = self.inner.as_mut() {
            state.children_count.fetch_add(1, Ordering::Relaxed);
            WakeParentOnDrop {
                inner: Arc::clone(state),
            }
        } else {
            // This is the first child.
            let state = self.inner.get_or_insert(Arc::new(ChildrenCountWithParentWaker {
                parent_waker: AtomicWaker::new(),
                children_count: AtomicUsize::new(1),
            }));
            WakeParentOnDrop {
                inner: Arc::clone(state),
            }
        }
    }

    #[allow(dead_code)]
    pub fn join_all(&mut self) -> impl Future<Output = ()> + '_ {
        poll_fn(|cx| self.poll(cx))
    }

    pub fn poll(&mut self, cx: &mut Context) -> Poll<()> {
        let Some(state) = self.inner.as_ref() else {
            return Poll::Ready(());
        };
        let current = state.children_count.load(Ordering::Relaxed);
        if current == 0 {
            self.inner = None;
            Poll::Ready(())
        } else {
            state.parent_waker.register(cx.waker());

            let current = state.children_count.load(Ordering::Relaxed);
            if current == 0 {
                self.inner = None;
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}

pub(crate) struct WakeParentOnDrop {
    inner: Arc<ChildrenCountWithParentWaker>,
}

impl Drop for WakeParentOnDrop {
    fn drop(&mut self) {
        let previous = self.inner.children_count.fetch_sub(1, Ordering::Relaxed);
        let remaining = previous - 1;
        if remaining == 0 {
            // Wake the parent.
            self.inner.parent_waker.wake();
        }
    }
}

struct ChildrenCountWithParentWaker {
    children_count: AtomicUsize,
    parent_waker: AtomicWaker,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_1() {
        let mut parent = ChildrenTracker::default();
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
