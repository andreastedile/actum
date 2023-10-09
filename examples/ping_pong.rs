use actum::prelude::*;
use tracing::info;

#[derive(Debug, Clone)]
struct Ping {
    reply_to: ActorRef<Pong>,
}

#[derive(Debug, Clone)]
struct Pong;

fn pinger(who: ActorRef<Ping>, cap: usize) -> setup::SetupBehavior<Pong, (), Option<usize>> {
    setup::setup(move |context| {
        let who = who.clone();

        let mut n_replies = 0;

        info!("Pinging {:?}", who);
        who.send(Ping {
            reply_to: context.me().clone(),
        });

        receive::receive_message(move |context, _| {
            n_replies += 1;

            if n_replies < cap {
                info!("Pinging {:?}", who);
                who.send(Ping {
                    reply_to: context.me().clone(),
                });

                receive::NextBehavior::Same
            } else {
                info!("Stopping");
                receive::stop_and(move |_| {
                    info!("Stopped");
                    Some(n_replies)
                })
            }
        })
        .into()
    })
}

fn ponger() -> receive::ReceiveBehavior<Ping, (), Option<usize>> {
    receive::receive(|input: ActorInput<Ping>| match input {
        ActorInput::Message { message, .. } => {
            info!("Ponging {:?}", message.reply_to);
            message.reply_to.send(Pong);

            receive::NextBehavior::Same
        }
        ActorInput::Stopped { .. } => receive::NextBehavior::Unhandled,
        ActorInput::PostStop { .. } => {
            info!("Stopped by ancestor actor");
            receive::stop(None)
        }
    })
}

fn ping_pong(cap: usize) -> setup::SetupBehavior<(), Option<usize>> {
    setup::setup(move |context| {
        let ponger = context.spawn("ponger", ponger());
        context.watch(&ponger);

        let pinger = context.spawn("pinger", pinger(ponger, cap));
        context.watch(&pinger);

        receive::receive_stopped(move |_context, path, result| {
            assert_eq!(path, &pinger.actor_path);
            info!("Pinger child actor stopped with output: {:?}", result);

            receive::stop(())
        })
        .into()
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let system = ActorSystem::new("ping pong").unwrap();

    actum(system, ping_pong(3), |_, _| {
        info!("Ping pong app");
    })
    .unwrap();
}
