struct ActorContext;

trait Receive {
    type Message;
    fn receive(&mut self, context: ActorContext, message: Self::Message);
}

enum MyState { S1 {}, S2 {}, S3 {}, }

struct M1 {} struct M2 {} struct M3 {}

impl Receive for MyState {
    type Message = M1;
    fn receive(&mut self, context: ActorContext, message: M1) {
        match self {
            MyState::S1 { .. } => {}
            MyState::S2 { .. } => {}
            MyState::S3 { .. } => {}
        }
    }
}

impl Receive for MyState {
    type Message = M2;
    fn receive(&mut self, context: ActorContext, message: M2) {
        match self {
            MyState::S1 { .. } => {}
            MyState::S2 { .. } => {}
            MyState::S3 { .. } => {}
        }
    }
}

impl Receive for MyState {
    type Message = M3;
    fn receive(&mut self, context: ActorContext, message: M3) {
        match self {
            MyState::S1 { .. } => {}
            MyState::S2 { .. } => {}
            MyState::S3 { .. } => {}
        }
    }
}

fn main() {}
