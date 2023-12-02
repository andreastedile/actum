use actum::prelude::*;

enum S1 { O1(O1), O2(O2) }
struct O1 {}
struct O2 {}
impl Into<S1> for O1 { fn into(self) -> S1 { S1::O1(self) } }

fn child1() -> Receive<(), O1> { todo!() }
fn child2() -> Receive<(), O2> { todo!() }

fn parent() -> Setup<(), (), S1> {
    setup(move |context| {
        let child1 = context.spawn("child1", child1()).unwrap();
        let child2 = context.spawn("child2", child2()).unwrap();
        receive_supervision(move |context, path, result| {
            match result.0 {
                Some(Ok(S1::O1(o1))) => {}
                Some(Ok(S1::O2(o2))) => {}
                Some(Err(ActorError::UnhandledSupervision)) => {}
                Some(Err(ActorError::Panic(_))) => {}
                None => {}
            }
            receive::Next::Empty
        }).into()
    })
}

fn main() {}
