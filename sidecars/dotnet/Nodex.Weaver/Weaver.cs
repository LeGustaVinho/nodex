using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Mono.Cecil;
using Mono.Cecil.Cil;
using Mono.Cecil.Rocks;

namespace Nodex.Weaver;

public sealed record PointMap(string File, int Start, int End);

public sealed record MethodMap(int Id, string Name, string Kind, List<PointMap> Points, Dictionary<int, string> Locals, List<string> Params);

public sealed record TraceMap(int Version, List<MethodMap> Methods);

public sealed record Summary(List<string> Assemblies, int Methods, string Map);

public static class Weaver
{
    public static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower };

    public static Summary Run(string root, string input, string output)
    {
        CopyDirectory(input, output);
        var traceDll = Path.Combine(AppContext.BaseDirectory, "Nodex.Trace.dll");
        File.Copy(traceDll, Path.Combine(output, "Nodex.Trace.dll"), overwrite: true);

        var resolver = new DefaultAssemblyResolver();
        resolver.AddSearchDirectory(input);
        resolver.AddSearchDirectory(Path.GetDirectoryName(typeof(object).Assembly.Location)!);
        var trace = AssemblyDefinition.ReadAssembly(traceDll);

        var methods = new List<MethodMap>();
        var woven = new List<string>();
        var sources = new SourceMaps(root);
        foreach (var dll in Directory.EnumerateFiles(input, "*.dll"))
        {
            var pdb = Path.ChangeExtension(dll, ".pdb");
            if (!File.Exists(pdb) || Path.GetFileName(dll) == "Nodex.Trace.dll") continue;
            var target = Path.Combine(output, Path.GetFileName(dll));
            using var module = ModuleDefinition.ReadModule(dll, new ReaderParameters
            {
                ReadSymbols = true,
                AssemblyResolver = resolver,
                ReadingMode = ReadingMode.Immediate,
            });
            var weaver = new ModuleWeaver(module, trace.MainModule, sources, methods);
            if (!weaver.Weave()) continue;
            module.Write(target, new WriterParameters { WriteSymbols = true });
            woven.Add(Path.GetFileName(dll));
        }

        foreach (var deps in Directory.EnumerateFiles(output, "*.deps.json")) RegisterTraceRuntime(deps);

        var mapPath = Path.Combine(output, "nodex-trace-map.json");
        File.WriteAllText(mapPath, JsonSerializer.Serialize(new TraceMap(1, methods), Json));
        return new Summary(woven, methods.Count, mapPath);
    }

    /// .NET apps only load assemblies listed in their deps.json; add Nodex.Trace to it.
    private static void RegisterTraceRuntime(string depsPath)
    {
        var deps = JsonNode.Parse(File.ReadAllText(depsPath))!.AsObject();
        const string key = "Nodex.Trace/1.0.0";
        if (deps["targets"] is JsonObject targets)
        {
            foreach (var (_, target) in targets)
            {
                if (target is JsonObject t && !t.ContainsKey(key))
                {
                    t[key] = new JsonObject { ["runtime"] = new JsonObject { ["Nodex.Trace.dll"] = new JsonObject() } };
                }
            }
        }
        var libraries = deps["libraries"] as JsonObject ?? new JsonObject();
        deps["libraries"] = libraries;
        if (!libraries.ContainsKey(key))
        {
            libraries[key] = new JsonObject { ["type"] = "project", ["serviceable"] = false, ["sha512"] = "" };
        }
        File.WriteAllText(depsPath, deps.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
    }

    private static void CopyDirectory(string from, string to)
    {
        Directory.CreateDirectory(to);
        foreach (var dir in Directory.EnumerateDirectories(from, "*", SearchOption.AllDirectories))
        {
            Directory.CreateDirectory(Path.Combine(to, Path.GetRelativePath(from, dir)));
        }
        foreach (var file in Directory.EnumerateFiles(from, "*", SearchOption.AllDirectories))
        {
            File.Copy(file, Path.Combine(to, Path.GetRelativePath(from, file)), overwrite: true);
        }
    }
}

/// Converts PDB line/column positions (UTF-16 columns) to UTF-8 byte offsets of files on disk.
public sealed class SourceMaps(string root)
{
    private readonly Dictionary<string, (string Rel, int[] LineStartBytes, string[] Lines, int Bom)?> _cache = new(StringComparer.OrdinalIgnoreCase);

    public PointMap? Map(SequencePoint sp)
    {
        var info = Load(sp.Document.Url);
        if (info is not { } f) return null;
        return new PointMap(f.Rel, Offset(f, sp.StartLine, sp.StartColumn), Offset(f, sp.EndLine, sp.EndColumn));
    }

    private static int Offset((string Rel, int[] LineStartBytes, string[] Lines, int Bom) f, int line, int column)
    {
        var l = Math.Clamp(line - 1, 0, f.Lines.Length - 1);
        var text = f.Lines[l];
        var chars = Math.Clamp(column - 1, 0, text.Length);
        return f.LineStartBytes[l] + Encoding.UTF8.GetByteCount(text.AsSpan(0, chars));
    }

    private (string Rel, int[] LineStartBytes, string[] Lines, int Bom)? Load(string path)
    {
        if (_cache.TryGetValue(path, out var cached)) return cached;
        (string Rel, int[] LineStartBytes, string[] Lines, int Bom)? result = null;
        var full = Path.GetFullPath(path);
        var rel = Path.GetRelativePath(root, full);
        if (!rel.StartsWith("..", StringComparison.Ordinal) && !Path.IsPathRooted(rel) && File.Exists(full))
        {
            var bytes = File.ReadAllBytes(full);
            var bom = bytes.Length >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF ? 3 : 0;
            var text = Encoding.UTF8.GetString(bytes, bom, bytes.Length - bom);
            // Split keeping line terminators so byte counts include them.
            var lines = new List<string>();
            var start = 0;
            for (var i = 0; i < text.Length; i++)
            {
                if (text[i] == '\n')
                {
                    lines.Add(text[start..(i + 1)]);
                    start = i + 1;
                }
            }
            lines.Add(text[start..]);
            var starts = new int[lines.Count];
            var acc = bom;
            for (var i = 0; i < lines.Count; i++)
            {
                starts[i] = acc;
                acc += Encoding.UTF8.GetByteCount(lines[i]);
            }
            result = (rel.Replace('\\', '/'), starts, lines.ToArray(), bom);
        }
        _cache[path] = result;
        return result;
    }
}
