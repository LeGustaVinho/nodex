# Sidecars

Helpers that run outside the Rust process.

| Path | Milestone | Purpose |
| :--- | :--- | :--- |
| `dotnet/Nodex.Roslyn/` | M3 ✅ | C# semantics through `MSBuildWorkspace` or loose files: calls, creations, bases, events, `var` types. Line-delimited JSON over stdio. Build: `dotnet build sidecars/dotnet/Nodex.Roslyn -c Release` |
| `dotnet/Nodex.Trace/` | M4 ✅ | Trace runtime (`netstandard2.1`; its source is the Unity package's `Runtime/T.cs`): buffered binary events over loopback TCP; no-op unless started by Nodex |
| `dotnet/Nodex.Weaver/` | M4 ✅ | Mono.Cecil IL weaver guided by PDB sequence points; writes the instrumented copy and `nodex-trace-map.json`. Build: `dotnet build sidecars/dotnet/Nodex.Weaver -c Release` |
| `unity/com.nodex.trace/` | M5 ✅ | UPM package (Link or Embed from Nodex's Unity card): the trace runtime's source (`Runtime/T.cs`, also compiled by `dotnet/Nodex.Trace`), the shared weaver sources (`CodeGen/ModuleWeaver.cs`, `SourceMaps.cs`, also compiled by `dotnet/Nodex.Weaver`) run as an IL post-processor while `Library/Nodex/trace-on` exists, and the Editor bridge driven by `.nodex/unity/session.json`. Tests: `cargo test -p nodex-server --test unity -- --ignored --test-threads 1` (batch-mode Unity on `fixtures/unity-sample`) |
| `node/nodex-ts/` | M6 ✅ | TypeScript compiler API: semantics over the same line-delimited JSON protocol as `Nodex.Roslyn`, and `nodex-ts instrument` (instrumented JavaScript copy + `nodex-trace-map.json`; M8: `--target browser` prints an instrumented TS/TSX/Vue source copy for dev servers, `--debug` emits plain JavaScript with source maps for the debugger). Install: `pnpm --dir sidecars/node/nodex-ts install` (also builds `dist/`) |
| `node/nodex-trace/` | M6 ✅ | `runtime.cjs`: dependency-free Node.js trace runtime, preloaded with `node --require`; same binary protocol as `Nodex.Trace`, plus a tail file for abrupt exits. M8 ✅ `runtime-browser.js`: the same probes for browser apps, sending frames over the run's `/trace` WebSocket |
| `rust/nodex-trace-rt/` | M7 ✅ | Rust trace runtime (std only): probes the Rust instrumenter inserts into a copy of the crate; same binary protocol, flushes at exit through `atexit`, records panics. Tests: `cargo test --manifest-path sidecars/rust/nodex-trace-rt/Cargo.toml` |

See the vault notes "Runtime Visualization Strategy", "ADR-005 IL weaving for CSharp and Unity" and "ADR-012 One Node sidecar for TypeScript semantics and traces" and "ADR-014 Rust semantics from rust-analyzer SCIP and traces by source instrumentation".
