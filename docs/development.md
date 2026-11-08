# Developing Nodex

How to build Nodex from source, run its tests, and produce the Windows installer. For using Nodex, see the [README](../README.md) and the [guide](guide.md). Design notes live in the Obsidian vault at `D:\MEGA\Obsidian\Default\Nodex` (start at `Nodex.md`).

## Requirements

- Rust (stable), with the `rust-analyzer` component for Rust semantics.
- Node.js 24 with pnpm.
- The .NET 10 SDK, for the C# helpers.

## Run from source

Build the web UI (the server embeds `web/dist`):

```bash
pnpm --dir web install
```

```bash
pnpm --dir web build
```

Optional, for exact C# calls, events and `var` types, and for ▶ Run with live traces of .NET programs:

```bash
dotnet build sidecars/dotnet/Nodex.Roslyn -c Release
```

```bash
dotnet build sidecars/dotnet/Nodex.Weaver -c Release
```

Optional, for TypeScript semantics and traces:

```bash
pnpm --dir sidecars/node/nodex-ts install
```

Then either the desktop app (open a folder from File → Open Folder…, by dropping it on the window, or as an argument):

```bash
cargo run -p nodex-desktop
```

or the command line, which prints a link with a session token and opens it in your browser:

```bash
cargo run --release -p nodex-server -- open fixtures/csharp/Sample
```

A build in the repository finds the sidecars in the source tree; an installed one finds them in its resource folder.

## Tests

```bash
cargo test --workspace
```

```bash
pnpm --dir web test
```

```bash
pnpm --dir web typecheck
```

Tests that drive the real sidecars, toolchains or debug adapters are ignored by default; run them after building the sidecars:

```bash
cargo test --workspace -- --ignored
```

After changing IR types, run `cargo test -p nodex-ir -p nodex-core` to regenerate `web/src/gen`, and commit the result.

## UI development with hot reload

Start the server allowing the Vite origin, then Vite, and open the "Dev UI" link the server prints:

```bash
cargo run -p nodex-server -- open fixtures/csharp/Sample --no-browser --dev-origin http://localhost:5173
```

```bash
pnpm --dir web dev
```

## The installer

`cargo xtask dist` builds the web UI, publishes the .NET sidecars, stages the TypeScript sidecar with its compiler, the trace runtimes, the samples, the license notices, and runs `cargo tauri build`. It needs pnpm, npm, the .NET SDK and the Tauri CLI:

```bash
cargo install tauri-cli --version "^2" --locked
```

```bash
cargo xtask dist
```

The installer lands in `target/release/bundle/nsis/`. It installs per user in `%LOCALAPPDATA%\Nodex` (no administrator rights) and adds "Open with Nodex" to folders in Explorer. Nodex keeps its settings in `%APPDATA%\io.github.legustavinho.nodex` and its logs, debug adapters and sample copies in `%LOCALAPPDATA%\io.github.legustavinho.nodex`.

Other tasks:

- `cargo xtask dist --no-bundle` stages the resources in `target/nodex-dist/resources` without building the installer.
- `cargo xtask licenses` (after staging) regenerates `THIRD-PARTY-NOTICES.txt`.
- `cargo xtask version 0.2.0` sets the version everywhere; `cargo xtask notes 0.2.0` prints its CHANGELOG section.

Releases are built by the GitHub workflow on a `v*` tag; see [CONTRIBUTING.md](../CONTRIBUTING.md#releasing-maintainers).

## Repository layout

| Path | Contents |
| :--- | :--- |
| `crates/nodex-ir` | Language-agnostic IR; generates TypeScript types into `web/src/gen` |
| `crates/nodex-lang-api` | `LanguageFrontend` trait, the lowering kit, sidecar and process helpers |
| `crates/nodex-lang-csharp` | C#/Unity frontend (tree-sitter), Roslyn and weaver drivers, Unity Editor bridge |
| `crates/nodex-lang-typescript` | TypeScript/JavaScript frontend and its sidecar driver |
| `crates/nodex-lang-rust` | Rust frontend, rust-analyzer SCIP semantics, trace instrumentation |
| `crates/nodex-core` | Project model, linker, concept catalog, canonical forms, knowledge graph |
| `crates/nodex-uml` | UML diagrams built from the structure, knowledge and flow graphs and recorded runs, as Mermaid text |
| `crates/nodex-trace` | Trace protocol decoder, span resolver, recorder |
| `crates/nodex-dap` | Debug Adapter Protocol client |
| `crates/nodex-server` | `nodex` CLI, HTTP/WebSocket API, file watcher, semantic workers, run and debug managers, embedded UI |
| `crates/nodex-desktop` | Desktop app (Tauri): the server in-process, the UI in a native window, menu, updates, installer configuration |
| `xtask/` | Build tasks (`cargo xtask dist`, `version`, `notes`, `licenses`) |
| `web/` | React + React Flow + ELK + Monaco + Mermaid UI |
| `catalog/` | Concept and flavor definitions (single source for code, UI and docs) |
| `sidecars/` | Roslyn semantics, IL weaver, trace runtimes (.NET, Node, Rust), TypeScript sidecar, Unity package |
| `fixtures/` | Test inputs: C#, Unity, TypeScript, Rust, full-stack and Rosetta (one algorithm in three languages) |
