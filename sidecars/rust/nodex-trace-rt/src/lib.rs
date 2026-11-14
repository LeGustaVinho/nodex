//! # Nodex trace runtime for Rust
//!
//! Instrumented copies of Rust programs (made by Nodex, see `crates/nodex-lang-rust/src/
//! instrument.rs`) call these probes. They send the Nodex binary trace protocol (the same as
//! `Nodex.Trace` for .NET and `runtime.cjs` for Node.js) to the Nodex server over loopback TCP:
//!
//! - `NODEX_TRACE` (`host:port`) and `NODEX_TRACE_TOKEN` say where; without them every probe
//!   is a no-op, so an instrumented build also runs normally outside Nodex;
//! - records are buffered and a background thread sends a frame every 15 ms; each frame starts
//!   with a `Sequence` record; above 16 MB pending, records are dropped and counted;
//! - at exit (also `std::process::exit`) the rest is flushed from a C `atexit` handler; frames
//!   that cannot be sent go to `NODEX_TRACE_TAIL/<pid>.bin`;
//! - a panic records `Throw` (`panic: message`) before the previous panic hook runs.
//!
//! Values are formatted inside the program: with `Debug` when the type has it (derived
//! implementations read fields; a hand-written `Debug` runs that code), otherwise the type name.
//! Strings are cut at 48 characters, every value at 96.

use std::cell::Cell;
use std::future::Future;
use std::io::Write;
use std::net::TcpStream;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

const ENTER: u8 = 1;
const STEP: u8 = 2;
const EXIT: u8 = 3;
const LOCAL: u8 = 4;
const RETURN: u8 = 5;
const ARG: u8 = 6;
const THROW: u8 = 7;
const DROPPED: u8 = 9;
const SEQUENCE: u8 = 10;
const LOCAL_TYPE: u8 = 11;
const ARG_TYPE: u8 = 12;

const HEADER: usize = 1 + 4 + 8 + 4 + 4 + 2;
const MAX_PENDING: usize = 16 * 1024 * 1024;
const MAX_TEXT: usize = 96;
const MAX_STRING: usize = 48;

struct Pending {
    bytes: Vec<u8>,
    dropped: i32,
}

struct Tracer {
    stream: Mutex<Option<TcpStream>>,
    pending: Mutex<Pending>,
    started: Instant,
    sequence: AtomicI32,
    tail: Option<std::path::PathBuf>,
    closed: AtomicBool,
}

static TRACER: OnceLock<Option<Tracer>> = OnceLock::new();
static NEXT_THREAD: AtomicI32 = AtomicI32::new(1);

thread_local! {
    static THREAD: Cell<i32> = const { Cell::new(0) };
}

fn thread_id() -> i32 {
    THREAD.with(|t| {
        if t.get() == 0 {
            t.set(NEXT_THREAD.fetch_add(1, Ordering::Relaxed));
        }
        t.get()
    })
}

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> i32;
}

extern "C" fn flush_at_exit() {
    if let Some(Some(t)) = TRACER.get() {
        t.close();
    }
}

fn tracer() -> Option<&'static Tracer> {
    TRACER
        .get_or_init(|| {
            let addr = std::env::var("NODEX_TRACE").ok()?;
            let token = std::env::var("NODEX_TRACE_TOKEN").unwrap_or_default();
            let socket = addr.parse().ok()?;
            let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2)).ok()?;
            let _ = stream.set_nodelay(true);
            stream.write_all(format!("NODEX1 {token}\n").as_bytes()).ok()?;
            let tail = std::env::var_os("NODEX_TRACE_TAIL").map(std::path::PathBuf::from);
            Some(Tracer {
                stream: Mutex::new(Some(stream)),
                pending: Mutex::new(Pending { bytes: Vec::with_capacity(64 * 1024), dropped: 0 }),
                started: Instant::now(),
                sequence: AtomicI32::new(0),
                tail,
                closed: AtomicBool::new(false),
            })
        })
        .as_ref()
        .inspect(|_| start_once())
}

/// Starts the flush thread, the exit handler and the panic hook (once).
fn start_once() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new().name("nodex-trace".into()).spawn(|| {
            loop {
                std::thread::sleep(Duration::from_millis(15));
                match TRACER.get() {
                    Some(Some(t)) if !t.closed.load(Ordering::Relaxed) => t.flush(),
                    _ => break,
                }
            }
        });
        // SAFETY: `flush_at_exit` is a plain `extern "C" fn()` that never unwinds.
        unsafe {
            atexit(flush_at_exit);
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_owned()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "panic".to_owned()
            };
            if let Some(Some(t)) = TRACER.get() {
                t.record(THROW, 0, 0, Some(&cut(&format!("panic: {message}"))));
                t.flush();
            }
            previous(info);
        }));
    });
}

