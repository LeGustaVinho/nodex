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

public sealed record SemanticFacts(
    string Source,
    List<SemanticRef> Refs,
    List<LocalTypeFact> Locals,
    List<NavigationFact> Navigation,
    List<string> Messages);
