struct ActorContext;

trait Receive<M> {
    fn receive(&mut self, context: ActorContext, message: M);
}

enum MyState { S1 {}, S2 {}, S3 {}, }

enum MyMessage { M1 {}, M2 {}, M3 {}, }

impl Receive<MyMessage> for MyState {
    fn receive(&mut self, context: ActorContext, message: MyMessage) {
        match self {
            MyState::S1 { .. } => {
                match message {
                    MyMessage::M1 { .. } => {}
                    MyMessage::M2 { .. } => {}
                    MyMessage::M3 { .. } => {}
                }
            },
            MyState::S2 { .. } => {
                match message {
                    MyMessage::M1 { .. } => {}
                    MyMessage::M2 { .. } => {}
                    MyMessage::M3 { .. } => {}
                }
            },
            MyState::S3 { .. } => {
                match message {
                    MyMessage::M1 { .. } => {}
                    MyMessage::M2 { .. } => {}
                    MyMessage::M3 { .. } => {}
                }
            },
        }
    }
}

fn main() {}
