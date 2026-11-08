#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using Mono.Cecil;
using Mono.Cecil.Cil;
using Unity.CompilationPipeline.Common.Diagnostics;
using Unity.CompilationPipeline.Common.ILPostProcessing;

namespace Nodex.Weaver
{
    /// <summary>
    /// Adds Nodex trace probes to the project's own script assemblies while Nodex asks for them
    /// (<c>Library/Nodex/trace-on</c> exists), with the same weaver as .NET programs. Writes each
    /// assembly's trace map to <c>Library/Nodex/maps/&lt;assembly&gt;.json</c>. Never fails a
    /// compilation: a weaving error becomes a warning and the assembly stays as compiled.
    /// </summary>
    public sealed class NodexPostProcessor : ILPostProcessor
    {
        public override ILPostProcessor GetInstance() => this;

        public override bool WillProcess(ICompiledAssembly compiledAssembly)
        {
            var root = ProjectRoot();
            return root != null && File.Exists(Path.Combine(root, "Library", "Nodex", "trace-on")) && Weavable(compiledAssembly.Name);
        }

        public override ILPostProcessResult Process(ICompiledAssembly compiledAssembly)
        {
            var diagnostics = new List<DiagnosticMessage>();
            var root = ProjectRoot();
            if (root == null) return null!;
            var mapPath = Path.Combine(root, "Library", "Nodex", "maps", compiledAssembly.Name + ".json");
            try
            {
                using var resolver = new Resolver(compiledAssembly);
                var parameters = new ReaderParameters
                {
                    SymbolStream = new MemoryStream(compiledAssembly.InMemoryAssembly.PdbData),
                    SymbolReaderProvider = new PortablePdbReaderProvider(),
                    AssemblyResolver = resolver,
                    ReadingMode = ReadingMode.Immediate,
                    ReadSymbols = true,
                };
                using var assembly = AssemblyDefinition.ReadAssembly(new MemoryStream(compiledAssembly.InMemoryAssembly.PeData), parameters);
                resolver.Self = assembly;
                var module = assembly.MainModule;
                var methods = new List<MethodMap>();
                // Only the project's own scripts: packages are inside the project folder too.
                var sources = new SourceMaps(root, rel => rel.StartsWith("Assets/", StringComparison.OrdinalIgnoreCase));
                var weaver = new ModuleWeaver(module, TraceProbes.ByName(module), sources, methods, IdBase(compiledAssembly.Name));
                if (!weaver.Weave())
                {
                    if (File.Exists(mapPath)) File.Delete(mapPath);
                    return null!;
                }
                var pe = new MemoryStream();
                var pdb = new MemoryStream();
                assembly.Write(pe, new WriterParameters { SymbolWriterProvider = new PortablePdbWriterProvider(), SymbolStream = pdb, WriteSymbols = true });
                Directory.CreateDirectory(Path.GetDirectoryName(mapPath)!);
                File.WriteAllText(mapPath, MapJson(methods), new UTF8Encoding(false));
                return new ILPostProcessResult(new InMemoryAssembly(pe.ToArray(), pdb.ToArray()), diagnostics);
            }
            catch (Exception e)
            {
                diagnostics.Add(new DiagnosticMessage
                {
                    DiagnosticType = DiagnosticType.Warning,
                    MessageData = $"Nodex: {compiledAssembly.Name} was not instrumented: {e.Message}",
                    File = "",
                    Line = 0,
                    Column = 0,
                });
                return new ILPostProcessResult(compiledAssembly.InMemoryAssembly, diagnostics);
            }
        }

        /// The project's own script assemblies: not Unity's, not packages', not Editor-only ones.
        internal static bool Weavable(string name)
        {
            string[] skipPrefixes = { "Unity.", "UnityEngine", "UnityEditor", "Nodex.", "System.", "Mono.", "Microsoft.", "netstandard", "mscorlib" };
            if (skipPrefixes.Any(p => name.StartsWith(p, StringComparison.Ordinal))) return false;
            if (name.EndsWith("Editor", StringComparison.Ordinal) || name.Contains(".Editor") || name.Contains("-Editor")) return false;
            if (name.EndsWith(".Tests", StringComparison.Ordinal) || name.EndsWith(".CodeGen", StringComparison.Ordinal)) return false;
            return true;
        }

