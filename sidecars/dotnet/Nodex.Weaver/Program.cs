// Nodex.Weaver — injects Nodex.Trace probes into compiled .NET assemblies.
//
// Usage: Nodex.Weaver --root <project folder> --input <build output> --output <traced output>
//
// Copies the build output, rewrites every assembly whose PDB points into the project folder,
// adds Nodex.Trace.dll, and writes nodex-trace-map.json mapping (method id, point index) to
// UTF-8 byte ranges of the source files. Prints a JSON summary on stdout.

using System.Text.Json;

namespace Nodex.Weaver;

public static class Program
{
    public static int Main(string[] args)
    {
        string? Arg(string name)
        {
            var i = Array.IndexOf(args, name);
            return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
        }
        var root = Arg("--root");
        var input = Arg("--input");
        var output = Arg("--output");
        if (root is null || input is null || output is null)
        {
            Console.Error.WriteLine("usage: Nodex.Weaver --root <project> --input <build dir> --output <traced dir>");
            return 2;
        }
        try
        {
            var summary = Weaver.Run(Path.GetFullPath(root), Path.GetFullPath(input), Path.GetFullPath(output));
            Console.WriteLine(JsonSerializer.Serialize(summary, Weaver.Json));
            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"weaving failed: {ex.Message}");
            return 1;
        }
    }
}
