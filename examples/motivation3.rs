struct ActorContext;

trait Receive<M> {
    fn receive(&mut self, context: ActorContext, message: M);
}

enum MyState {
    S1(S1),
    S2(S2),
    S3(S3),
}
struct S1;
struct S2;
struct S3;

enum MyMessage {
    M1 {},
    M2 {},
    M3 {},
}

fn takes_s1(s1: &mut S1) -> MyState {
    todo!()
}

impl Receive<MyMessage> for MyState {
    fn receive(&mut self, context: ActorContext, message: MyMessage) {
        match self {
            MyState::S1(s) => match message {
                MyMessage::M1 { .. } => {}
                MyMessage::M2 { .. } => {}
                MyMessage::M3 { .. } => {}
            },
            MyState::S2(s) => match message {
                MyMessage::M1 { .. } => {}
                MyMessage::M2 { .. } => {}
                MyMessage::M3 { .. } => {}
            },
            MyState::S3(s) => match message {
                MyMessage::M1 { .. } => {}
                MyMessage::M2 { .. } => {}
                MyMessage::M3 { .. } => {}
            },
        }
    }
}

fn main() {}
