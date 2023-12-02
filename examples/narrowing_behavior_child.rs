use actum::prelude::*;
use tokio::time::interval;

enum Public {
    Public,
}

impl Into<All> for Public {
    fn into(self) -> All {
        All::Public
    }
}

enum All {
    Public,
    Private,
}

fn service_impl(parent: ActorRef<All>) -> Setup<()> {
    setup(move |context1| {
        assert!(parent.send(All::Private));
        stop_with_output(()).into()
        // stop_with_callback(move |context| {
        //     context1.spawn("a", receive_message::<u32, (), ()>(move |x, y: u32| {
        //         todo!()
        //     }));
        //     ()
        // }).into()
    })
}

fn service() -> Setup<Public, ActorResult<()>> {
    let private = setup::<All, (), ()>(move |context| {
        context.spawn("serviceimpl", service_impl(context.me.clone())).unwrap();

        receive_message(move |context, message| {
            match message {
                All::Public => println!("Received public message!"),
                All::Private => println!("Received private message!"),
            }
            receive::Next::Same
        })
        .into()
    });
    let public = narrow::<Public, _, _, _>(private);
    public
}

fn main() {
    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    actum(system, service(), |guardian| {
        guardian.send(Public::Public);
        // nothing
    });
}
