use std::time::Duration;
use tokio::time::sleep;
use actum::actor_system::{ActorSystem, ActorSystemConfig};
use actum::actum::actum;
use actum::behavior::*;
use actum::prelude::{ActorRef, ActorResult, Receive, Setup};
use tracing::info;

struct Authenticate { username: String, password: String, reply_to: ActorRef<AuthenticateResult>, }
enum Command { DoThis, DoThat, }
enum ServerMessage { Authenticate(Authenticate), Command(Command), }
struct AuthenticateResult(Option<ActorRef<Command>>);
impl Into<ServerMessage> for Authenticate { fn into(self) -> ServerMessage { ServerMessage::Authenticate(self) } }
impl Into<ServerMessage> for Command { fn into(self) -> ServerMessage { ServerMessage::Command(self) } }

fn server_behavior() -> Setup<Authenticate, ActorResult<()>> {
    let private = receive_message(move |context, message| {
        match message {
            ServerMessage::Authenticate(authenticate) => {
                let service = context.me.clone().narrow::<Command>();
                authenticate.reply_to.send(AuthenticateResult(Some(service)));
            }
            ServerMessage::Command(Command::DoThis) => info!("Doing this"),
            ServerMessage::Command(Command::DoThat) => info!("Doing that"),
        }
        receive::Next::Same
    });
    let public = narrow::<Authenticate, ServerMessage, (), ()>(private);
    public
}
fn client(server: ActorRef<Authenticate>) -> Setup<AuthenticateResult> {
    setup(move |context| {
        server.send(Authenticate { username: "steddy".to_string(), password: "1234".to_string(), reply_to: context.me.clone(), });
        receive_message(move |context, message: AuthenticateResult| {
            if let Some(service) = message.0 {
                service.send(Command::DoThis);
                service.send(Command::DoThat);
            }
            receive::Next::Empty
        }).into()
    })
}

struct Supervision;

impl Into<Supervision> for () {
    fn into(self) -> Supervision {
        Supervision
    }
}

impl Into<Supervision> for ActorResult<()> {
    fn into(self) -> Supervision {
        Supervision
    }
}

fn simulation() -> Setup<(), (), Supervision> {
    setup(move |context| {
        let server = context.spawn("server", server_behavior()).unwrap();
        let client = context.spawn("client", client(server));
        setup::Next::Empty
    })
}

fn main() {
    tracing_subscriber::fmt()
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
        .with_target(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let system = ActorSystem::new("example", ActorSystemConfig::default()).unwrap();
    actum(system, simulation(), |_| {}).0.unwrap().unwrap();
}
