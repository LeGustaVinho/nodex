using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Text;

namespace Nodex.Roslyn;

/// Walks documents and records calls, creations, bases, event usage and local types.
public sealed class FactCollector(ProjectHost host)
{
    private static readonly SymbolDisplayFormat Display = new(
        typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypes,
        genericsOptions: SymbolDisplayGenericsOptions.IncludeTypeParameters,
        memberOptions: SymbolDisplayMemberOptions.IncludeContainingType | SymbolDisplayMemberOptions.IncludeParameters,
        parameterOptions: SymbolDisplayParameterOptions.IncludeType,
        miscellaneousOptions: SymbolDisplayMiscellaneousOptions.UseSpecialTypes);

    // `var` locals are always nullable-annotated in Roslyn; show the type as the reader thinks of it.
    private static readonly SymbolDisplayFormat TypeDisplay = SymbolDisplayFormat.MinimallyQualifiedFormat
        .RemoveMiscellaneousOptions(SymbolDisplayMiscellaneousOptions.IncludeNullableReferenceTypeModifier);

    private readonly List<SemanticRef> _refs = [];
    private readonly List<LocalTypeFact> _locals = [];
    private readonly Dictionary<string, ByteMap> _maps = new(StringComparer.OrdinalIgnoreCase);
    private int _unresolvedCalls;

    public async Task CollectAsync(Document doc)
    {
        var model = await doc.GetSemanticModelAsync();
        var root = await doc.GetSyntaxRootAsync();
        if (model is null || root is null || doc.FilePath is null) return;
        var map = MapFor(root.SyntaxTree);

        foreach (var node in root.DescendantNodes())
        {
            switch (node)
            {
                case InvocationExpressionSyntax inv:
                    Invocation(model, map, inv);
                    break;
                case BaseObjectCreationExpressionSyntax creation:
                    if (model.GetSymbolInfo(creation).Symbol is IMethodSymbol ctor)
                    {
                        var target = ctor.DeclaringSyntaxReferences.Length > 0 ? Target(ctor) : Target(ctor.ContainingType) with { Kind = "constructor" };
                        Add(map, creation.Span, RefKind.Construct, target);
                    }
                    break;
                case AssignmentExpressionSyntax assign when assign.IsKind(SyntaxKind.AddAssignmentExpression) || assign.IsKind(SyntaxKind.SubtractAssignmentExpression):
                    if (model.GetSymbolInfo(assign.Left).Symbol is IEventSymbol ev)
                    {
                        var kind = assign.IsKind(SyntaxKind.AddAssignmentExpression) ? RefKind.Subscribe : RefKind.Unsubscribe;
                        Add(map, assign.Span, kind, Target(ev));
                        var handlerInfo = model.GetSymbolInfo(assign.Right);
                        if ((handlerInfo.Symbol ?? handlerInfo.CandidateSymbols.FirstOrDefault()) is IMethodSymbol handler)
                        {
                            Add(map, assign.Span, RefKind.Handler, Target(handler));
                        }
                    }
                    break;
                case BaseTypeSyntax baseType:
                    if (model.GetTypeInfo(baseType.Type).Type is { } type)
                    {
                        Add(map, baseType.Span, RefKind.Base, type.TypeKind == TypeKind.Error
                            ? new SemanticTarget(null, type.Name, null, "unresolved", false, true)
                            : Target(type));
                    }
                    break;
                case VariableDeclaratorSyntax declarator:
                    Local(map, model.GetDeclaredSymbol(declarator), declarator.Identifier.Span);
                    break;
                case ForEachStatementSyntax forEach:
                    Local(map, model.GetDeclaredSymbol(forEach), forEach.Identifier.Span);
                    break;
                case SingleVariableDesignationSyntax designation:
                    Local(map, model.GetDeclaredSymbol(designation), designation.Identifier.Span);
                    break;
            }
        }
    }

    private void Invocation(SemanticModel model, ByteMap map, InvocationExpressionSyntax inv)
    {
        // Event raise: `Handler?.Invoke(x)`, `Handler.Invoke(x)` or `Handler(x)`.
        ExpressionSyntax? receiver = inv.Expression switch
        {
            MemberAccessExpressionSyntax { Name.Identifier.Text: "Invoke" } ma => ma.Expression,
            MemberBindingExpressionSyntax { Name.Identifier.Text: "Invoke" } => (inv.Parent as ConditionalAccessExpressionSyntax)?.Expression,
            IdentifierNameSyntax or MemberAccessExpressionSyntax => inv.Expression,
            _ => null,
        };
        if (receiver is not null && model.GetSymbolInfo(receiver).Symbol is IEventSymbol raised)
        {
            Add(map, inv.Span, RefKind.Raise, Target(raised));
            return;
        }
        var info = model.GetSymbolInfo(inv);
        if ((info.Symbol ?? info.CandidateSymbols.FirstOrDefault()) is IMethodSymbol method)
        {
            Add(map, inv.Span, RefKind.Call, Target(method));
        }
        else if (inv.Expression is not IdentifierNameSyntax { Identifier.Text: "nameof" })
        {
            _unresolvedCalls++;
        }
    }

