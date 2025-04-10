use actum::prelude::*;

async fn root_v2<A, R>(cell: A, mut receiver: R, _me: ActorRef<u64>) -> (A, u32)
where
    A: Actor<u64, String>,
    //            ^^^^^^ it complies, but it should not: String != u32
    R: Receiver<u64>,
{
    let _ = receiver.recv().await;
    (cell, 4)
}

#[tokio::main]
async fn main() {
    let _root = actum(root_v2);
}
