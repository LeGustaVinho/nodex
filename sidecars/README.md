# Sidecars

Helpers that run outside the Rust process.

| Path | Milestone | Purpose |
| :--- | :--- | :--- |
| `dotnet/Nodex.Roslyn/` | M3 ✅ | C# semantics through `MSBuildWorkspace` or loose files: calls, creations, bases, events, `var` types. Line-delimited JSON over stdio. Build: `dotnet build sidecars/dotnet/Nodex.Roslyn -c Release` |
| `dotnet/Nodex.Trace/` | M4 ✅ | Trace runtime (`netstandard2.1`, so Unity can load it): buffered binary events over loopback TCP; no-op unless started by Nodex |
| `dotnet/Nodex.Weaver/` | M4 ✅ | Mono.Cecil IL weaver guided by PDB sequence points; writes the instrumented copy and `nodex-trace-map.json`. Build: `dotnet build sidecars/dotnet/Nodex.Weaver -c Release` |
| `unity/com.nodex.trace/` | M5 | UPM package: `ILPostProcessor` host for the weaver and Play Mode connection |

See the vault notes "Runtime Visualization Strategy" and "ADR-005 IL weaving for CSharp and Unity".
