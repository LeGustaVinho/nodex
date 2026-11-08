# Performance: opening a project

Goal: the time from choosing a folder to seeing something useful, for a project Nodex has never
opened. This page records how it is measured and the numbers after each round of work.

## How to measure

```bash
cargo xtask bench-open <folder> [--runs N] [--semantic] [--slowest N]
```

It opens the folder in-process the way the server does, through the lowering cache (see
[Reopening](#reopening-the-lowering-cache)). It removes the folder's cache first, so run 1 is a
cold open and later runs are warm reopens; `--no-cache` opens without it. Then it prints one line
per step:

- `open`: walk plus lowering;
- `structure + knowledge`: the first `/api/structure`;
- the size and serialization time of each payload;
- `time to first data`, the sum of the three.

The timings per language of `detect` come first. Stderr carries the finer `nodex_perf` lines
(`open.detect`, `open.walk`, `open.cache.load`, `open.lower`, `open.cache.save`, `structure.link`, `structure.semantic`,
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

## After the work (November 2026)

Same machine and projects, with a release build. The machine was busy, so times vary by about
±20% from run to run.

Since this round, the open respects the project's `.gitignore`. warcodes reads 1,447 files: its
28 `AgentScripts/` are git-ignored.

| step | warcodes | cs10k | ts10k | rs10k |
|---|---:|---:|---:|---:|
| walk (parallel) | 25 | 135 | 88 | 74 |
| detect (with lowering) | 96 | 999 | 563 | 464 |
| lowering | 795 | 982 | 464 | 462 |
| open (walk + max(detect, lowering)) | **820** | **1,141** | **658** | **548** |
| structure + knowledge | 180 | 1,348 | 582 | 731 |
| structure payload | 16.0 MB | 90.4 MB | 42.0 MB | 51.7 MB |
| **time to first data** | **1,025** | **2,591** | **1,289** | **1,343** |

The structure + knowledge build no longer delays the first screen the way it used to:

- the server builds it in the background as soon as the project is loaded, off the project lock;
- the Overview shows the summary counts at once;
- the Architecture view, which used to come first, never finished laying out on warcodes or on
  the 10k projects.

In the UI, measured from `project-loading`:

| project | before | after |
|---|---|---|
| warcodes | Architecture: never finished laying out (namespaces: 317 nodes, 1,555 edges) | Overview after about 1.0–1.2 s |
| ts10k | Architecture: 10,000 module nodes, never finished | Overview after about 1.8–2.1 s |

Semantic analysis on warcodes, which runs in the background and refreshes the graphs:

| step | before | after |
|---|---:|---:|
| Roslyn start (MSBuild loads the `.sln`) | 5,161 | about 5,700 (unchanged) |
| Roslyn analyze | 10,612 | 4,841 |
| rebuild with facts | 1,016 | about 230 |

### What made the difference

- **C# `#if` handling** (`nodex-lang-csharp/src/prepare.rs`):
  - each group of directives is tried once, not once per failing line;
  - candidates are parsed incrementally;
  - a file is parsed once when nothing needs rewriting.

  A 39 KB DOTween file went from 7.6 s to 0.75 s, with identical output on every file.
- **Detection**:
  - one shared list of ignored folders;
  - entry types read from the directory listing;
  - only the languages that have files;
  - run alongside lowering.
- **Linking**: maps instead of scans for type references, semantic edges, DI dispatch and tests.
- **Server**:
  - the graphs are built off the project lock, once, ahead of the first request;
  - each payload is serialized once;
  - `semantic-changed` no longer reloads everything when only a status changed.
- **UI**:
  - opens on the Overview;
  - ELK keeps the model order only for sparse graphs (it exploded on dense ones: 800 edges took
    73 s);
  - Architecture groups above 300 scopes;
  - a virtualized sidebar;
  - one shared `StructureIndex`;
  - Monaco loads only when needed.
- **Payload**: `/api/structure` leaves out the `Contains` edges (they repeat `parent`) and the
  default symbol fields, about 25% smaller.

### Not done yet

- **Lazy flow graphs.** Lowering builds every Blueprint up front, and knowledge reads them for
  complexity and throws.
- **Faster MSBuild loading for Unity solutions.** It takes about 5–6 s on warcodes.
- **Reusing rust-analyzer's `.scip` output** when nothing changed.

## Reopening: the lowering cache

Lowering was the main cost of an open, so reopening a project Nodex has opened before reads
the files that did not change back from disk instead of lowering them again
(`crates/nodex-core/src/cache.rs`).

- **Where**: one bundle per project, `<data dir>/cache/<hash of the root>/files.bin`. The data
  dir is `%LOCALAPPDATA%\io.github.legustavinho.nodex` on Windows.
- **Key**: each entry is keyed by its relative path, size and modification time. The whole
  bundle is keyed by a stamp of:
  - the Nodex version;
  - the embedded catalog;
  - the bundle layout;
  - the size and time of the running executable, so a development rebuild never reads a stale
    lowering.
- **Value**: the file's `FileIr` as MessagePack with named fields, LZ4-compressed, with a
  checksum. Named fields are needed because symbols leave their default fields out, which
  postcard or bincode cannot read back. The compression makes it about 6× smaller: field
  names and the file path in every span repeat a lot.
- **Open**: the bundle is read while the walk runs. Files whose size and time match are decoded
  instead of lowered, in parallel.
- **Write**: afterwards the bundle is rewritten in one atomic write (temporary file + rename),
  only if a file was lowered or one disappeared. Gone files are pruned. Files changed in the
  last 2 seconds are left out, because a second change within the timestamp resolution could
  keep the same size and time.
- **Safety**: a missing, truncated, damaged or old-format bundle, or a damaged entry, only means
  those files are lowered again. The open never fails because of the cache
  (`crates/nodex-core/tests/lowering_cache.rs`).

The server's open and `nodex <folder>` use the cache.

Measured with `cargo xtask bench-open <folder> --runs 2`, same machine, release build. Run 1 is
cold (the bench removes the folder's cache first) and run 2 is warm. Two repetitions were
within about 10% of each other. The structure and knowledge payload hashes are identical
between the cold and warm runs on all four projects.

| step | warcodes cold | warm | cs10k cold | warm | ts10k cold | warm | rs10k cold | warm |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| cache load | – | 5 | – | 26 | – | 12 | – | 14 |
| lowering (or reading back) | 752 | 41 | 963 | 464 | 462 | 90 | 437 | 117 |
| detect (alongside) | 35 | 44 | 1,032 | 275 | 503 | 188 | 442 | 138 |
| cache save | 20 | – | 126 | – | 61 | – | 65 | – |
| **open** | **796** | **61** | **1,322** | **608** | **658** | **277** | **579** | **214** |
| structure + knowledge | 121 | 122 | 1,215 | 1,015 | 460 | 406 | 564 | 504 |
| **time to first data** | **935** | **202** | **2,649** | **1,720** | **1,162** | **726** | **1,197** | **769** |
| bundle on disk | 18 MB | | 85 MB | | 43 MB | | 49 MB | |

What is left on a warm reopen:

- **The structure + knowledge build** is now most of the time to first data (it is not cached).
- **cs10k reads back slowly** (464 ms for 373 MB of decoded MessagePack). The time goes into the
  Windows heap allocator, not the format. A probe with mimalloc as the global allocator showed:
  - decode on 32 threads: 280 → 41 ms;
  - decompression: 120 → 13 ms;
  - freeing the decoded files: 1.4 s → 0.12 s.

  The same contention makes later opens in one process slower. Runs 3 and 4 of a
  `--runs 5` bench took about 2 s to read cs10k back, after a whole project had been freed.
  Switching the app's global allocator is a separate decision; it would speed up the cold
  open as well.
- **Detection** still runs on every open (275 ms on cs10k) and now outlasts the warm lowering on
  ts10k and rs10k.
- **Old projects' bundles** are never removed: each project opened once keeps its folder under
  `cache/` until the user deletes it.
