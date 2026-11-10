using System.Collections.Concurrent;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Operations;
using Microsoft.CodeAnalysis.Text;

namespace Nodex.Roslyn;

/// Walks documents and records calls, creations, bases, event usage and local types.
/// Collectors of several documents can run at once when they share <paramref name="maps"/>;
/// <see cref="Merge"/> then joins them in document order.
public sealed class FactCollector(
    ProjectHost host,
    ConcurrentDictionary<string, FactCollector.ByteMap> maps,
    ConcurrentDictionary<Compilation, Lazy<HashSet<INamedTypeSymbol>>> interfaceConflicts,
    bool sharedDocument = false)
{
    public FactCollector(ProjectHost host) : this(host, NewMaps(), new()) { }

    /// A cache of byte maps by file path, shared by collectors that run in parallel.
    public static ConcurrentDictionary<string, ByteMap> NewMaps() => new(StringComparer.OrdinalIgnoreCase);

    private static readonly SymbolDisplayFormat Display = new(
        typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypes,
        genericsOptions: SymbolDisplayGenericsOptions.IncludeTypeParameters,
        memberOptions: SymbolDisplayMemberOptions.IncludeContainingType | SymbolDisplayMemberOptions.IncludeParameters,
        parameterOptions: SymbolDisplayParameterOptions.IncludeType,
        miscellaneousOptions: SymbolDisplayMiscellaneousOptions.UseSpecialTypes);

    // `var` locals are always nullable-annotated in Roslyn; show the type as the reader thinks of it.
    private static readonly SymbolDisplayFormat TypeDisplay = SymbolDisplayFormat.MinimallyQualifiedFormat
        .RemoveMiscellaneousOptions(SymbolDisplayMiscellaneousOptions.IncludeNullableReferenceTypeModifier);

    private static readonly SymbolDisplayFormat BlueprintTypeDisplay = new(
        typeQualificationStyle: SymbolDisplayTypeQualificationStyle.NameAndContainingTypesAndNamespaces,
        genericsOptions: SymbolDisplayGenericsOptions.IncludeTypeParameters,
        miscellaneousOptions: SymbolDisplayMiscellaneousOptions.UseSpecialTypes
            | SymbolDisplayMiscellaneousOptions.IncludeNullableReferenceTypeModifier
            | SymbolDisplayMiscellaneousOptions.EscapeKeywordIdentifiers);

    private readonly List<SemanticRef> _refs = [];
    private readonly List<LocalTypeFact> _locals = [];
    private readonly List<ExpressionFact> _expressions = [];
    private readonly List<ValueFact> _values = [];
    private readonly List<TypeFact> _types = [];
    private readonly List<BindingUseFact> _bindingUses = [];
    private readonly List<NavigationFact> _navigation = [];
    private readonly Dictionary<ISymbol, SemanticTarget> _navigationTargets = new(SymbolEqualityComparer.Default);
    private int _unresolvedCalls;

    /// Appends what <paramref name="other"/> collected (a later document).
    public void Merge(FactCollector other)
    {
        _refs.AddRange(other._refs);
        _locals.AddRange(other._locals);
        _expressions.AddRange(other._expressions);
        _values.AddRange(other._values);
        _types.AddRange(other._types);
        _bindingUses.AddRange(other._bindingUses);
        _navigation.AddRange(other._navigation);
        _unresolvedCalls += other._unresolvedCalls;
    }

    public async Task CollectAsync(Document doc)
    {
        var model = await doc.GetSemanticModelAsync();
        var root = await doc.GetSyntaxRootAsync();
        if (model is null || root is null || doc.FilePath is null) return;
        var map = MapFor(root.SyntaxTree);

        foreach (var node in root.DescendantNodes())
        {
            if (node is ExpressionSyntax expression)
            {
                var conversion = model.GetConversion(expression);
                if (node.Parent is not CastExpressionSyntax && conversion.MethodSymbol is { } implicitConversion)
                    AddUsage(map, expression.Span, implicitConversion);
                if (!sharedDocument)
                    CollectExpressionFact(model, map, expression, conversion);
            }
            CollectValue(model, map, node);
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
                        var site = creation is ObjectCreationExpressionSyntax explicitCreation
                            ? explicitCreation.Type.Span : creation.NewKeyword.Span;
                        AddUsage(map, site, ctor);
                        if (creation is ImplicitObjectCreationExpressionSyntax)
                            AddUsage(map, site, ctor.ContainingType);
                    }
                    break;
                case ConstructorInitializerSyntax initializer:
                    AddUsage(map, initializer.ThisOrBaseKeyword.Span, model.GetSymbolInfo(initializer).Symbol);
                    break;
                case AttributeSyntax attribute:
                    if (model.GetSymbolInfo(attribute).Symbol is IMethodSymbol attributeConstructor)
                    {
                        AddUsage(map, attribute.Name.Span, attributeConstructor);
                        AddUsage(map, attribute.Name.Span, attributeConstructor.ContainingType);
                    }
                    break;
                case CastExpressionSyntax cast when model.GetOperation(cast) is IConversionOperation { OperatorMethod: { } conversion }:
                    AddUsage(map, cast.Type.Span, conversion);
                    break;
                case BinaryExpressionSyntax binary:
                    AddUsage(map, binary.OperatorToken.Span, model.GetSymbolInfo(binary).Symbol);
                    break;
                case PrefixUnaryExpressionSyntax prefix:
                    AddUsage(map, prefix.OperatorToken.Span, model.GetSymbolInfo(prefix).Symbol);
                    break;
                case PostfixUnaryExpressionSyntax postfix:
                    AddUsage(map, postfix.OperatorToken.Span, model.GetSymbolInfo(postfix).Symbol);
                    break;
                case AssignmentExpressionSyntax assign when !assign.IsKind(SyntaxKind.SimpleAssignmentExpression):
                    if (model.GetOperation(assign) is ICompoundAssignmentOperation { OperatorMethod: { } compoundOperator })
                        AddUsage(map, assign.OperatorToken.Span, compoundOperator);
                    if ((assign.IsKind(SyntaxKind.AddAssignmentExpression) || assign.IsKind(SyntaxKind.SubtractAssignmentExpression))
                        && model.GetSymbolInfo(assign.Left).Symbol is IEventSymbol ev)
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
                case ElementAccessExpressionSyntax access:
                    AddUsage(map, access.ArgumentList.OpenBracketToken.Span, model.GetSymbolInfo(access).Symbol);
                    break;
                case ElementBindingExpressionSyntax binding:
                    AddUsage(map, binding.ArgumentList.OpenBracketToken.Span, model.GetSymbolInfo(binding).Symbol);
                    break;
                case ImplicitElementAccessSyntax implicitAccess:
                    AddUsage(map, implicitAccess.ArgumentList.OpenBracketToken.Span, model.GetSymbolInfo(implicitAccess).Symbol);
                    break;
                case SimpleNameSyntax name:
                    AddUsage(map, name.Identifier.Span, model.GetSymbolInfo(name).Symbol);
                    break;
                case BaseTypeDeclarationSyntax declaration when model.GetDeclaredSymbol(declaration) is INamedTypeSymbol declaredType:
                    CollectImplementations(declaredType, model.Compilation);
                    break;
            }
        }
    }

    private SemanticTarget NavigationTarget(ISymbol symbol)
    {
        if (!_navigationTargets.TryGetValue(symbol, out var target))
            _navigationTargets[symbol] = target = Target(symbol);
        return target;
    }

    private void AddUsage(ByteMap map, TextSpan site, ISymbol? symbol)
    {
        if (symbol is IAliasSymbol alias) symbol = alias.Target;
        if (symbol is ILocalSymbol or IParameterSymbol)
        {
            var declaration = symbol.Locations.FirstOrDefault(location => location.IsInSource);
            if (declaration?.SourceTree is { } tree && host.IsSourceFile(tree.FilePath))
                _bindingUses.Add(new BindingUseFact(map.Range(site), MapFor(tree).Range(declaration.SourceSpan)));
            return;
        }
        if (symbol is not (INamedTypeSymbol or IMethodSymbol or IFieldSymbol or IPropertySymbol or IEventSymbol)) return;
        // The same token in a linked document can bind to a local member in one
        // compilation and a metadata member in another. Keep both for the Blueprint.
        var trackExternalMember = sharedDocument && symbol is (IFieldSymbol or IPropertySymbol or IEventSymbol);
        if (!trackExternalMember && !symbol.Locations.Any(location => location.IsInSource)) return;
        var target = NavigationTarget(symbol);
        if (target.Decl is not null || trackExternalMember) _navigation.Add(new NavigationFact(map.Range(site), target));
    }

    private void AddImplementation(ISymbol? contract, ISymbol implementation)
    {
        if (contract is null || SymbolEqualityComparer.Default.Equals(contract, implementation)) return;
        var target = NavigationTarget(contract);
        if (target.Decl is not null && NavigationTarget(implementation).Decl is { } site)
            _navigation.Add(new NavigationFact(site, target, Implementation: true));
    }

    private void CollectImplementations(INamedTypeSymbol type, Compilation compilation)
    {
        foreach (var member in type.GetMembers())
        {
            var overridden = member switch
            {
                IMethodSymbol { AssociatedSymbol: null } method => (ISymbol?)method.OverriddenMethod,
                IPropertySymbol property => property.OverriddenProperty,
                IEventSymbol ev => ev.OverriddenEvent,
                _ => null,
            };
            AddImplementation(overridden, member);
            if (type.TypeKind == TypeKind.Interface && !member.IsAbstract)
            {
                IEnumerable<ISymbol> explicitContracts = member switch
                {
                    IMethodSymbol { AssociatedSymbol: null } method => method.ExplicitInterfaceImplementations,
                    IPropertySymbol property => property.ExplicitInterfaceImplementations,
                    IEventSymbol ev => ev.ExplicitInterfaceImplementations,
                    _ => [],
                };
                foreach (var contract in explicitContracts) AddImplementation(contract, member);
            }
        }
        AddImplementation(type.BaseType, type);
        if (type.TypeKind == TypeKind.Interface)
        {
            foreach (var contract in type.Interfaces) AddImplementation(contract, type);
            return;
        }
        // A default method can have a body and still lack a unique most specific implementation.
        if (type.AllInterfaces.Length > 1 && interfaceConflicts.GetOrAdd(compilation, static c =>
                new Lazy<HashSet<INamedTypeSymbol>>(() => FindInterfaceConflicts(c))).Value.Contains(type))
            return;
        foreach (var contract in type.AllInterfaces)
        {
            var complete = true;
            foreach (var member in contract.GetMembers())
            {
                if (member is not (IMethodSymbol or IPropertySymbol or IEventSymbol)) continue;
                if (type.FindImplementationForInterfaceMember(member) is { } implementation)
                {
                    if (member is not IMethodSymbol { AssociatedSymbol: not null })
                        AddImplementation(member, implementation);
                }
                else if (member.IsAbstract)
                    complete = false;
            }
            if (complete) AddImplementation(contract, type);
        }
    }

    private static HashSet<INamedTypeSymbol> FindInterfaceConflicts(Compilation compilation)
    {
        var conflicts = new HashSet<INamedTypeSymbol>(SymbolEqualityComparer.Default);
        foreach (var diagnostic in compilation.GetDeclarationDiagnostics())
        {
            if (diagnostic.Id != "CS8705" || diagnostic.Location.SourceTree is not { } tree) continue;
            var token = tree.GetRoot().FindToken(diagnostic.Location.SourceSpan.Start);
            var declaration = token.Parent?.AncestorsAndSelf().OfType<BaseTypeDeclarationSyntax>().FirstOrDefault();
            if (declaration is not null && compilation.GetSemanticModel(tree).GetDeclaredSymbol(declaration) is INamedTypeSymbol type)
                conflicts.Add(type);
        }
        return conflicts;
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
        if (info.Symbol is IMethodSymbol method)
        {
            Add(map, inv.Span, RefKind.Call, Target(method));
        }
        else if (inv.Expression is not IdentifierNameSyntax { Identifier.Text: "nameof" })
        {
            _unresolvedCalls++;
            // The unresolved call still counts when deciding whether a statement has one target.
            Add(map, inv.Span, RefKind.Call,
                new SemanticTarget(null, inv.Expression.ToString(), null, "unresolved", false, false));
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

    private ByteMap MapFor(SyntaxTree tree) => maps.GetOrAdd(tree.FilePath, _ => NewMap(tree));

    private ByteMap NewMap(SyntaxTree tree)
    {
        var bom = 0;
        try
        {
            using var stream = File.OpenRead(tree.FilePath);
            Span<byte> head = stackalloc byte[3];
            if (stream.Read(head) == 3 && head[0] == 0xEF && head[1] == 0xBB && head[2] == 0xBF) bom = 3;
        }
        catch (IOException) { }
        return new ByteMap(host.Relative(tree.FilePath), tree.GetText().ToString(), bom);
    }

    public SemanticFacts Finish(string source, List<string> hostMessages, bool shared = false)
    {
        var messages = new List<string>(hostMessages);
        if (_unresolvedCalls > 0)
        {
            messages.Add($"{_unresolvedCalls} calls could not be resolved (missing references or syntax errors).");
        }
        return shared
            ? new SemanticFacts(source, MergeReferences(), _locals.DistinctBy(local => (local.Site, local.Name)).ToList(),
                _expressions.Distinct().ToList(), _values.Distinct().ToList(), _types, _bindingUses.Distinct().ToList(),
                _navigation.Distinct().ToList(), messages)
            : new SemanticFacts(source, _refs, _locals, _expressions, _values, _types, _bindingUses, _navigation, messages);
    }

    private void CollectValue(SemanticModel model, ByteMap map, SyntaxNode node)
    {
        switch (node)
        {
            case ParameterSyntax p when model.GetDeclaredSymbol(p) is IParameterSymbol parameter:
                AddValue(p.Identifier.Span, "binding", parameter.Type, Mode(parameter.RefKind), parameter.RefKind != Microsoft.CodeAnalysis.RefKind.None);
                break;
            case VariableDeclaratorSyntax d:
                switch (model.GetDeclaredSymbol(d))
                {
                    case ILocalSymbol local:
                        AddValue(d.Identifier.Span, "binding", local.Type, AliasMode(local.RefKind), local.RefKind != Microsoft.CodeAnalysis.RefKind.None);
                        break;
                    case IFieldSymbol field:
                        AddValue(d.Identifier.Span, "member", field.Type, AliasMode(field.RefKind), field.RefKind != Microsoft.CodeAnalysis.RefKind.None);
                        break;
                }
                break;
            case SingleVariableDesignationSyntax d when model.GetDeclaredSymbol(d) is ILocalSymbol local:
                AddValue(d.Identifier.Span, "binding", local.Type);
                break;
            case ForEachStatementSyntax f when model.GetDeclaredSymbol(f) is ILocalSymbol local:
                AddValue(f.Identifier.Span, "binding", local.Type, AliasMode(local.RefKind), local.RefKind != Microsoft.CodeAnalysis.RefKind.None);
                break;
            case PropertyDeclarationSyntax p when model.GetDeclaredSymbol(p) is IPropertySymbol property:
                AddValue(p.Identifier.Span, "member", property.Type, AliasMode(property.RefKind), property.RefKind != Microsoft.CodeAnalysis.RefKind.None);
                break;
            case MethodDeclarationSyntax m when model.GetDeclaredSymbol(m) is IMethodSymbol method && !method.ReturnsVoid:
                AddValue(m.Identifier.Span, "member", method.ReturnType, AliasMode(method.RefKind), method.RefKind != Microsoft.CodeAnalysis.RefKind.None);
                break;
            case ExpressionSyntax expression when node is not TypeSyntax || node is IdentifierNameSyntax:
                var expressionType = model.GetTypeInfo(expression);
                var type = expressionType.Type;
                if (expression.Parent is ArgumentSyntax argument &&
                    model.GetOperation(argument) is IArgumentOperation { Parameter: { } formal })
                {
                    AddValue(expression.Span, "expression", type, Mode(formal.RefKind), flowState: expressionType.Nullability.FlowState);
                }
                else if (expression is InvocationExpressionSyntax &&
                    model.GetSymbolInfo(expression).Symbol is IMethodSymbol called && !called.ReturnsVoid)
                {
                    AddValue(expression.Span, "expression", type, AliasMode(called.RefKind), called.RefKind != Microsoft.CodeAnalysis.RefKind.None,
                        expressionType.Nullability.FlowState);
                }
                else if (type is not null && type.SpecialType != SpecialType.System_Void)
                {
                    AddValue(expression.Span, "expression", type, flowState: expressionType.Nullability.FlowState);
                }
                break;
        }

        void AddValue(TextSpan span, string target, ITypeSymbol? type, string? passing = null, bool alias = false,
            NullableFlowState flowState = NullableFlowState.None)
        {
            var (kind, qualifier) = alias ? ("Reference", "Alias") : type switch
            {
                { TypeKind: TypeKind.Pointer or TypeKind.FunctionPointer } => ("Pointer", "Raw"),
                { TypeKind: TypeKind.Error } or null => ("Unknown", null),
                { IsValueType: true } => ("Value", null),
                { IsReferenceType: true } => ("Reference", "Object"),
                _ => ("Unknown", null),
            };
            _values.Add(new ValueFact(map.Range(span), target, new ValueInfo(kind, "TypeAnalysis", qualifier, passing)));
            if (type is null && node is not LiteralExpressionSyntax { RawKind: (int)SyntaxKind.NullLiteralExpression })
                return;
            var written = node switch
            {
                ParameterSyntax { Type: not null } => true,
                VariableDeclaratorSyntax { Parent: VariableDeclarationSyntax declaration } => !declaration.Type.IsVar,
                ForEachStatementSyntax loop => !loop.Type.IsVar,
                PropertyDeclarationSyntax or MethodDeclarationSyntax => true,
                _ => false,
            };
            if (type is { IsReferenceType: true } && !written)
            {
                // An implicitly typed local can be nullable even when its initializer is known
                // to be non-null. The type at a read also follows Roslyn's flow state.
                if (flowState == NullableFlowState.None && ImplicitInitializer(node) is { } initializer)
                    flowState = initializer is BaseObjectCreationExpressionSyntax or ArrayCreationExpressionSyntax
                        or ImplicitArrayCreationExpressionSyntax or LiteralExpressionSyntax { RawKind: (int)SyntaxKind.StringLiteralExpression }
                        ? NullableFlowState.NotNull : model.GetTypeInfo(initializer).Nullability.FlowState;
                if (flowState == NullableFlowState.NotNull)
                    type = type.WithNullableAnnotation(NullableAnnotation.NotAnnotated);
            }
            var shape = type is null ? new { kind = "Scalar", name = "Null" } : BlueprintTypes.Shape(type);
            if (alias) shape = new { kind = "Indirect", pointer = false, target = shape };
            _types.Add(new TypeFact(map.Range(span), target, new TypeInfo(shape, written ? "Declared" : "Compiler",
                type?.ToDisplayString(BlueprintTypeDisplay))));
        }
    }

    private static ExpressionSyntax? ImplicitInitializer(SyntaxNode node)
    {
        if (node is VariableDeclaratorSyntax variable) return variable.Initializer?.Value;
        if (node is not SingleVariableDesignationSyntax designation) return null;
        VariableDesignationSyntax pattern = designation;
        var elements = new Stack<int>();
        while (pattern.Parent is ParenthesizedVariableDesignationSyntax group)
        {
            elements.Push(group.Variables.IndexOf(pattern));
            pattern = group;
        }
        if (pattern.Parent is not DeclarationExpressionSyntax declaration || !declaration.Type.IsVar ||
            declaration.Parent is not AssignmentExpressionSyntax assignment || assignment.Left != declaration)
            return null;
        var value = assignment.Right;
        while (elements.TryPop(out var index))
        {
            while (value is ParenthesizedExpressionSyntax parens) value = parens.Expression;
            if (value is not TupleExpressionSyntax tuple || index < 0 || index >= tuple.Arguments.Count) return null;
            value = tuple.Arguments[index].Expression;
        }
        return value;
    }

    private static string? AliasMode(Microsoft.CodeAnalysis.RefKind kind) =>
        kind == Microsoft.CodeAnalysis.RefKind.None ? null :
        kind == Microsoft.CodeAnalysis.RefKind.In ? "RefReadonly" : Mode(kind);

    private static string Mode(Microsoft.CodeAnalysis.RefKind kind) => kind switch
    {
        Microsoft.CodeAnalysis.RefKind.Ref => "Ref",
        Microsoft.CodeAnalysis.RefKind.Out => "Out",
        Microsoft.CodeAnalysis.RefKind.In => "In",
        Microsoft.CodeAnalysis.RefKind.RefReadOnlyParameter => "RefReadonly",
        _ => "ByValue",
    };

    private void CollectExpressionFact(SemanticModel model, ByteMap map, ExpressionSyntax expression, Conversion conversion)
    {
        if (conversion.IsBoxing ||
            (expression is CastExpressionSyntax cast && model.GetTypeInfo(cast.Type).Type is { } target &&
             model.ClassifyConversion(cast.Expression, target, isExplicitInSource: true).IsBoxing))
        {
            _expressions.Add(new(map.Range(expression.Span), ExpressionFactKind.Boxing));
            return;
        }
        if (expression is InvocationExpressionSyntax invocation && model.GetOperation(invocation) is IDynamicInvocationOperation)
        {
            _expressions.Add(new(map.Range(expression.Span), ExpressionFactKind.Dynamic));
            return;
        }
        if (expression is not (IdentifierNameSyntax or MemberAccessExpressionSyntax or ElementAccessExpressionSyntax))
            return;
        // The context belongs to the outer expression, while the graph uses the operand span.
        var value = expression;
        while (value.Parent is ParenthesizedExpressionSyntax parens)
            value = parens;
        if (value.Parent is not EqualsValueClauseSyntax { Parent: VariableDeclaratorSyntax } &&
            !(value.Parent is AssignmentExpressionSyntax assignment &&
              assignment.IsKind(SyntaxKind.SimpleAssignmentExpression) && assignment.Right == value))
            return;
        if (value != expression)
            conversion = model.GetConversion(value);
        if (!conversion.IsIdentity && !conversion.IsReference)
            return;
        var type = model.GetTypeInfo(value).ConvertedType;
        if (type is null || type.TypeKind is TypeKind.Error or TypeKind.Dynamic or TypeKind.TypeParameter)
            return;
        if (type.IsValueType)
            _expressions.Add(new(map.Range(expression.Span), ExpressionFactKind.ValueCopy));
        else if (type.IsReferenceType)
            _expressions.Add(new(map.Range(expression.Span), ExpressionFactKind.ReferenceCopy));
    }

    private List<SemanticRef> MergeReferences()
    {
        var refs = new List<SemanticRef>();
        var calls = new Dictionary<(ByteRange, RefKind), int>();
        foreach (var fact in _refs.Distinct())
        {
            if (fact.Kind is RefKind.Call or RefKind.Construct)
            {
                if (!calls.TryAdd((fact.Site, fact.Kind), refs.Count))
                {
                    var index = calls[(fact.Site, fact.Kind)];
                    var previous = refs[index].Target;
                    var sameTarget = previous.Decl is not null ? previous.Decl == fact.Target.Decl : previous == fact.Target;
                    if (!sameTarget)
                    {
                        refs[index] = fact with { Target = new SemanticTarget(null, previous.Display, null, "unresolved", false, false) };
                    }
                    continue;
                }
            }
            refs.Add(fact);
        }
        return refs;
    }

    /// Converts UTF-16 positions (Roslyn) to UTF-8 byte offsets (tree-sitter).
    public sealed class ByteMap
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
