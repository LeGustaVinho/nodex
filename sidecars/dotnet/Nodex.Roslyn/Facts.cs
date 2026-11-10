namespace Nodex.Roslyn;

// JSON contract shared with crates/nodex-ir/src/semantic.rs (snake_case on the wire).

public sealed record ByteRange(string File, int Start, int End);

public enum RefKind
{
    Call,
    Construct,
    Base,
    Subscribe,
    Unsubscribe,
    Handler,
    Raise,
}

public sealed record SemanticTarget(
    ByteRange? Decl,
    string Display,
    string? Container,
    string Kind,
    bool Linq,
    bool External);

public sealed record SemanticRef(ByteRange Site, RefKind Kind, SemanticTarget Target);

public sealed record NavigationFact(ByteRange Site, SemanticTarget Target, bool Implementation = false);

public sealed record LocalTypeFact(ByteRange Site, string Name, string Ty);
public sealed record BindingUseFact(ByteRange Site, ByteRange Declaration);

public enum ExpressionFactKind { Boxing, ValueCopy, ReferenceCopy, Dynamic }
public sealed record ExpressionFact(ByteRange Site, ExpressionFactKind Kind);

public sealed record ValueInfo(string Kind, string Basis, string? Qualifier, string? Passing);
public sealed record ValueFact(ByteRange Site, string Target, ValueInfo Info);
public sealed record TypeInfo(object Shape, string Origin, string? Native);
public sealed record TypeFact(ByteRange Site, string Target, TypeInfo Info);

public sealed record SemanticFacts(
    string Source,
    List<SemanticRef> Refs,
    List<LocalTypeFact> Locals,
    List<ExpressionFact> Expressions,
    List<ValueFact> Values,
    List<TypeFact> Types,
    List<BindingUseFact> BindingUses,
    List<NavigationFact> Navigation,
    List<string> Messages);
