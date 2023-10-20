use actum::behavior::Wrap;
use actum::prelude::*;

enum Outer {
    Public,
}

enum Inner {
    Public,
    Private,
}

impl From<Outer> for Inner {
    fn from(outer: Outer) -> Self {
        match outer {
            Outer::Public => Inner::Public,
        }
    }
}

fn make_behavior() -> Setup<Outer, u32, Wrap<u32>> {
    let inner: Receive<Inner, u32, ()> = receive_message(move |context, inner: Inner| {
        match inner {
            Inner::Public => println!("Received public message"),
            Inner::Private => println!("Received private message"),
        };
        receive::Next::Same
    });
    narrow::<Outer, Inner, u32, ()>(inner)
}

fn main() {}
