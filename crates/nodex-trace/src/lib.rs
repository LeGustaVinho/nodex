//! # Nodex trace
//!
//! Live execution support, independent of the transport and of the language:
//! - [`protocol`]: decodes the binary event stream written by instrumented programs,
//! - [`map`]: the instrumenter's map from `(method, point)` to source byte ranges,
//! - [`resolve`]: source byte ranges → flow nodes of the current IR,
//! - [`recorder`]: aggregates events into live frames (counts, active nodes, values) and keeps
//!   a timeline for replay.

pub mod map;
pub mod protocol;
pub mod recorder;
pub mod resolve;

pub use map::TraceMap;
pub use protocol::{Decoder, Record, RecordKind};
pub use recorder::{ActiveNode, Recorder, StepInfo, StepsPage, TraceFrame};
pub use resolve::Resolver;
