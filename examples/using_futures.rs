use actum::prelude::*;
use std::time::Duration;
use tokio::time::sleep;

async fn infinite(me: ActorRef<String>) -> String {
    let mut counter = 0;
    loop {
        sleep(Duration::from_millis(500)).await;
        me.send(format!("Hello {}!", counter));
        counter += 1;
    }
}

fn perpeptual_hello(mut remaining: usize) -> Setup<String> {
    setup(move |context| {
        println!("remaining = {}", remaining);
        if remaining == 0 {
            stop_with_output(()).into()
        } else {
            context.pipe_to_self({
                let me = context.me.clone();

                infinite(me)

                // let mut counter = 0;
                // async move {
                //     loop {
                //         sleep(Duration::from_millis(500)).await;
                //         me.send(format!("Hello {}!", counter));
                //         counter += 1;
                //     }
                // }
            });
            receive_message(move |context, message| {
                remaining -= 1;

                println!("Received: {}, remaining: {}", message, remaining);

                if remaining == 0 {
                    stop_with_output(()).into()
                } else {
                    receive::Next::Same
                }
            })
            .into()
        }
    })
    // receive_message(move |_context, message| {
    //     count += 1;
    //
    //     if count >= cap {
    //         stop_with_output(());
    //     }
    //
    //     if message > 0 {
    //         receive::Next::Same
    //     } else {
    // behavior2(std::mem::take(&mut received)).into()
    // }
    // })
}

fn main() {
    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    actum(system, perpeptual_hello(5), |guardian| {
        // behavior1
        // guardian.send(3);
        // guardian.send(2);
        // guardian.send(1);
        // guardian.send(0);
        // behavior2
        // guardian.send(3);
        // guardian.send(2);
        // guardian.send(1);
        // guardian.send(0);
    })
    .unwrap()
    .unwrap();
}
