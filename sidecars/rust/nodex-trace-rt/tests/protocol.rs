//! The runtime against a local listener: handshake, frames, records and their order.
//! (Its own process: the tracer reads its environment once.)

use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;

#[derive(Debug, Clone, PartialEq)]
struct Record {
    kind: u8,
    thread: i32,
    method: i32,
    arg: i32,
    text: String,
}

fn decode(bytes: &[u8]) -> Vec<Record> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 4 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let frame = &bytes[pos + 4..pos + 4 + len];
        let mut i = 0;
        while i < frame.len() {
            let kind = frame[i];
            let thread = i32::from_le_bytes(frame[i + 1..i + 5].try_into().unwrap());
            let method = i32::from_le_bytes(frame[i + 13..i + 17].try_into().unwrap());
            let arg = i32::from_le_bytes(frame[i + 17..i + 21].try_into().unwrap());
            let tl = u16::from_le_bytes(frame[i + 21..i + 23].try_into().unwrap()) as usize;
            let text = String::from_utf8_lossy(&frame[i + 23..i + 23 + tl]).into_owned();
            out.push(Record { kind, thread, method, arg, text });
            i += 23 + tl;
        }
        pos += 4 + len;
    }
    out
}

#[derive(Debug)]
struct Order {
    id: u32,
}

fn traced_function(id: u32) -> u32 {
    let _g = nodex_trace_rt::enter(5);
    nodex_trace_rt::nx_arg!(5, 0, id);
    nodex_trace_rt::step(5, 1);
    let order = Order { id };
    nodex_trace_rt::nx_local!(5, 0, order);
    nodex_trace_rt::step(5, 2);
    let r = order.id * 2;
    nodex_trace_rt::nx_ret!(5, r);
    r
}

#[test]
fn sends_the_nodex_protocol() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // SAFETY: set before any other thread of this test process reads the environment.
    unsafe {
        std::env::set_var("NODEX_TRACE", addr.to_string());
        std::env::set_var("NODEX_TRACE_TOKEN", "secret");
    }
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut hello = String::new();
        reader.read_line(&mut hello).unwrap();
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        (hello, rest)
    });

    assert_eq!(traced_function(21), 42);
    let worker = std::thread::spawn(|| {
        nodex_trace_rt::step(9, 0);
    });
    worker.join().unwrap();
    let panicked = std::panic::catch_unwind(|| {
        let _g = nodex_trace_rt::enter(6);
        panic!("boom");
    });
    assert!(panicked.is_err());
    // What the exit handler does: send the rest and close.
    std::thread::sleep(std::time::Duration::from_millis(50));
    nodex_trace_rt::close_for_tests();

    let (hello, bytes) = server.join().unwrap();
    assert_eq!(hello, "NODEX1 secret\n");
    let records = decode(&bytes);
    assert_eq!(records[0].kind, 10, "every frame starts with Sequence");
    assert_eq!(records[0].arg, 1);
    let kinds: Vec<(u8, i32, i32, &str)> =
        records.iter().filter(|r| r.kind != 10).map(|r| (r.kind, r.method, r.arg, r.text.as_str())).collect();
    assert_eq!(
        &kinds[..7],
        &[
            (1, 5, 0, ""),
            (6, 5, 0, "21"),
            (2, 5, 1, ""),
            (4, 5, 0, "Order { id: 21 }"),
            (2, 5, 2, ""),
            (5, 5, 0, "42"),
            (3, 5, 0, ""),
        ]
    );
    let main_thread = records.iter().find(|r| r.kind == 1).unwrap().thread;
    let worker_step = records.iter().find(|r| r.kind == 2 && r.method == 9).unwrap();
    assert_ne!(worker_step.thread, main_thread, "threads get their own ids");
    let throw = records.iter().find(|r| r.kind == 7).expect("panic recorded");
    assert_eq!(throw.text, "panic: boom");
    assert!(records.iter().any(|r| r.kind == 3 && r.method == 6), "the guard closes the frame while unwinding");
}
