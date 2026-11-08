# Changelog

All notable changes to Nodex are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

The release workflow publishes the section of the version it builds as the release notes, so write each entry for people who use Nodex.

## [Unreleased]

## [0.1.0] - 2026-11-08

First public release.

### Added

- Desktop app for Windows: install it, open a folder and read the code. No command line, no browser.
  - Welcome screen with recent folders, sample projects and the toolchains found on the computer.
  - Native menu, folder picker and Save dialog; drop a folder on the window or use "Open with Nodex" in Explorer.
  - Automatic updates from GitHub Releases.
- Code as language-agnostic, Blueprint-style node graphs for C# (plain .NET and Unity), TypeScript/JavaScript and Rust.
- Views: Overview, Hierarchy, Knowledge, UML (use case, component, class, sequence, activity, state, package), Architecture, Types, Calls, Timeline and Blueprint.
- Exact semantics with Roslyn, the TypeScript compiler and rust-analyzer when their toolchains are installed.
- ▶ Run with live execution traces and replay for .NET, Node, browser apps, Rust and Unity Play Mode.
- Debugging through the Debug Adapter Protocol, with adapters downloaded on demand.

[Unreleased]: https://github.com/LeGustaVinho/nodex/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LeGustaVinho/nodex/releases/tag/v0.1.0