    private void Local(ByteMap map, ISymbol? symbol, TextSpan identifier)
    {
        if (symbol is ILocalSymbol local && local.Type.TypeKind != TypeKind.Error)
        {
            _locals.Add(new LocalTypeFact(map.Range(identifier), local.Name, local.Type.ToDisplayString(TypeDisplay)));
        }
    }

    private void Add(ByteMap map, TextSpan site, RefKind kind, SemanticTarget target) =>
        _refs.Add(new SemanticRef(map.Range(site), kind, target));

    private SemanticTarget Target(ISymbol symbol)
    {
        symbol = symbol switch
        {
            IMethodSymbol m => ((m.ReducedFrom ?? m).PartialImplementationPart ?? (m.ReducedFrom ?? m)).OriginalDefinition,
            _ => symbol.OriginalDefinition,
        };
        ByteRange? decl = null;
        foreach (var reference in symbol.DeclaringSyntaxReferences)
        {
            if (!host.IsSourceFile(reference.SyntaxTree.FilePath)) continue;
            decl = MapFor(reference.SyntaxTree).Range(reference.Span);
            break;
        }
        var kind = symbol switch
        {
            IMethodSymbol { MethodKind: MethodKind.Constructor or MethodKind.StaticConstructor } => "constructor",
            IMethodSymbol => "method",
            INamedTypeSymbol => "type",
            IEventSymbol => "event",
            IPropertySymbol => "property",
            IFieldSymbol => "field",
            _ => symbol.Kind.ToString().ToLowerInvariant(),
        };
        var container = symbol.ContainingType?.ToDisplayString() ?? symbol.ContainingNamespace?.ToDisplayString();
        var linq = symbol is IMethodSymbol && container is "System.Linq.Enumerable" or "System.Linq.Queryable";
        var display = symbol is INamedTypeSymbol type && type.ContainingNamespace is { IsGlobalNamespace: false } ns
            ? $"{ns.ToDisplayString()}.{symbol.ToDisplayString(Display)}"
            : symbol.ToDisplayString(Display);
        return new SemanticTarget(decl, display, container, kind, linq, decl is null);
    }

    private ByteMap MapFor(SyntaxTree tree)
    {
        if (_maps.TryGetValue(tree.FilePath, out var map)) return map;
        var bom = 0;
        try
        {
            using var stream = File.OpenRead(tree.FilePath);
            Span<byte> head = stackalloc byte[3];
            if (stream.Read(head) == 3 && head[0] == 0xEF && head[1] == 0xBB && head[2] == 0xBF) bom = 3;
        }
        catch (IOException) { }
        map = new ByteMap(host.Relative(tree.FilePath), tree.GetText().ToString(), bom);
        _maps[tree.FilePath] = map;
        return map;
    }

    public SemanticFacts Finish(string source, List<string> hostMessages)
    {
        var messages = new List<string>(hostMessages);
        if (_unresolvedCalls > 0)
        {
            messages.Add($"{_unresolvedCalls} calls could not be resolved (missing references or syntax errors).");
        }
        return new SemanticFacts(source, _refs, _locals, messages);
    }

    /// Converts UTF-16 positions (Roslyn) to UTF-8 byte offsets (tree-sitter).
    private sealed class ByteMap
    {
        private readonly string _file;
        private readonly int[] _bytes;

        public ByteMap(string file, string text, int bom)
        {
            _file = file;
            _bytes = new int[text.Length + 1];
            _bytes[0] = bom;
            for (var i = 0; i < text.Length;)
            {
                var c = text[i];
                if (char.IsHighSurrogate(c) && i + 1 < text.Length && char.IsLowSurrogate(text[i + 1]))
                {
                    _bytes[i + 1] = _bytes[i] + 2;
                    _bytes[i + 2] = _bytes[i] + 4;
                    i += 2;
                }
                else
                {
                    _bytes[i + 1] = _bytes[i] + (c < 0x80 ? 1 : c < 0x800 ? 2 : 3);
                    i++;
                }
            }
        }

        public ByteRange Range(TextSpan span) =>
            new(_file, _bytes[Math.Min(span.Start, _bytes.Length - 1)], _bytes[Math.Min(span.End, _bytes.Length - 1)]);
    }
}
