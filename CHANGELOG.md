# Changelog

All notable changes to Nodex are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

The release workflow publishes the section of the version it builds as the release notes, so write each entry for people who use Nodex.

## [Unreleased]

### Added

- The Rust Blueprint Guide explains how `?` in the first argument of `matches!` can return early before the macro produces a boolean.
- A Rust `map` callback explains at the start of its Blueprint when it runs and how the caller uses its return value.
- The Rust Inspector presents proven boolean `|=` and `&=` as assignments, without the numeric bitwise signature.
- Rust Blueprints show that boolean `|=` and `&=` still evaluate the right-hand expression when the destination already determines the result.
- The Rust Blueprint Guide shows how iterator `zip` forms pairs and what happens when one input has no item for the pair.
- The Rust Blueprint Guide shows what makes iterator `any` and `all` stop, including their results when there are no items to test.
- The Rust Blueprint Guide explains how `Peekable::peek` and `next_if` can leave the next item available for a later call.
- The Rust Blueprint Guide shows how iterator `find` and `position` stop at the first match and what each call returns.
- The Rust Blueprint Guide shows when `Arc::make_mut` can edit an existing value and when `make_mut` or `unwrap_or_clone` need to clone it.
- Rust Blueprints distinguish UTF-8 byte length from the number of elements in a vector or slice when Rust Analyzer resolves `len`.
- The Rust Blueprint Guide shows how `Vec::retain` and `retain_mut` test each element and what a `true` or `false` return means inside the test function.
- Rust Blueprints distinguish `str::split`, which yields borrowed text slices, from `split_once`, which returns a pair at the first match.
- The Rust Blueprint Guide explains how `Option::as_deref_mut` borrows a target for editing and lets you use the original option again when the borrow ends.
- The Rust Blueprint Guide explains what `Vec::drain` removes, what the iterator yields by value, and what happens when the iterator is dropped.
- The Rust Blueprint Guide names the argument, method, field, or operator that will use the value extracted by `?`.
- Rust Blueprints show how `RangeInclusive::contains` checks both included endpoints and the current bounds of a partially consumed range.
- The Rust Blueprint Guide distinguishes evaluating the default in `Option::map_or` from calling the fallback function in `map_or_else`.
- The Rust Blueprint Guide explains how `String::insert` changes a string at a UTF-8 byte offset, including insertion at the start.
- The Rust Blueprint Guide shows when a statement ending in `?;` ignores the successful value and continues.
- The Rust Blueprint Guide shows how `Option::and` evaluates both options and uses the first option to decide whether to return the second.
- The Rust Blueprint Guide explains that `NonZero::get` returns the stored integer, which is guaranteed to be nonzero.
- The Rust Blueprint Guide follows a successful `?` from the selected `if` branch or `match` arm into the expression that uses its value.
- Rust Blueprints show `mem::replace` as a value replacement when Rust Analyzer resolves it, without a text operation label.
- When a Rust block supplies the argument or receiver of a call, the Blueprint Guide shows that a successful `?` in the final expression still feeds that call.
- Rust Blueprints keep the `Option::replace` and `Path::join` calls distinct from text replacement and joining when Rust Analyzer resolves the methods.
- The Rust Blueprint Guide shows that `Option::or` evaluates its second option even when the first contains a value.
- The Rust Blueprint Guide shows how `OnceLock` reuses or initializes a stored value, lends it for editing, and becomes uninitialized after `take`.
- The Rust Blueprint Guide explains how `Option::flatten` removes one layer of optionality and how `Option::zip` pairs two present values.
- The Rust Blueprint Guide shows how `Option::ok_or` evaluates its error argument and turns `Some` into `Ok` or `None` into `Err`.
- The Rust Blueprint Guide keeps a successful `?` in the current node while its value is still needed by a call or another expression.
- The Rust Blueprint Guide explains when `Option::is_some_and` and `Option::is_none_or` call the test function and what each method returns for `None`.
- The Rust Blueprint Guide names the empty `Vec` or `IndexMap` installed by `mem::take` or returned by `unwrap_or_default` when Rust Analyzer confirms the collection type.
- Rust Blueprints explain the paths through `let` conditions and short-circuit operators, which bounds a range includes, and where borrowed values come from. The Guide shows the full explanations, and the Inspector highlights the Rust syntax each explanation describes.
- The Inspector keeps Rust declaration explanations visible across its tabs. Select a row to jump to the signature or type it describes.
- When Rust skips the rest of an iteration with `continue`, the Blueprint Guide can take you back to the right loop, including a labeled outer loop.
- Rust declarations explain which types a trait implementation chooses and when a returned reference shares a lifetime with an input. Open an associated type on a trait to find the definitions in its implementations.
- The Inspector highlights Rust `match` guards in assignments and other expressions. A guard runs after its pattern matches and can send execution on to the next alternative.
- The Rust Guide shows the generic arguments written in `collect::<...>()` and explains placeholders such as `Vec<_>` and `[u8; _]`.
- When Rust Analyzer confirms the standard iterator and types, the Guide explains how collecting optional items into `Option<Vec<_>>` stops at the first `None`.
- Rust Blueprints distinguish the type of a captured reference from how a closure or async block uses it. The Guide, Inspector and wires show when the access mode is uncertain.
- When a Rust closure's result expression uses `&` or `&mut`, its Blueprint explains that borrow and highlights the operator in the source.
- The Rust Inspector explains the exact `@`, `|`, `..` and `_` tokens in patterns. In a `match`, each explanation follows the arm that contains it.
- Rust Blueprints keep the final `if` or `match` inside a block's expression tree. The reading cards show which final expression gives a block, closure or future its result.
- Rust expression nodes label the values chosen by `if` and `match`. The Guide can show a `match` guard in the source before you follow its arm.
- Rust pattern cards show what `name @ pattern` tests and binds, which alternative supplies each name, and where a slice pattern takes its remaining items.
- The Rust Guide previews the value produced by each branch of an embedded `if` or `match`, including branches that transfer control before supplying a value.
- When rust-analyzer identifies the standard `matches!` macro, Rust Blueprints explain its boolean result and optional guard and offer to show the generated Rust.
- Rust Blueprints explain what happens when `?` exits a standard iterator callback. A missing value can send `find_map` to the next item or make `filter_map` skip the current one. A failure in `try_fold` stops the accumulation.
- When rust-analyzer confirms that a `try_fold` callback returns `Option`, the Guide shows when `?` makes the fold return `None`. A callback can still succeed with `Some(None)` if its accumulator is itself optional.
- When rust-analyzer confirms the standard `bool` methods, the Guide shows when `then` calls its closure and when `then_some` evaluates its argument, including the value each method returns for a false condition.
- Rust Blueprints highlight the `as` operator. When the numeric types are known, the Guide explains rounding, truncation, range limits, and changes in how the destination interprets the bits.
- For standard numeric conversions, the Rust Guide shows when `From` returns the destination value directly and when `TryFrom` returns `Ok` or `Err`. Follow the conversion separately from the `expect` or other call that handles its result.
- The Rust Guide explains where a mapped `break value` sends its result. It identifies the loop or labeled block that ends and shows when the value initializes a variable outside it.
- Rust casts explain the conversion after arithmetic when rust-analyzer confirms a standard numeric operator and its result type.
- The Rust Guide explains how `Result::ok` and `Result::err` keep one outcome in `Some` and turn the other into `None`, including when another call uses the resulting `Option`.
- The Rust Guide highlights the semicolon in `[value; length]` and explains how Rust determines the array length and evaluates the repeated value.
- The Rust Guide highlights `..base` in a struct expression and explains when Rust evaluates the base and how it fills the omitted fields.
- The Rust Guide shows when `array::from_fn` calls a function for each index and when `iter::from_fn` waits for a request for the next item.
- In a recognized `tokio::select!` inside a loop, the Rust Guide explains when a branch's direct `break` leaves the loop or `continue` starts the next iteration.
- The Rust Guide explains when a standard channel's `recv` blocks or reports disconnection, and how `Result::is_ok` turns its outcome into a boolean.
- Open a separate Rust Blueprint for a callback or local closure whose body is directly a loop, including a `while` that waits for channel messages.
- From a Rust `break`, the Guide can highlight the first mapped statement after the loop without advancing the execution path.
- The Rust Guide highlights the semicolon in the standard `vec![value; length]` macro and explains evaluation, cloning, and the zero-length case.
- When rust-analyzer resolves a numeric type alias in a cast, the Rust Guide shows the alias alongside its primitive type and explains the conversion.
- The Rust Guide explains that `VecDeque::pop_front` removes and returns the first queued item, or returns `None` when the queue is empty.
- The Rust Guide shows what `mem::take` and `mem::replace` store at a mutable reference and what previous value they return.
- The Rust Guide distinguishes replacing an `Option` from filling it only when it is empty, and explains when `unwrap_or_default` computes a default value.
- The Rust Guide explains the pattern check on each `while let` attempt and what makes the loop stop. A successful `?` in the condition keeps checking the condition.
- The Rust Guide explains when `Option::take_if` runs its test, when the value moves out, and when changes made by the test stay in the option.
- A successful Rust `?` inside a condition, the value tested by `match`, or the source of a `for` loop stays in that expression in the Guide. The early return is shown separately.
- In Rust `let ... else`, the Guide separates an early return from `?` in the initializer from the pattern check that determines whether the `else` block runs.

