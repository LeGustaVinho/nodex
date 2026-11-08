// Nodex.Roslyn — C# semantic sidecar for Nodex.
//
// Protocol: one JSON object per line on stdin, one JSON object per line on stdout.
//   request : {"id": 1, "method": "open", "params": {"root": "D:/project"}}
//   response: {"id": 1, "result": {...}}  or  {"id": 1, "error": "message"}
// Methods: open {root} · analyze · update {files: [absolute paths]} · shutdown
//
// Facts are keyed by UTF-8 byte ranges of the files on disk (BOM included), which is how the
// Rust side joins them with its tree-sitter IR. The JSON shapes mirror
// crates/nodex-ir/src/semantic.rs.

using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace Nodex.Roslyn;

public static class Program
{
    internal static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.SnakeCaseLower) },
    };

    public static async Task<int> Main()
    {
        // Anything printed by libraries must not corrupt the protocol stream.
        var output = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false)) { AutoFlush = true, NewLine = "\n" };
        Console.SetOut(Console.Error);
        using var input = new StreamReader(Console.OpenStandardInput(), new UTF8Encoding(false));

        ProjectHost? host = null;
        string? line;
        while ((line = await input.ReadLineAsync()) is not null)
        {
            if (string.IsNullOrWhiteSpace(line)) continue;
            JsonNode? id = null;
            try
            {
                var request = JsonNode.Parse(line)!.AsObject();
                id = request["id"]?.DeepClone();
                var method = request["method"]?.GetValue<string>() ?? "";
                var parameters = request["params"] as JsonObject;
                object? result;
                switch (method)
                {
                    case "open":
                        var root = parameters?["root"]?.GetValue<string>() ?? throw new ArgumentException("root is required");
                        var opened = await ProjectHost.OpenAsync(root);
                        host?.Dispose();
                        host = opened;
                        result = new { source = host.Source, messages = host.Messages, documents = host.DocumentCount };
                        break;
                    case "analyze":
                        result = await Require(host).AnalyzeAsync();
                        break;
                    case "update":
                        var files = parameters?["files"]?.AsArray().Select(f => f!.GetValue<string>()).ToList() ?? [];
                        var current = Require(host);
                        if (files.Any(ProjectHost.IsBuildConfiguration))
                        {
                            host = await ProjectHost.OpenAsync(current.Root);
                            current.Dispose();
                        }
                        else
                        {
                            current.Update(files);
                        }
                        result = await Require(host).AnalyzeAsync();
                        break;
                    case "shutdown":
                        await Write(output, new JsonObject { ["id"] = id, ["result"] = true });
                        return 0;
                    default:
                        throw new ArgumentException($"unknown method '{method}'");
                }
                // Serialized straight to text: facts of a large project are tens of megabytes, too
                // many to build as a JSON tree first.
                await output.WriteLineAsync($"{{\"id\":{id?.ToJsonString() ?? "null"},\"result\":{JsonSerializer.Serialize(result, Json)}}}");
            }
            catch (Exception ex)
            {
                await Write(output, new JsonObject { ["id"] = id, ["error"] = ex.Message });
            }
        }
        return 0;
    }

    private static ProjectHost Require(ProjectHost? host) =>
        host ?? throw new InvalidOperationException("call 'open' first");

    private static Task Write(StreamWriter output, JsonObject message) =>
        output.WriteLineAsync(message.ToJsonString(Json));
}
