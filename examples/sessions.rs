use actum::prelude::*;

struct GetSession {
    reply_to: ActorRef<GetSessionResult>,
}
struct ActiveSession {
    service: ActorRef<SessionCommand>,
}

impl Into<GetSessionResult> for ActiveSession {
    fn into(self) -> GetSessionResult {
        GetSessionResult::Active(self)
    }
}
impl Into<GetSessionResult> for NewSession {
    fn into(self) -> GetSessionResult {
        GetSessionResult::New(self)
    }
}
impl Into<AuthenticateResult> for ActiveSession {
    fn into(self) -> AuthenticateResult {
        AuthenticateResult::Active(self)
    }
}

struct NewSession {
    auth: ActorRef<Authenticate>,
}

struct Authenticate {
    username: String,
    password: String,
    reply_to: ActorRef<AuthenticateResult>,
}

enum AuthenticateResult {
    Active(ActiveSession),
    Failed,
}

enum SessionCommand {
    CommandA,
    CommandB,
}

enum GetSessionResult {
    Active(ActiveSession),
    New(NewSession),
}

enum Protocol {
    GetSession(GetSession),
    Authenticate(Authenticate),
    SessionCommand(SessionCommand),
}

impl Into<Protocol> for Authenticate {
    fn into(self) -> Protocol {
        Protocol::Authenticate(self)
    }
}

impl Into<Protocol> for SessionCommand {
    fn into(self) -> Protocol {
        Protocol::SessionCommand(self)
    }
}

fn service() -> Receive<Protocol> {
    receive_message(move |context, message| {
        match message {
            Protocol::GetSession(GetSession { reply_to }) => {
                reply_to.send(GetSessionResult::New(NewSession {
                    auth: context.me.narrow::<Authenticate>(),
                }));
                reply_to.send(GetSessionResult::Active(ActiveSession {
                    service: context.me.narrow::<SessionCommand>(),
                }));
            }
            Protocol::Authenticate(Authenticate {
                username,
                password,
                reply_to,
            }) => {
                reply_to.send(AuthenticateResult::Failed);
                reply_to.send(AuthenticateResult::Active(ActiveSession {
                    service: context.me.narrow::<SessionCommand>(),
                }));
            }
            Protocol::SessionCommand(SessionCommand::CommandA) => {todo!()}
            Protocol::SessionCommand(SessionCommand::CommandB) => {todo!()}
        }
        receive::Next::Same
    })
}

fn main() {}
