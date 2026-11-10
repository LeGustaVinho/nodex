using Microsoft.CodeAnalysis;

namespace Nodex.Roslyn;

internal static class BlueprintTypes
{
    internal static object Shape(ITypeSymbol? type, int depth = 0)
    {
        if (depth >= 8 || type is null || type.TypeKind is TypeKind.Dynamic or TypeKind.Error)
            return new { kind = "Unknown" };
        if (type.NullableAnnotation == NullableAnnotation.Annotated && type.IsReferenceType)
            return new { kind = "Union", items = new[] { Core(type, depth), Scalar("Null") } };
        return Core(type, depth);
    }

    private static object Scalar(string name) => new { kind = "Scalar", name };

    private static object Core(ITypeSymbol type, int depth)
    {
        var scalar = type.SpecialType switch
        {
            SpecialType.System_Boolean => "Bool",
            SpecialType.System_String => "String",
            SpecialType.System_Char => "Char",
            SpecialType.System_SByte => "Int8",
            SpecialType.System_Byte => "UInt8",
            SpecialType.System_Int16 => "Int16",
            SpecialType.System_UInt16 => "UInt16",
            SpecialType.System_Int32 => "Int32",
            SpecialType.System_UInt32 => "UInt32",
            SpecialType.System_Int64 => "Int64",
            SpecialType.System_UInt64 => "UInt64",
            SpecialType.System_IntPtr => "IntSize",
            SpecialType.System_UIntPtr => "UIntSize",
            SpecialType.System_Single => "Float32",
            SpecialType.System_Double => "Float64",
            SpecialType.System_Decimal => "Decimal",
            SpecialType.System_Object => "Object",
            SpecialType.System_Void => "Unit",
            _ => null,
        };
        if (scalar is not null) return Scalar(scalar);
        if (type is IArrayTypeSymbol array)
            return new { kind = "Collection", family = "Array", arguments = new[] { Shape(array.ElementType, depth + 1) }, rank = array.Rank > 1 ? array.Rank : (int?)null };
        if (type is IPointerTypeSymbol pointer)
            return new { kind = "Indirect", pointer = true, target = Shape(pointer.PointedAtType, depth + 1) };
        if (type is IFunctionPointerTypeSymbol)
            return new { kind = "Indirect", pointer = true, target = Scalar("Function") };
        if (type is not INamedTypeSymbol named)
            return new { kind = "Named", name = type.Name, arguments = Array.Empty<object>() };
        if (named.IsAnonymousType) return Scalar("Object");
        if (named.IsTupleType)
            return new { kind = "Tuple", items = named.TupleElements.Take(16).Select(element => Shape(element.Type, depth + 1)).ToArray() };
        var args = named.TypeArguments.Take(16).Select(arg => Shape(arg, depth + 1)).ToArray();
        if (named.OriginalDefinition.SpecialType == SpecialType.System_Nullable_T && args.Length == 1)
            return new { kind = "Union", items = new[] { args[0], Scalar("Null") } };
        var definition = named.OriginalDefinition;
        var ns = definition.ContainingNamespace.ToDisplayString();
        if (IsFramework(definition))
        {
            var wide = (ns, definition.MetadataName) switch
            {
                ("System", "Half") => "Float16",
                ("System", "Int128") => "Int128",
                ("System", "UInt128") => "UInt128",
                _ => null,
            };
            if (wide is not null) return Scalar(wide);
            var family = CollectionFamily(ns, definition.MetadataName);
            if (family is not null) return new { kind = "Collection", family, arguments = args };
        }
        return new { kind = "Named", name = named.Name, arguments = args };
    }

    private static bool IsFramework(INamedTypeSymbol type)
    {
        var identity = type.ContainingAssembly.Identity;
        return (identity.Name is "mscorlib" or "System.Private.CoreLib" or "System" or "System.Runtime" or "System.Collections" or "netstandard")
            && !identity.PublicKeyToken.IsDefaultOrEmpty;
    }

    private static string? CollectionFamily(string ns, string name) => (ns, name) switch
    {
        ("System.Collections.Generic", "List`1" or "IList`1" or "IReadOnlyList`1" or "LinkedList`1") => "List",
        ("System.Collections.Generic", "Dictionary`2" or "IDictionary`2" or "IReadOnlyDictionary`2" or "SortedDictionary`2") => "Dictionary",
        ("System.Collections.Generic", "HashSet`1" or "SortedSet`1" or "ISet`1" or "IReadOnlySet`1") => "Set",
        ("System.Collections.Generic", "Queue`1") => "Queue",
        ("System.Collections.Generic", "Stack`1") => "Stack",
        ("System.Collections.Generic", "PriorityQueue`2") => "PriorityQueue",
        ("System", "Span`1" or "ReadOnlySpan`1") => "Slice",
        _ => null,
    };
}
