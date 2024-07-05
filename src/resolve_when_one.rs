use futures::task::AtomicWaker;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

pub struct ResolveWhenOne(Arc<ResolveWhenOneInner>);

impl Default for ResolveWhenOne {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ResolveWhenOne {
    fn clone(&self) -> Self {
        let clone = Self(Arc::clone(&self.0));
        clone.0.count.fetch_add(1, Ordering::Relaxed);
        clone
    }
}

impl ResolveWhenOne {
    pub fn new() -> Self {
        Self(Arc::new(ResolveWhenOneInner {
            waker: AtomicWaker::new(),
            count: AtomicUsize::new(1),
        }))
    }
}

struct ResolveWhenOneInner {
    waker: AtomicWaker,
    count: AtomicUsize,
}

impl Drop for ResolveWhenOne {
    fn drop(&mut self) {
        let previous = self.0.count.fetch_sub(1, Ordering::Relaxed);
        let remaining = previous - 1;
        if remaining == 1 {
            // Wake the remaining instance.
            self.0.waker.wake();
        }
    }
}

impl Future for ResolveWhenOne {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let current = self.0.count.load(Ordering::Relaxed);
        if current == 1 {
            Poll::Ready(())
        } else {
            self.0.waker.register(cx.waker());

            let current = self.0.count.load(Ordering::Relaxed);
            if current == 1 {
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
        let r1 = ResolveWhenOne::new();
        r1.await;
    }

    #[tokio::test]
    async fn test_2() {
        let r1 = ResolveWhenOne::new();
        let r2 = r1.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(r2);
        });

        r1.await;
    }

    #[tokio::test]
    async fn test_3() {
        let r1 = ResolveWhenOne::new();
        let r2 = r1.clone();
        let r3 = r1.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(r2);
        });

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            drop(r3);
        });

        r1.await;
    }
}
