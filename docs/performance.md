# Performance: opening a project

Goal: the time from choosing a folder to seeing something useful, for a project Nodex has never
opened. This page records how it is measured and the numbers after each round of work.

## How to measure

```bash
cargo xtask bench-open <folder> [--runs N] [--semantic] [--slowest N]
```

It opens the folder in-process the way the server does, then prints one line per step:

- `open`: walk plus lowering;
- `structure + knowledge`: the first `/api/structure`;
- the size and serialization time of each payload;
- `time to first data`, the sum of the three.

The timings per language of `detect` come first. Stderr carries the finer `nodex_perf` lines
(`open.detect`, `open.walk`, `open.lower`, `structure.link`, `structure.semantic`,
`structure.knowledge`, `structure.merge`). The server writes the same lines to its log.

The other options:

- `--semantic` also runs the semantic providers (Roslyn, the TypeScript compiler,
  rust-analyzer) and times the rebuild with their facts.
- `--slowest N` lowers each file again on one thread and lists the N slowest.

### Synthetic projects

```bash
cargo xtask gen-corpus <out> --files 10000 --lang cs|ts|rs
```

It copies a sample fixture under distinct names until the project has about that many files.

### In the UI

The browser console, at verbose level, shows `[nodex perf]` lines, and DevTools → Performance
shows `nodex:` marks. They cover:

- `project-changed`;
- download and parse of `/api/structure` and `/api/knowledge`;
- `StructureIndex`, `architectureGraph` and the ELK layout;
- `first-view:<view>`, measured from `project-loading`.

## Reference projects

- **warcodes**: a Unity mobile game with 1,477 `.cs` files (6.5 MB) outside `Library/`, and a
  `.sln` with 36 projects.
- **cs10k / ts10k / rs10k**: `gen-corpus --files 10000` from the C#, TypeScript and Rust samples.

Measured on Windows 10 with a release build. Times are the second of two runs (warm file cache),
in milliseconds.

## Baseline (before any change)

| step | warcodes | cs10k | ts10k | rs10k |
|---|---:|---:|---:|---:|
| detect (all languages) | 1,632 | 2,462 | 1,764 | 1,321 |
| walk | 43 | 268 | 149 | 92 |
| lowering | 8,605 | 1,070 | 429 | 402 |
| structure + knowledge | 166 | 2,480 | 507 | 880 |
| structure JSON (serialize) | 28 | 111 | 55 | 81 |
| **time to first data** | **10,478** | **6,427** | **2,916** | **2,794** |
| structure payload | 20.9 MB | 118.1 MB | 55.1 MB | 69.5 MB |
| knowledge payload | 1.9 MB | 10.4 MB | 4.3 MB | 5.2 MB |

What each column shows:

- **warcodes**: one file dominates the lowering. `DOTweenAnimationInspector.cs`, 39 KB, takes
  7.6 s alone, because C# `prepare` re-parses it for each of its `#if` groups (81 parses of
  ~30 ms).
- **detect**: the Rust detector walks Unity's `Library/` twice. It skips no Unity folders and
  stats every entry.
- **cs10k**: `structure.link` takes 2,052 ms for 190k symbols.

Semantic analysis on warcodes (Roslyn, in the background after the open):

| step | time |
|---|---:|
| start (MSBuild loads the `.sln`) | 5,161 |
| analyze (30,540 refs) | 10,612 |
| rebuild of the structure with the facts | 1,016 (of which `structure.semantic` 856) |
