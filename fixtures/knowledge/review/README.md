# Review fixture

Small cases for knowledge extraction bugs found in review: one file per case.

- `dotnet`: `[Route]` + `[HttpGet]` on one action, `[action]` in the controller route, a Minimal
  API handler declared in another file, a throw caught where it is thrown.
- `web`: an axios instance named `api` (a client, not a router).
- `server`: an Express route whose handler is imported from another module, Fastify route
  options (`{ handler }`), and a handler from a package (`controller.ping`).
- `nest-a`, `nest-b`: two NestJS apps with different global prefixes.
- `rust`: diesel writes without the `diesel::` prefix, an axum handler in another module.
