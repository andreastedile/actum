use actum::prelude::*;
use tracing::info;

#[derive(Debug)]
pub struct Stop;

pub fn child() -> receive::ReceiveBehavior<Stop, (), u32> {
    receive::receive_message(|context, stop| {
        info!("Stopping");
        receive::stop(42)
    })
}

pub fn parent() -> setup::SetupBehavior<Stop, u32> {
    setup::setup(move |context| {
        //
        let child = context.spawn("child1", child());

        context.watch(&child);

        receive::receive_message(move |context, stop| {
            info!("Stopping child1");
            child.send(Stop);

            receive::receive(move |input| match input {
                ActorInput::Message { .. } => receive::NextBehavior::Unhandled,
                ActorInput::Stopped { actor_result, .. } => {
                    assert_eq!(*actor_result.unwrap().unwrap().unwrap(), 42);
                    receive::stop(())
                }
                ActorInput::PostStop { .. } => receive::NextBehavior::Unhandled,
            })
            .into()
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

    let system = ActorSystem::new("simple_watch").unwrap();
    actum(system, parent(), |system, guardian| {
        guardian.send(Stop);
    })
    .unwrap();
}
