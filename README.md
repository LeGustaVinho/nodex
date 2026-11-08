# Nodex

Nodex turns source code into language-agnostic, Blueprint-style node graphs so a programmer can read code in a language they do not know.
The same logic looks the same in every language: a C# `foreach`, a TypeScript `for..of` and a Rust `for x in iter` are one **For Each** node.

**Status**: milestones M0–M4.1 — C# (plain .NET and Unity) structure, Blueprints, Roslyn semantics with a Calls view, and live execution traces for .NET programs with replay, exceptions and a call Timeline. Unity Play Mode traces, TypeScript and Rust are on the roadmap. M9 adds a knowledge graph for C#, TypeScript/JavaScript and Rust — entry points, HTTP routes and the front-end calls that reach them, dependency injection, tests, exceptions and data models — ported from CodeGraph. It also reads authorization (roles, policies, anonymous endpoints), mounted routers (Express `app.use`, axum `nest`, actix `scope`, Rocket `mount`), commands, background jobs, message consumers, UI handlers and pages, constructor injection, and state machines declared with XState, Stateless or a Unity Animator.
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

Or run the desktop app, which hosts the same server and UI in its own window (open a folder with File → Open Folder…, by dropping it on the window, or by passing it as an argument):

```bash
cargo run --release -p nodex-desktop
```

## Views

- **Overview** — what the project is: entry points (program, routes, commands, jobs, message consumers, UI), HTTP routes, hotspots (cyclomatic complexity), data models, declared state machines, and relations by kind.
- **Hierarchy** — folders and namespaces, modules, types and members as boxes that open in place; relations roll up onto the visible boxes.
- **Knowledge** — HTTP calls from front-end code to back-end routes, dependency injection (registrations and constructor injection), tests, thrown and caught exceptions, and data writes, around one symbol or for the whole project. Relations no compiler proves are drawn dashed (inferred) or dotted red (unresolved).
- **UML** — the seven UML diagrams (Mermaid) of the project, a folder, a file, a symbol or the selected code: members with UML visibility, inheritance and realization, associations with multiplicities, composition and aggregation, entities and their keys, controllers and DI services as stereotypes; packages with coupling metrics and dependency cycles; components with the HTTP calls between front end and back end, provided interfaces, databases, caches and external APIs. Right-click a folder, a file, a symbol or a Hierarchy box, use the Inspector, the palette, or Ctrl+Shift+U in the code. Activity diagrams show how a function runs (decisions, loops with their way back, try/catch, fork/join, the HTTP requests and data changes it makes, calls expanded in place on demand); sequence diagrams follow a function's calls through other types and modules, across HTTP from front end to back end, through dependency injection to the implementation, down to the database, with alt/opt/loop/par/critical fragments. A wider scope lists its functions to start from (entry points first). State diagrams come from declared machines (XState, Stateless, Unity Animator) and from the code: enum, literal and flag fields, a stored entity's status changed by services (triggered by the route that reaches them), React `useState`, the State pattern, a local driving a loop and Rust type-states, with source states read from guard clauses, `switch` cases and helper guards. Use case diagrams show the entry points by component with their actors (roles from `[Authorize]`, `@Roles` or `requireRole`, visitors, operators, schedulers, brokers), «include» and «extend» from shared operations, and UI actions that include the API use case they call. After a run, a sequence diagram can draw the recorded calls of any call picked in the Timeline (arguments, return values, exceptions, repeated calls folded into loops), and activity and sequence diagrams mark what ran. Click an element to see why it is drawn; export Mermaid, Markdown, SVG or PNG. 
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
| `crates/nodex-core` | Project model, linker, concept catalog, canonical forms, knowledge graph (routes, HTTP links, DI, tests, caught exceptions) |
| `crates/nodex-uml` | UML diagrams (use case, component, class, sequence, activity, state, package) built from the structure, knowledge and flow graphs and recorded runs, as Mermaid text |
| `crates/nodex-trace` | Trace protocol decoder, span resolver, recorder (counts, values, replay timeline, call tree with callers and call sites) |
| `crates/nodex-server` | `nodex` CLI, HTTP/WebSocket API, file watcher, semantic worker, run manager, embedded UI |
| `crates/nodex-desktop` | Desktop app (Tauri): the server in-process and the UI in a native window, menu, single instance |
| `web/` | React + React Flow + ELK + Monaco UI |
| `catalog/` | Concept and flavor definitions (single source for code, UI and docs) |
| `fixtures/` | C#, Unity and Rosetta (same algorithm in C#/TS/Rust) test inputs |
| `sidecars/` | Planned .NET and Unity helpers (Roslyn semantics, trace runtime, IL weaver) |

Press **Ctrl+K** (or `/`) anywhere to search symbols and views.

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
