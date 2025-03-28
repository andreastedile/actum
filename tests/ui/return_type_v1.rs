use actum::prelude::*;

#[tokio::main]
async fn main() {
    let _root = actum::<u64, _, _, u32>(|mut cell, mut receiver, _me| async move {
        let _ = cell.recv(&mut receiver).await;
        (cell, 4)
    });
}
