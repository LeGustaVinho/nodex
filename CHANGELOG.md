# Changelog

All notable changes to Nodex are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

The release workflow publishes the section of the version it builds as the release notes, so write each entry for people who use Nodex.

## [Unreleased]

### Added

- Every panel can be resized: drag the edge of the sidebar, the right panel, the line between the Inspector and the code, the top of the Problems list or the top of the console. Double-click an edge to go back to the default size. Nodex keeps the sizes for the next time.

### Changed

- Large projects open much faster. A Unity project of 1,500 C# files shows its Overview in about 1 second instead of more than 10, and a project of 10,000 TypeScript files in about 2 seconds.
- A newly opened project starts on the Overview. Architecture groups namespaces and modules by folder when there are more than 300 of them.
- Opening a folder shows how many files have been found and read.
- C# semantic analysis (Roslyn) runs about twice as fast.
- Nodex skips more build and cache folders (`Builds`, `vendor`, `.yarn`, `.cache`, `Pods`, …) and the files a project's own `.gitignore` leaves out.

### Fixed

- The Architecture view of a project with many interdependent namespaces never finished laying out.
- After opening another project, the sidebar could keep showing the previous one.
- In a Unity project without the Nodex package, the code panel next to a Blueprint showed only its title.

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
