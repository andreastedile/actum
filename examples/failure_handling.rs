use actum::prelude::*;

fn parent() -> Setup<(), u32> {
    setup(move |context| {
        let child = context.spawn("andrea", child()).unwrap();
        context.watch(&child);

        child.send(false);
        child.send(false);
        child.send(true);

        receive_supervision(move |_context, path, result| {
            assert!(!child.send(false));

            stop_with_output(42).into()
        })
        .into()
    })
}

fn child() -> Receive<bool> {
    receive_message(move |_context, crash| {
        println!("crash: {crash}");

        if crash {
            panic!("Simulate a crash!!!");
        } else {
            receive::Next::Same
        }
    })
}

fn main() {
    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    let output = actum(system, parent(), |guardian| {
        // nothing
    })
    .unwrap()
    .unwrap();
    println!("Output: {output}");
}
