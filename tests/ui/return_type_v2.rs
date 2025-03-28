use actum::prelude::*;

async fn root_v2<A>(mut cell: A, mut receiver: MessageReceiver<u64>, _me: ActorRef<u64>) -> (A, u32)
where
    A: Actor<u64, String>,
    //            ^^^^^^ it complies, but it should not: String != u32
{
    let _ = cell.recv(&mut receiver).await;
    (cell, 4)
}

#[tokio::main]
async fn main() {
    let _root = actum(root_v2);
}