impl Tracer {
    fn record(&self, kind: u8, method: i32, arg: i32, text: Option<&str>) {
        let micros = self.started.elapsed().as_micros() as i64;
        let thread = thread_id();
        let Ok(mut p) = self.pending.lock() else { return };
        if p.bytes.len() > MAX_PENDING {
            p.dropped = p.dropped.saturating_add(1);
            return;
        }
        write_record(&mut p.bytes, kind, thread, micros, method, arg, text);
    }

    /// The pending records as one frame, or `None` when there are none.
    fn take_frame(&self) -> Option<Vec<u8>> {
        let mut p = self.pending.lock().ok()?;
        if p.dropped > 0 {
            let d = std::mem::take(&mut p.dropped);
            let micros = self.started.elapsed().as_micros() as i64;
            write_record(&mut p.bytes, DROPPED, thread_id(), micros, 0, d, None);
        }
        if p.bytes.is_empty() {
            return None;
        }
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let mut payload = Vec::with_capacity(HEADER + p.bytes.len());
        write_record(&mut payload, SEQUENCE, std::process::id() as i32, 0, 0, seq, None);
        payload.append(&mut p.bytes);
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&payload);
        Some(frame)
    }

    fn flush(&self) {
        let Some(frame) = self.take_frame() else { return };
        let sent = match self.stream.lock() {
            Ok(mut s) => match s.as_mut() {
                Some(stream) => stream.write_all(&frame).is_ok(),
                None => false,
            },
            Err(_) => false,
        };
        if !sent {
            self.to_tail(&frame);
        }
    }

    fn to_tail(&self, frame: &[u8]) {
        let Some(dir) = &self.tail else { return };
        let _ = std::fs::create_dir_all(dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{}.bin", std::process::id()))) {
            let _ = f.write_all(frame);
        }
    }

    /// Sends the rest and closes the connection (process exit).
    fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.flush();
        if let Ok(mut s) = self.stream.lock()
            && let Some(stream) = s.take()
        {
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    }
}

fn write_record(out: &mut Vec<u8>, kind: u8, thread: i32, micros: i64, method: i32, arg: i32, text: Option<&str>) {
    let bytes = text.map(str::as_bytes).unwrap_or(&[]);
    let len = bytes.len().min(u16::MAX as usize);
    out.push(kind);
    out.extend_from_slice(&thread.to_le_bytes());
    out.extend_from_slice(&micros.to_le_bytes());
    out.extend_from_slice(&method.to_le_bytes());
    out.extend_from_slice(&arg.to_le_bytes());
    out.extend_from_slice(&(len as u16).to_le_bytes());
    out.extend_from_slice(&bytes[..len]);
}

/// Sends what is pending and closes the connection, as the exit handler does. For tests.
#[doc(hidden)]
pub fn close_for_tests() {
    flush_at_exit();
}

/// True when a Nodex server is listening (probes that format values check this first).
#[inline]
pub fn on() -> bool {
    tracer().is_some()
}

/// Records the start of a function; the returned guard records its end when dropped (also on
/// early returns, `?` and panics).
#[must_use = "the guard records the end of the function when it is dropped"]
pub fn enter(method: i32) -> Guard {
    if let Some(t) = tracer() {
        t.record(ENTER, method, 0, None);
    }
    Guard { method }
}

/// Ends a function's frame when dropped.
pub struct Guard {
    method: i32,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(Some(t)) = TRACER.get() {
            t.record(EXIT, self.method, 0, None);
        }
    }
}

/// A statement (point `point` of the method's map) is about to run.
#[inline]
pub fn step(method: i32, point: i32) {
    if let Some(t) = tracer() {
        t.record(STEP, method, point, None);
    }
}

/// The value of parameter `index`, already formatted (see [`value!`]).
pub fn arg(method: i32, index: i32, text: String) {
    if let Some(t) = tracer() {
        t.record(ARG, method, index, Some(&text));
    }
}

/// The value written to local `slot`, already formatted.
pub fn local(method: i32, slot: i32, text: String) {
    if let Some(t) = tracer() {
        t.record(LOCAL, method, slot, Some(&text));
    }
}

#[doc(hidden)]
pub fn observe<T: ?Sized>(method: i32, slot: i32, parameter: bool, value: &T) {
    if let Some(t) = tracer() {
        let native = std::any::type_name_of_val(value);
        if native.len() <= 1024 {
            let descriptor = format!("\n{native}");
            t.record(if parameter { ARG_TYPE } else { LOCAL_TYPE }, method, slot, Some(&descriptor));
        }
    }
}

/// The value the function returns, already formatted.
pub fn ret(method: i32, text: String) {
    if let Some(t) = tracer() {
        t.record(RETURN, method, 0, Some(&text));
    }
}

