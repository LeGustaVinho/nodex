using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.MSBuild;
using Microsoft.CodeAnalysis.Text;

namespace Nodex.Roslyn;

/// Loads a project folder into a Roslyn <see cref="Solution"/> and keeps it current.
/// Prefers MSBuild (.sln/.slnx/.csproj); falls back to a loose-files compilation against the
/// running .NET's reference assemblies when there is no project file or loading fails
/// (for example a Unity project opened without the editor's generated projects).
public sealed class ProjectHost
{
    // Must match nodex-core's IGNORED_DIRS.
    private static readonly HashSet<string> IgnoredDirs = new(StringComparer.OrdinalIgnoreCase)
    {
        ".git", ".hg", ".svn", ".vs", ".idea", ".vscode", ".nodex", "bin", "obj", "node_modules", "target",
        "dist", "build", "Library", "Temp", "Logs", "UserSettings", "MemoryCaptures",
    };

    private Solution _solution;

    public string Root { get; }
    public string Source { get; }
    public List<string> Messages { get; } = [];
    public int DocumentCount => SourceDocuments().Count();

    private ProjectHost(string root, Solution solution, string source, List<string> messages)
    {
        Root = root;
        _solution = solution;
        Source = source;
        Messages.AddRange(messages);
    }

    public static async Task<ProjectHost> OpenAsync(string root)
    {
        root = Path.GetFullPath(root);
        if (!Directory.Exists(root)) throw new DirectoryNotFoundException(root);
        var messages = new List<string>();

        var entries = FindProjectEntries(root);
        if (entries.Count > 0)
        {
            try
            {
                var workspace = MSBuildWorkspace.Create();
#pragma warning disable CS0618 // WorkspaceFailed is the portable way to observe load problems.
                workspace.WorkspaceFailed += (_, e) =>
                {
                    if (messages.Count < 20) messages.Add($"{e.Diagnostic.Kind}: {e.Diagnostic.Message}");
                };
#pragma warning restore CS0618
                foreach (var entry in entries)
                {
                    if (entry.EndsWith(".sln", StringComparison.OrdinalIgnoreCase) || entry.EndsWith(".slnx", StringComparison.OrdinalIgnoreCase))
                    {
                        await workspace.OpenSolutionAsync(entry);
                        break; // a solution covers its projects
                    }
                    if (workspace.CurrentSolution.Projects.Any(p => string.Equals(p.FilePath, entry, StringComparison.OrdinalIgnoreCase))) continue;
                    await workspace.OpenProjectAsync(entry);
                }
                var host = new ProjectHost(root, workspace.CurrentSolution,
                    $"Roslyn · MSBuild · {string.Join(", ", entries.Select(Path.GetFileName).Take(3))}", messages);
                if (host.DocumentCount > 0) return host;
                messages.Add("MSBuild loaded no C# documents; using loose files instead.");
            }
            catch (Exception ex)
            {
                messages.Add($"MSBuild load failed ({ex.Message}); using loose files instead.");
            }
        }
        return new ProjectHost(root, LooseFiles(root), "Roslyn · loose files (no build)", messages);
    }

    /// Solutions first (at most two folders deep), otherwise every .csproj (three deep).
    private static List<string> FindProjectEntries(string root)
    {
        var solutions = Find(root, 2, f => f.EndsWith(".sln", StringComparison.OrdinalIgnoreCase) || f.EndsWith(".slnx", StringComparison.OrdinalIgnoreCase));
        if (solutions.Count > 0) return [solutions.OrderBy(s => s.Count(c => c == Path.DirectorySeparatorChar)).First()];
        return Find(root, 3, f => f.EndsWith(".csproj", StringComparison.OrdinalIgnoreCase));
    }

    private static List<string> Find(string dir, int depth, Func<string, bool> predicate)
    {
        var found = new List<string>();
        try
        {
            found.AddRange(Directory.EnumerateFiles(dir).Where(predicate));
            if (depth > 0)
            {
                foreach (var sub in Directory.EnumerateDirectories(dir))
                {
                    if (!IgnoredDirs.Contains(Path.GetFileName(sub))) found.AddRange(Find(sub, depth - 1, predicate));
                }
            }
        }
        catch (UnauthorizedAccessException) { }
        return found;
    }

