# Nodex

**Read code in any language as node graphs.**

Nodex turns C#, Unity, TypeScript/JavaScript and Rust projects into language-agnostic, Blueprint-style node graphs, UML diagrams and live execution traces. A programmer can then read code in a language they do not know. The same logic looks the same in every language: a C# `foreach`, a TypeScript `for..of` and a Rust `for x in iter` are one **For Each** node.

![A function as a Blueprint, with its inspector and code](docs/images/blueprint.png)

## Download

Get the installer from the [latest release](https://github.com/LeGustaVinho/nodex/releases/latest) (`Nodex_<version>_x64-setup.exe`, Windows 10 and 11) and run it. It installs for your user only, without administrator rights, and keeps itself up to date.

> The installer is not code-signed yet, so Windows may say it "protected your PC". Choose **More info → Run anyway**.

Then open a folder from the welcome screen, drop one on the window, or right-click a folder in Explorer and choose **Open with Nodex**. New to Nodex? Start with one of the sample projects on the welcome screen.

![The welcome screen](docs/images/welcome.png)

## What you need

Nodex shows every project's structure and graphs on its own. Each language's toolchain adds exact, compiler-grade analysis, ▶ Run with live traces, and debugging. The welcome screen and the Environment dialog show what is installed.

| Language | Install | It adds |
| :--- | :--- | :--- |
| C# (.NET) | [.NET SDK 10](https://dotnet.microsoft.com/download) or newer | Exact calls, events and types; Run and Debug |
| Unity | [Unity](https://unity.com/download) 2021.3 or newer (and the .NET SDK for exact C# analysis) | Play Mode traces and debugging |
| TypeScript / JavaScript | [Node.js](https://nodejs.org/en/download) | Exact calls and types; Run and Debug for Node and browser apps |
| Rust | [rustup](https://rustup.rs) with `rust-analyzer` | Exact calls and types; Run and Debug |

## What it shows

- **Overview**: entry points, HTTP routes, hotspots, data models and state machines.
- **Hierarchy**: folders, namespaces, types and members as boxes that open in place.
- **Knowledge**: front-end calls to back-end routes, dependency injection, tests, exceptions and data writes.
- **UML**: use case, component, class, sequence, activity, state and package diagrams, exported as Mermaid, SVG or PNG.
- **Architecture**, **Types** and **Calls**: how the code fits together.
- **Blueprint**: one function as nodes, with plain-language labels, an inspector that explains each concept, and two-way sync with the code.
- **▶ Run** and **Timeline**: run the program and watch the graphs light up, then replay any call step by step.
- **Debug**: breakpoints and stepping, in the code and in the graph.

![A sequence diagram drawn from the code](docs/images/uml.png)

The [guide](docs/guide.md) explains each view, Run and debugging, settings and troubleshooting.

## Privacy

Your code never leaves your computer. Nodex goes online only for two things: to check for updates, and to download a debug adapter when you click Install. Adapters are checked against a pinned hash, and updates are signed.

## Contributing

Bug reports and pull requests are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md). To build from source, run the tests or make the installer, see [docs/development.md](docs/development.md). Security issues: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE). Nodex includes third-party software under its own licenses, listed in the app under Help → Third-Party Licenses.
