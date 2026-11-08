# Knowledge fixtures

Inputs for the knowledge graph tests (`crates/nodex-core/tests/knowledge.rs`): HTTP routes, the
front-end code that calls them, dependency injection, configuration, data models, tests and
exception handling.

`fullstack/` was copied from CodeGraph (`fixtures/fullstack`, MIT, same author), without the
Python back end: an ASP.NET Core API (`backend-dotnet`) with its xUnit tests, a NestJS API
(`backend-nest`) and a React client (`frontend-react`) that calls both.

`auth/` checks authorization (ASP.NET controllers and Minimal API groups, NestJS guards and
roles, Express middlewares), mounted Express routers and constructor injection.

`cli-jobs/` checks the entry points that are not HTTP routes: System.CommandLine, commander,
clap subcommands, `BackgroundService`, Hangfire, `setInterval`, node-cron, MassTransit, KafkaJS
and amqplib, plus nested axum routers, an actix scope and a Rocket mount.

`uml-states/` checks declared state machines (XState, Stateless, a Unity Animator) and UI entry
points (React click handlers, React Router routes, Next.js pages).

`rust/` was copied from CodeGraph (`fixtures/edge-cases/rust`): an axum service with diesel
entities, environment reads, a reqwest call and unit and integration tests.

`uml-states/` also has one example per state detector of the UML view: an enum field with guard
clauses, a `switch` and a helper guard (`csharp/Order.cs`), boolean flags (`Connection.cs`), the
State pattern (`TrafficLight.cs`), a local that drives a loop (`Lexer.cs`), React `useState`
(`ts/src/useUpload.ts`), and a Rust enum field plus a type-state (`rust/src/lib.rs`).
