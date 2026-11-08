using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Mono.Cecil;
using Mono.Cecil.Cil;
using Mono.Cecil.Rocks;

namespace Nodex.Weaver;

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
            var weaver = new ModuleWeaver(module, TraceProbes.FromModule(module, trace.MainModule), sources, methods);
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