        /// Each assembly's method ids start at its own base (a 16-bit FNV-1a hash of its name,
        /// shifted left 15 bits), so the maps of separately woven assemblies do not collide.
        internal static int IdBase(string name)
        {
            uint h = 2166136261;
            foreach (var c in name)
            {
                h ^= c;
                h *= 16777619;
            }
            return (int)((h & 0xFFFF) << 15);
        }

        /// The Unity project folder: the IL post-processor runs with it as working directory.
        private static string? ProjectRoot()
        {
            var dir = new DirectoryInfo(Environment.CurrentDirectory);
            while (dir != null)
            {
                if (Directory.Exists(Path.Combine(dir.FullName, "ProjectSettings")) && Directory.Exists(Path.Combine(dir.FullName, "Assets"))) return dir.FullName;
                dir = dir.Parent;
            }
            return null;
        }

        private static string MapJson(List<MethodMap> methods)
        {
            var sb = new StringBuilder("{\"version\":1,\"methods\":[");
            for (var i = 0; i < methods.Count; i++)
            {
                var m = methods[i];
                if (i > 0) sb.Append(',');
                sb.Append("{\"id\":").Append(m.Id).Append(",\"name\":").Append(Str(m.Name)).Append(",\"kind\":").Append(Str(m.Kind)).Append(",\"points\":[");
                for (var p = 0; p < m.Points.Count; p++)
                {
                    var pt = m.Points[p];
                    if (p > 0) sb.Append(',');
                    sb.Append("{\"file\":").Append(Str(pt.File)).Append(",\"start\":").Append(pt.Start).Append(",\"end\":").Append(pt.End).Append('}');
                }
                sb.Append("],\"locals\":{");
                var first = true;
                foreach (var kv in m.Locals)
                {
                    if (!first) sb.Append(',');
                    first = false;
                    sb.Append(Str(kv.Key.ToString(System.Globalization.CultureInfo.InvariantCulture))).Append(':').Append(Str(kv.Value));
                }
                sb.Append("},\"params\":[").Append(string.Join(",", m.Params.Select(Str))).Append("]}");
            }
            return sb.Append("]}").ToString();
        }

        private static string Str(string s)
        {
            var sb = new StringBuilder("\"");
            foreach (var c in s)
            {
                switch (c)
                {
                    case '"': sb.Append("\\\""); break;
                    case '\\': sb.Append("\\\\"); break;
                    case '\n': sb.Append("\\n"); break;
                    case '\r': sb.Append("\\r"); break;
                    case '\t': sb.Append("\\t"); break;
                    default:
                        if (c < ' ') sb.Append("\\u").Append(((int)c).ToString("x4")); else sb.Append(c);
                        break;
                }
            }
            return sb.Append('"').ToString();
        }

        /// Resolves the compiled assembly's references from their paths (read into memory, so
        /// no file stays locked for Unity's next compilation).
        private sealed class Resolver : IAssemblyResolver
        {
            private readonly Dictionary<string, string> _paths = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            private readonly Dictionary<string, AssemblyDefinition> _cache = new Dictionary<string, AssemblyDefinition>(StringComparer.OrdinalIgnoreCase);
            public AssemblyDefinition? Self;

            public Resolver(ICompiledAssembly compiled)
            {
                foreach (var r in compiled.References) _paths[Path.GetFileNameWithoutExtension(r)] = r;
            }

            public AssemblyDefinition Resolve(AssemblyNameReference name) => Resolve(name, new ReaderParameters());

            public AssemblyDefinition Resolve(AssemblyNameReference name, ReaderParameters parameters)
            {
                if (Self != null && name.Name == Self.Name.Name) return Self;
                if (_cache.TryGetValue(name.Name, out var cached)) return cached;
                if (!_paths.TryGetValue(name.Name, out var path) || !File.Exists(path)) throw new AssemblyResolutionException(name);
                var assembly = AssemblyDefinition.ReadAssembly(new MemoryStream(File.ReadAllBytes(path)), new ReaderParameters { AssemblyResolver = this, ReadingMode = ReadingMode.Deferred });
                _cache[name.Name] = assembly;
                return assembly;
            }

            public void Dispose()
            {
                foreach (var a in _cache.Values) a.Dispose();
                _cache.Clear();
            }
        }
    }
}
