//! Timing of the expensive steps of opening a project (walk, lowering, linking, knowledge).
//!
//! Each finished phase is one `info` line on the `nodex_perf` target, so the log of a slow
//! open says where the time went (`RUST_LOG=nodex_perf=info` shows only these lines).

use std::fmt::Display;
use std::time::{Duration, Instant};

/// A running phase; [`Phase::finish`] logs how long it took.
#[must_use = "call finish() to log the phase"]
pub struct Phase {
    name: &'static str,
    started: Instant,
}

/// Starts timing `name`.
pub fn phase(name: &'static str) -> Phase {
    Phase { name, started: Instant::now() }
}

impl Phase {
    /// Logs the elapsed time with a short detail (counts, sizes) and returns it.
    pub fn finish(self, detail: impl Display) -> Duration {
        let elapsed = self.started.elapsed();
        tracing::info!(target: "nodex_perf", "{}: {:.1} ms {}", self.name, elapsed.as_secs_f64() * 1000.0, detail);
        elapsed
    }
}
