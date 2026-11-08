# Contributing to Nodex

Thanks for helping. Bug reports, ideas and pull requests are all welcome.

## Reporting a problem

Open an issue with:

- what you did, what you expected and what happened;
- the Nodex version (Help → About Nodex) and Windows version;
- the language of the project you opened, and a small example if you can share one;
- the latest log file (Help → Open Logs Folder).

## Building from source

You need Rust (stable), Node.js with pnpm, and the .NET 10 SDK. See [docs/development.md](docs/development.md) for the details. In short:

```bash
pnpm --dir web install
```

```bash
pnpm --dir web build
```

```bash
cargo run -p nodex-desktop
```

## Before you open a pull request

- `cargo test --workspace` and `pnpm --dir web test` pass, and `pnpm --dir web typecheck` reports no errors.
- New behaviour has a test where it can have one.
- A change people will notice has a line under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md).
- After changing IR types, run `cargo test -p nodex-ir -p nodex-core` to regenerate `web/src/gen` and commit the result.

## Releasing (maintainers)

1. Set the version everywhere: `cargo xtask version 0.2.0`.
2. Move the `Unreleased` notes in `CHANGELOG.md` under the new version.
3. Commit, then tag and push: `git tag v0.2.0` and `git push origin v0.2.0`.

The release workflow builds the installer, signs the update and publishes the GitHub release; installed copies update themselves from it.
