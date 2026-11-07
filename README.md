# Nodex

Nodex turns source code into language-agnostic, Blueprint-style node graphs so a programmer can read code in a language they do not know.
The same logic looks the same in every language: a C# `foreach`, a TypeScript `for..of` and a Rust `for x in iter` are one **For Each** node.

**Status**: milestones M0–M2 — C# (plain .NET and Unity) structure and Blueprints. Live execution, TypeScript and Rust are on the roadmap.
Full documentation lives in the Obsidian vault at `D:\MEGA\Obsidian\Default\Nodex` (start at `Nodex.md`).

## Quick start

```bash
pnpm --dir web install
```

```bash
pnpm --dir web build
```

```bash
cargo run --release -p nodex-server -- open fixtures/csharp/Sample
```

The server prints a link with a session token and opens it in your browser.

## Views

- **Architecture** — namespaces, imported namespaces and packages.
- **Types** — classes, interfaces and records, with *extends*, *implements*, *uses* and *creates* edges.
- **Blueprint** — the statements of one function as nodes, with exec wires, data wires for locals, plain-language labels, an inspector that explains each concept, and two-way sync with the code.

## Repository layout

| Path | Contents |
| :--- | :--- |
| `crates/nodex-ir` | Language-agnostic IR; generates TypeScript types into `web/src/gen` |
| `crates/nodex-lang-api` | `LanguageFrontend` trait and the lowering kit |
| `crates/nodex-lang-csharp` | C#/Unity frontend (tree-sitter) |
| `crates/nodex-core` | Project model, linker, concept catalog, canonical forms |
| `crates/nodex-server` | `nodex` CLI, HTTP/WebSocket API, file watcher, embedded UI |
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

UI development with hot reload — start the server allowing the Vite origin, then Vite:

```bash
cargo run -p nodex-server -- open fixtures/csharp/Sample --no-browser --dev-origin http://localhost:5173
```

```bash
pnpm --dir web dev
```

Open the "Dev UI" link the server prints. After changing IR types, run `cargo test -p nodex-ir -p nodex-core` to regenerate `web/src/gen`.
