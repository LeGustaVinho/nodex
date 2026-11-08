# Knowledge fixtures

Inputs for the knowledge graph tests (`crates/nodex-core/tests/knowledge.rs`): HTTP routes, the
front-end code that calls them, dependency injection, configuration, data models, tests and
exception handling.

`fullstack/` was copied from CodeGraph (`fixtures/fullstack`, MIT, same author), without the
Python back end: an ASP.NET Core API (`backend-dotnet`) with its xUnit tests, a NestJS API
(`backend-nest`) and a React client (`frontend-react`) that calls both.
