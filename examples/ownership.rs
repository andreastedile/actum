use actum::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::net::TcpStream;
use tokio::join;

// struct Resource(usize);
//
// impl Resource {
//     fn mutate(&mut self) {
//         self.0 += 1;
//         println!("Mutated: {}", self.0);
//     }
// }
//
// fn ownership_v4() -> Receive<()> {
//     let mut resource = Some(Box::new(Resource(0)));
//     receive(move |input| {
//         let mut resource = Some(resource.take().unwrap());
//         if let Some(ref mut resource) = resource {
//             resource.mutate();
//         }
//         receive(move |input| {
//             let mut resource = Some(resource.take().unwrap());
//             if let Some(ref mut resource) = resource {
//                 resource.mutate();
//             }
//             receive::Next::Same
//         })
//         .into()
//     })
// }
//
// fn ownership_v5() -> Receive<()> {
//     let mut resource = Some(Resource(0));
//     receive(move |input| {
//         let mut resource = Some(resource.take().unwrap());
//         if let Some(ref mut resource) = resource {
//             resource.mutate();
//         }
//         receive(move |input| {
//             let mut resource = Some(resource.take().unwrap());
//             if let Some(ref mut resource) = resource {
//                 resource.mutate();
//             }
//             receive::Next::Same
//         })
//         .into()
//     })
// }

fn file_behavior_1(mut file: Option<File>) -> Receive<String> {
    receive_message(move |context, message: String| {
        let file = file.take().unwrap(); // use file
        file_behavior_2(Some(file)).into()
    })
}

fn file_behavior_2(mut file: Option<File>) -> Receive<String> {
    receive_message(move |context, message: String| {
        let file = file.take().unwrap(); // use file
        file_behavior_1(Some(file)).into()
    })
}

fn push_two(mut vec: Vec<u32>) {
    vec.push(2);
}

struct S(u32, Vec<u32>);

impl S {
    fn im<'a>(&'a self) -> &'a u32 {
        &self.1[0]
    }

    fn mu<'a>(&'a mut self) {
        self.0 = 1;
    }
}

struct Cl<'a>(Box<dyn FnMut() + Send + 'a>);

async fn behavior<'a>(mut cl: Cl<'a>) {
    loop {
        cl.0();
    }
}

async fn behavior2<'a>(mut cl: impl FnMut() + 'a) {
    loop {
        cl();
    }
}

fn main() {
    let mut v = vec![1, 2, 3];
    let mut cl = |n| {
        println!("Push {} to v", n);
        v.push(n);
        v.len()
    };
    // println!("v: {:?},", v); // Compiler error!
    let len = cl(4);
    // println!("Len: {}, v: {:?},", len, v);
    let len = cl(5);
    println!("Len: {}, v: {:?},", len, v);
}
