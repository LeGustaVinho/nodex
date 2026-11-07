# Nodex

Nodex turns source code into language-agnostic, Blueprint-style node graphs so a programmer can read code in a language they do not know.
The same logic looks the same in every language: a C# `foreach`, a TypeScript `for..of` and a Rust `for x in iter` are one **For Each** node.

**Status**: milestones M0–M4.1 — C# (plain .NET and Unity) structure, Blueprints, Roslyn semantics with a Calls view, and live execution traces for .NET programs with replay, exceptions and a call Timeline. Unity Play Mode traces, TypeScript and Rust are on the roadmap.
Full documentation lives in the Obsidian vault at `D:\MEGA\Obsidian\Default\Nodex` (start at `Nodex.md`).

## Quick start

```bash
pnpm --dir web install
```

```bash
pnpm --dir web build
```

Optional, for exact calls, events and `var` types (needs the .NET 10 SDK):

```bash
dotnet build sidecars/dotnet/Nodex.Roslyn -c Release
```

Optional, to run programs with live traces (**▶ Run** in the UI):

```bash
dotnet build sidecars/dotnet/Nodex.Weaver -c Release
```

```bash
cargo run --release -p nodex-server -- open fixtures/csharp/Sample
```

The server prints a link with a session token and opens it in your browser.

## Views

- **Architecture** — namespaces, imported namespaces and packages.
- **Types** — classes, interfaces and records, with *extends*, *implements*, *uses* and *creates* edges.
- **Calls** — callers and callees of a function, including events it raises and who handles them (needs the sidecar).
- **Timeline** — after a run, every function call as a bar over time (flame chart); double-click one to replay it.
- **▶ Run** — builds an instrumented copy of a .NET program and runs it: nodes count executions, show the values they produce and glow while running; **Replay** walks through a function step by step.
- **Blueprint** — the statements of one function as nodes, with exec wires, data wires for locals, plain-language labels, an inspector that explains each concept, and two-way sync with the code.

## Repository layout

| Path | Contents |
| :--- | :--- |
| `crates/nodex-ir` | Language-agnostic IR; generates TypeScript types into `web/src/gen` |
| `crates/nodex-lang-api` | `LanguageFrontend` trait and the lowering kit |
| `crates/nodex-lang-csharp` | C#/Unity frontend (tree-sitter) |
| `crates/nodex-core` | Project model, linker, concept catalog, canonical forms |
| `crates/nodex-trace` | Trace protocol decoder, span resolver, recorder (counts, values, replay timeline) |
| `crates/nodex-server` | `nodex` CLI, HTTP/WebSocket API, file watcher, semantic worker, run manager, embedded UI |
| `web/` | React + React Flow + ELK + Monaco UI |
| `catalog/` | Concept and flavor definitions (single source for code, UI and docs) |
| `fixtures/` | C#, Unity and Rosetta (same algorithm in C#/TS/Rust) test inputs |
| `sidecars/` | Planned .NET and Unity helpers (Roslyn semantics, trace runtime, IL weaver) |

## Development

```bash
cargo test --workspace
```

```bash
pnpm --dir web test
```

Tests that drive the real .NET sidecars (Roslyn, weaver) are ignored by default; run them after building both:

```bash
cargo test --workspace -- --ignored
```

UI development with hot reload — start the server allowing the Vite origin, then Vite:

```bash
cargo run -p nodex-server -- open fixtures/csharp/Sample --no-browser --dev-origin http://localhost:5173
```

```bash
pnpm --dir web dev
```

Open the "Dev UI" link the server prints. After changing IR types, run `cargo test -p nodex-ir -p nodex-core` to regenerate `web/src/gen`.
