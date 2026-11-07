# Sidecars

Helpers that run outside the Rust process. None exist yet; this folder reserves their place in the architecture.

| Planned path | Milestone | Purpose |
| :--- | :--- | :--- |
| `dotnet/Nodex.Roslyn/` | M3 | C# semantics through `MSBuildWorkspace` (call targets, types, inheritance), JSON-RPC over stdio |
| `dotnet/Nodex.Trace/` | M4 | Trace runtime (`netstandard2.1`, so Unity can load it): ring buffer + WebSocket sender |
| `dotnet/Nodex.Weaver/` | M4 | Mono.Cecil IL weaver guided by PDB sequence points; shared by `nodex run` and Unity |
| `unity/com.nodex.trace/` | M5 | UPM package: `ILPostProcessor` host for the weaver and Play Mode connection |

See the vault notes "Runtime Visualization Strategy" and "ADR-005 IL weaving for CSharp and Unity".