- Blueprint badges identify values, references, and pointers. Select a syntax badge for the exact source and examples, or a Rust reference for its owner and known call sites.

- The **Context** picker limits six explorer views to one or more folders and symbols. Nodex remembers the selection for each project and dims Knowledge neighbors outside it.
- **Open as Blueprint** on a type shows its fields and method cards in one box. Expand methods in place to follow calls and field writes.

- Every panel can be resized: drag the edge of the sidebar, the right panel, the line between the Inspector and the code, the top of the Problems list or the top of the console. Double-click an edge to go back to the default size. Nodex keeps the sizes for the next time.
- Hierarchy and Knowledge explore like CodeGraph's explorer. Select a node to light up its relations, or show only its relations, incoming or outgoing (key E). A filters panel turns each relation and confidence on or off and hides third-party code. Hovering a node or a wire shows its evidence. Each view has a bar with breadcrumbs, Back, counts, spacing, fit (F) and PNG export.
- Knowledge layouts: layers from the focus (callers on the left, callees on the right), force-directed, concentric, circle and grid. Nodes can be grouped in class, file or namespace boxes.
- Hierarchy: open one more level everywhere, open everything inside a box, hide the rolled-up links. Folders and namespaces can be selected and show what they hold. A box keeps its place on screen when it opens.
- The Inspector of a symbol has tabs: Overview, Evidence (filtered by confidence), Flows (from the front end to the data written, callers and callees, where errors are caught), Data (primary and foreign keys, one or many) and Actions (impact radius, tests, file skeleton, copy, open in VS Code).
- The Overview lists every entry point, route, hotspot, test, data model and unresolved relation, with a filter, and shows how sure Nodex is of the relations and the files per language.
- The palette has more commands (fit, export, edges shown, layouts, lists, reload the project) and highlights what matches.