/// Wraps an awaited future: when it is not ready the current frame ends (`Exit`), and the next
/// poll enters `resume` (the `resume` method of the async function in the map), the same frame
/// model as .NET state machines and JavaScript `await`.
pub fn traced<F: std::future::IntoFuture>(resume: i32, future: F) -> Traced<F::IntoFuture> {
    Traced { inner: Box::pin(future.into_future()), resume, suspended: false }
}

pub struct Traced<F: Future> {
    inner: Pin<Box<F>>,
    resume: i32,
    suspended: bool,
}

impl<F: Future> Future for Traced<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let t = tracer();
        if self.suspended {
            self.suspended = false;
            if let Some(t) = t {
                t.record(ENTER, self.resume, 0, None);
            }
        }
        let r = self.inner.as_mut().poll(cx);
        if r.is_pending() {
            self.suspended = true;
            if let Some(t) = t {
                t.record(EXIT, self.resume, 0, None);
            }
        }
        r
    }
}

// --- value formatting ----------------------------------------------------------------------

/// Cuts a formatted value: strings at 48 characters, everything at 96.
pub fn cut(text: &str) -> String {
    let quoted = text.len() > 1 && text.starts_with('"') && text.ends_with('"');
    let max = if quoted { MAX_STRING + 2 } else { MAX_TEXT };
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max - 2).collect();
    out.push('…');
    if quoted {
        out.push('"');
    }
    out
}

/// Holds a value for formatting (see [`value!`]).
pub struct Fmt<'a, T: ?Sized>(pub &'a T);

/// Formats with `Debug` (chosen first when the type implements it).
pub trait ViaDebug {
    fn nodex_fmt(&self) -> String;
}

impl<T: ?Sized + std::fmt::Debug> ViaDebug for Fmt<'_, T> {
    fn nodex_fmt(&self) -> String {
        cut(&format!("{:?}", self.0))
    }
}

/// Formats as the type's name (for types without `Debug`).
pub trait ViaTypeName {
    fn nodex_fmt(&self) -> String;
}

impl<T: ?Sized> ViaTypeName for &Fmt<'_, T> {
    fn nodex_fmt(&self) -> String {
        let name = std::any::type_name::<T>();
        // `alloc::vec::Vec<app::Order>` → `Vec<Order>`.
        let mut out = String::new();
        for part in name.split_inclusive(['<', '>', ',', ' ']) {
            out.push_str(part.rsplit("::").next().unwrap_or(part));
        }
        out
    }
}

/// Formats a value: `Debug` when available, otherwise the type name.
#[macro_export]
macro_rules! value {
    ($e:expr) => {{
        #[allow(unused_imports)]
        use $crate::{ViaDebug as _, ViaTypeName as _};
        (&$crate::Fmt(&$e)).nodex_fmt()
    }};
}

/// `arg(method, index, value!(x))` when tracing.
#[macro_export]
macro_rules! nx_arg {
    ($m:expr, $i:expr, $e:expr) => {
        if $crate::on() {
            let value = &$e;
            $crate::arg($m, $i, $crate::value!(*value));
            $crate::observe($m, $i, true, value);
        }
    };
}

/// `local(method, slot, value!(x))` when tracing.
#[macro_export]
macro_rules! nx_local {
    ($m:expr, $s:expr, $e:expr) => {
        if $crate::on() {
            let value = &$e;
            $crate::local($m, $s, $crate::value!(*value));
            $crate::observe($m, $s, false, value);
        }
    };
}

/// Observe a tail value while preserving the expected type of the enclosing expression.
pub fn returning<T>(value: T, inspect: impl FnOnce(&T)) -> T {
    inspect(&value);
    value
}

/// `ret(method, value!(x))` when tracing.
#[macro_export]
macro_rules! nx_ret {
    ($m:expr, $e:expr) => {
        if $crate::on() {
            $crate::ret($m, $crate::value!($e));
        }
    };
}

#[cfg(test)]
mod tests {
    struct Opaque;
    #[derive(Debug)]
    #[allow(dead_code)]
    struct Order {
        id: u32,
    }

    #[test]
    fn values_use_debug_or_the_type_name() {
        assert_eq!(value!(42), "42");
        assert_eq!(value!("alice"), "\"alice\"");
        assert_eq!(value!(Order { id: 7 }), "Order { id: 7 }");
        assert_eq!(value!(Opaque), "Opaque");
        assert_eq!(value!(vec![Opaque]), "Vec<Opaque>");
        let long = "x".repeat(200);
        assert_eq!(value!(long).chars().count(), 50);
        assert!(value!(vec![1u64; 100]).ends_with('…'));
    }
}
