// Shared by Nodex.Weaver and the Unity IL post-processor (C# 9, .NET Framework / netstandard2.1).

#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using Mono.Cecil.Cil;

namespace Nodex.Weaver
{
/// One probe point: a UTF-8 byte range of a project file.
public sealed class PointMap
{
    public PointMap(string file, int start, int end)
    {
        File = file;
        Start = start;
        End = end;
    }

    public string File { get; }
    public int Start { get; }
    public int End { get; }
}

/// One instrumented method of the trace map (`nodex-trace-map.json`).
public sealed class MethodMap
{
    public MethodMap(int id, string name, string kind, List<PointMap> points, Dictionary<int, string> locals, List<string> @params)
    {
        Id = id;
        Name = name;
        Kind = kind;
        Points = points;
        Locals = locals;
        Params = @params;
    }

    public int Id { get; }
    public string Name { get; }
    public string Kind { get; }
    public List<PointMap> Points { get; }
    public Dictionary<int, string> Locals { get; }
    public List<string> Params { get; }
}

/// Converts PDB line/column positions (UTF-16 columns) to UTF-8 byte offsets of files on disk.
public sealed class SourceMaps
{
    private sealed class FileInfo
    {
        public string Rel = "";
        public int[] LineStartBytes = Array.Empty<int>();
        public string[] Lines = Array.Empty<string>();
    }

    private readonly string _root;
    private readonly Func<string, bool> _include;
    private readonly Dictionary<string, FileInfo?> _cache = new Dictionary<string, FileInfo?>(StringComparer.OrdinalIgnoreCase);

    /// <param name="include">Which project-relative files (`/` separators) get probes; all by
    /// default. Unity keeps `Assets/` only (packages live inside the project folder too).</param>
    public SourceMaps(string root, Func<string, bool>? include = null)
    {
        _root = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        _include = include ?? (_ => true);
    }

    public PointMap? Map(SequencePoint sp)
    {
        var f = Load(sp.Document.Url);
        if (f is null) return null;
        return new PointMap(f.Rel, Offset(f, sp.StartLine, sp.StartColumn), Offset(f, sp.EndLine, sp.EndColumn));
    }

    private static int Clamp(int value, int min, int max) => value < min ? min : value > max ? max : value;

    private static int Offset(FileInfo f, int line, int column)
    {
        var l = Clamp(line - 1, 0, f.Lines.Length - 1);
        var text = f.Lines[l];
        var chars = Clamp(column - 1, 0, text.Length);
        return f.LineStartBytes[l] + Encoding.UTF8.GetByteCount(text.Substring(0, chars));
    }

    /// The project-relative path (`/` separators) of a file inside the project, else null.
    private string? Relative(string full)
    {
        var prefix = _root + Path.DirectorySeparatorChar;
        var normalized = full.Replace(Path.AltDirectorySeparatorChar, Path.DirectorySeparatorChar);
        if (!normalized.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) return null;
        return normalized.Substring(prefix.Length).Replace('\\', '/');
    }

    private FileInfo? Load(string path)
    {
        if (_cache.TryGetValue(path, out var cached)) return cached;
        FileInfo? result = null;
        var full = Path.GetFullPath(path);
        var rel = Relative(full);
        if (rel != null && _include(rel) && File.Exists(full))
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
                    lines.Add(text.Substring(start, i + 1 - start));
                    start = i + 1;
                }
            }
            lines.Add(text.Substring(start));
            var starts = new int[lines.Count];
            var acc = bom;
            for (var i = 0; i < lines.Count; i++)
            {
                starts[i] = acc;
                acc += Encoding.UTF8.GetByteCount(lines[i]);
            }
            result = new FileInfo { Rel = rel, LineStartBytes = starts, Lines = lines.ToArray() };
        }
        _cache[path] = result;
        return result;
    }
}
}