### Changed

- Blueprints explain TypeScript spread and rest, resource cleanup and recognized collection stages. Use **Guide** to follow possible paths. Replay highlights a transition when the recording identifies its execution wire. Type Blueprints also show contracts and enum variants.

- In **Blueprint**, clicking a file with a clear main type opens that type's graph in C#, TypeScript and Rust. TypeScript and Rust files can also open a module graph with their free functions directly from **Files**.

- Large projects open much faster. A Unity project of 1,500 C# files shows its Overview in about 1 second instead of more than 10, and a project of 10,000 TypeScript files in about 2 seconds.
- A newly opened project starts on the Overview. Architecture groups namespaces and modules by folder when there are more than 300 of them.
- Opening a folder shows how many files have been found and read.
- C# semantic analysis (Roslyn) runs about twice as fast.
- Nodex skips more build and cache folders (`Builds`, `vendor`, `.yarn`, `.cache`, `Pods`, …) and the files a project's own `.gitignore` leaves out.

### Fixed

- The Architecture view of a project with many interdependent namespaces never finished laying out.
- After opening another project, the sidebar could keep showing the previous one.
- In a Unity project without the Nodex package, the code panel next to a Blueprint showed only its title.
- The minimap of the graph views was drawn at the top left, over the graph, instead of the bottom right.

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