    private static Solution LooseFiles(string root)
    {
        var workspace = new AdhocWorkspace();
        var references = ((string?)AppContext.GetData("TRUSTED_PLATFORM_ASSEMBLIES") ?? "")
            .Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries)
            .Where(p => Path.GetFileName(p).StartsWith("System", StringComparison.OrdinalIgnoreCase)
                        || Path.GetFileName(p).Equals("mscorlib.dll", StringComparison.OrdinalIgnoreCase)
                        || Path.GetFileName(p).Equals("netstandard.dll", StringComparison.OrdinalIgnoreCase))
            .Select(p => MetadataReference.CreateFromFile(p));
        var projectId = ProjectId.CreateNewId();
        var info = ProjectInfo.Create(projectId, VersionStamp.Default, "Nodex", "Nodex", LanguageNames.CSharp,
            compilationOptions: new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, nullableContextOptions: NullableContextOptions.Enable),
            parseOptions: new CSharpParseOptions(LanguageVersion.Preview),
            metadataReferences: references);
        var solution = workspace.CurrentSolution.AddProject(info);
        foreach (var file in Find(root, 32, f => f.EndsWith(".cs", StringComparison.OrdinalIgnoreCase)))
        {
            solution = solution.AddDocument(DocumentId.CreateNewId(projectId), Path.GetFileName(file), Load(file), filePath: file);
        }
        return solution;
    }

    private static SourceText Load(string file)
    {
        using var stream = File.OpenRead(file);
        return SourceText.From(stream);
    }

    /// True for C# files inside the root that are not in an ignored folder.
    public bool IsSourceFile(string? path)
    {
        if (path is null || !path.EndsWith(".cs", StringComparison.OrdinalIgnoreCase)) return false;
        var rel = Path.GetRelativePath(Root, path);
        if (rel.StartsWith("..", StringComparison.Ordinal) || Path.IsPathRooted(rel)) return false;
        var parts = rel.Split(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        return !parts.Take(parts.Length - 1).Any(IgnoredDirs.Contains);
    }

    public string Relative(string path) => Path.GetRelativePath(Root, path).Replace('\\', '/');

    /// One document per file path (multi-targeted projects list a file several times).
    public IEnumerable<Document> SourceDocuments()
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var project in _solution.Projects.Where(p => p.Language == LanguageNames.CSharp))
        {
            foreach (var doc in project.Documents)
            {
                if (IsSourceFile(doc.FilePath) && seen.Add(doc.FilePath!)) yield return doc;
            }
        }
    }

    public void Update(IEnumerable<string> files)
    {
        foreach (var raw in files)
        {
            var path = Path.GetFullPath(raw);
            var ids = _solution.GetDocumentIdsWithFilePath(path);
            if (File.Exists(path))
            {
                var text = Load(path);
                if (ids.Length > 0)
                {
                    foreach (var id in ids) _solution = _solution.WithDocumentText(id, text);
                }
                else if (IsSourceFile(path))
                {
                    // A new file: add it to the project whose folder contains it (deepest wins).
                    var project = _solution.Projects
                        .Where(p => p.Language == LanguageNames.CSharp)
                        .OrderByDescending(p => p.FilePath is { } f && path.StartsWith(Path.GetDirectoryName(f)!, StringComparison.OrdinalIgnoreCase) ? Path.GetDirectoryName(f)!.Length : -1)
                        .FirstOrDefault();
                    if (project is not null)
                    {
                        _solution = _solution.AddDocument(DocumentId.CreateNewId(project.Id), Path.GetFileName(path), text, filePath: path);
                    }
                }
            }
            else
            {
                foreach (var id in ids) _solution = _solution.RemoveDocument(id);
            }
        }
    }

    public async Task<SemanticFacts> AnalyzeAsync()
    {
        var collector = new FactCollector(this);
        foreach (var doc in SourceDocuments())
        {
            await collector.CollectAsync(doc);
        }
        return collector.Finish(Source, Messages);
    }
}
