use actum::prelude::*;
use tracing::info;

enum Public { Public, }

impl Into<All> for Public { fn into(self) -> All { All::Public } }

enum All { Public, Private, }

fn service() -> Setup<Public, ActorResult<()>> {
    let service_impl = setup::<All, (), ()>(move |context| {
        context.me.send(All::Private);
        receive_message(move |context, message| {
            match message {
                All::Public => info!("Received public message!"),
                All::Private => info!("Received private message!"),
            }
            receive::Next::Same
        })
        .into()
    });
    let service = narrow::<Public, _, _, _>(service_impl);
    service
}

fn main() {
    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    actum(system, service(), |root| {
        root.send(Public::Public);
    });
}
