use actum::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::info;

const COST: u32 = 50;
enum Action {
    AddCoin { value: u32, reply_to: ActorRef<u32>, },
    Select { beverage: Beverage, reply_to: ActorRef<SelectionResult>, },
    Brewed { beverage: Beverage, reply_to: ActorRef<SelectionResult> },
}
enum Beverage { Tea, Coffee, }
enum SelectionResult {
    Coin { value: u32 },
    Beverage(Beverage)
}
fn pay() -> Receive<Action> {
    let mut total = 0;
    receive_message(move |context, message| match message {
        Action::AddCoin { value, reply_to } => {
            total += value;
            receive::Next::Same
        }
        Action::Select { beverage, reply_to } => {
            if total == COST {
                brew(beverage, reply_to).into()
            } else if total > COST {
                reply_to.send(SelectionResult::Coin { value: total - COST });
                brew(beverage, reply_to).into()
            } else {
                receive::Next::Same
            }
        }
        Action::Brewed { .. } => receive::Next::Unhandled,
    })
}
fn brew(beverage: Beverage, reply_to: ActorRef<SelectionResult>) -> Setup<Action> {
    setup(move |context| {
        context.pipe_to_self(async {
            tokio::time::sleep(Duration::from_secs(15)).await;
            Action::Brewed { beverage, reply_to }
        });
        receive_message(move |context, message| match message {
            Action::AddCoin { value, reply_to } => {
                reply_to.send(value);
                receive::Next::Same
            }
            Action::Select { .. } => receive::Next::Same,
            Action::Brewed { beverage, reply_to } => {
                reply_to.send(SelectionResult::Beverage(beverage));
                pay().into()
            },
        }).into()
    })
}

fn main() {}