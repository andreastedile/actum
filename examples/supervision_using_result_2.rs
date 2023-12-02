use actum::prelude::*;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{Error, ErrorKind, Write};
use std::path::Path;
use tracing::{error, info};

enum ChildError { Reason1, Reason2, }
fn child() -> Setup<(), Result<u32, ChildError>> {
    setup(move |context| {
        //
        stop_with_output(Err(ChildError::Reason1)).into()
    })
}

fn parent() -> Setup<(), (), Result<u32, ChildError>> {
    setup(move |context| {
        let child = context.spawn("child", child()).unwrap();
        context.watch(&child);
        receive_supervision(move |context, path, result| {
            match result.0.unwrap().unwrap() {
                Ok(output) => {}
                Err(ChildError::Reason1) => {}
                Err(ChildError::Reason2) => {}
                Err(_) => {}
            }
            receive::Next::Empty
        })
        .into()
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_target(false)
        .with_line_number(false)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let system = ActorSystem::new("sum", ActorSystemConfig::default()).unwrap();

    actum(system, parent(), |guardian| {});
}
