using Mono.Cecil;
using Mono.Cecil.Cil;
using System.Text.RegularExpressions;
using Mono.Cecil.Rocks;

namespace Nodex.Weaver;

/// Rewrites the methods of one module:
/// - <c>T.Enter(id)</c> and <c>T.Arg(id, i, value)</c> at the start,
/// - <c>T.Step(id, point)</c> before every visible sequence point,
/// - <c>T.Local(id, slot, value)</c> after every store to a named local,
/// - <c>T.Return(value, id)</c> or <c>T.Exit(id)</c> before every <c>ret</c>.
/// Branches and exception-handler boundaries that pointed at a rewritten instruction are moved
/// to the inserted probe, so probes run on every path.
public sealed partial class ModuleWeaver
{
    /// Slots for locals the compiler hoisted into state-machine fields start here.
    private const int HoistedSlotBase = 1000;

    /// `<order>5__2` — a local of an async method or iterator, stored as a state-machine field.
    [GeneratedRegex(@"^<(?<name>[^>]+)>5__\d+$")]
    private static partial Regex HoistedLocal();

    private readonly ModuleDefinition _module;
    private readonly SourceMaps _sources;
    private readonly List<MethodMap> _methods;
    private readonly MethodReference _enter, _step, _exit, _return, _local, _arg;

    public ModuleWeaver(ModuleDefinition module, ModuleDefinition trace, SourceMaps sources, List<MethodMap> methods)
    {
        _module = module;
        _sources = sources;
        _methods = methods;
        var t = trace.GetType("Nodex.Trace.T");
        MethodReference Import(string name) => module.ImportReference(t.Methods.First(m => m.Name == name));
        _enter = Import("Enter");
        _step = Import("Step");
        _exit = Import("Exit");
        _return = Import("Return");
        _local = Import("Local");
        _arg = Import("Arg");
    }

    /// Returns true when at least one method was instrumented.
    public bool Weave()
    {
        var any = false;
        foreach (var type in _module.GetTypes().ToList())
        {
            if (type.Namespace == "Nodex.Trace") continue;
            foreach (var method in type.Methods.Where(m => m.HasBody).ToList())
            {
                any |= WeaveMethod(method);
            }
        }
        return any;
    }

    private bool WeaveMethod(MethodDefinition method)
    {
        var debug = method.DebugInformation;
        if (!debug.HasSequencePoints) return false;
        var body = method.Body;

        // Visible sequence points inside the project, with their instructions (before edits).
        var points = new List<(Instruction Instruction, PointMap Map)>();
        var byOffset = body.Instructions.ToDictionary(i => i.Offset);
        foreach (var sp in debug.SequencePoints)
        {
            if (sp.IsHidden || !byOffset.TryGetValue(sp.Offset, out var ins)) continue;
            var map = _sources.Map(sp);
            if (map is not null) points.Add((ins, map));
        }
        if (points.Count == 0) return false;

        var id = _methods.Count + 1;
        var locals = new Dictionary<int, string>();
        if (debug.Scope is { } scope) CollectLocals(scope, locals);
        var kind = IsStateMachine(method.DeclaringType) ? "resume" : method.IsConstructor ? "constructor" : "method";
        // In async methods and iterators, locals live in fields of the state machine.
        var hoisted = new Dictionary<FieldDefinition, int>();
        if (kind == "resume")
        {
            var fields = method.DeclaringType.Fields.ToList();
            for (var f = 0; f < fields.Count; f++)
            {
                var m = HoistedLocal().Match(fields[f].Name);
                if (m.Success)
                {
                    hoisted[fields[f]] = HoistedSlotBase + f;
                    locals[HoistedSlotBase + f] = m.Groups["name"].Value;
                }
            }
        }
        _methods.Add(new MethodMap(id, $"{method.DeclaringType.FullName}::{method.Name}", kind,
            points.Select(p => p.Map).ToList(), locals, method.Parameters.Select(p => p.Name).ToList()));

        // Hoisted-local stores that belong to user statements. Stores under a hidden sequence point
        // are compiler bookkeeping (for example clearing fields when the state machine finishes).
        var userStores = new HashSet<Instruction>();
        if (hoisted.Count > 0)
        {
            var ordered = debug.SequencePoints.OrderBy(sp => sp.Offset).ToList();
            foreach (var ins in body.Instructions.Where(i => i.OpCode == OpCodes.Stfld))
            {
                var governing = ordered.LastOrDefault(sp => sp.Offset <= ins.Offset);
                if (governing is not null && !governing.IsHidden) userStores.Add(ins);
            }
        }

        body.SimplifyMacros();
        var il = body.GetILProcessor();

        // Steps.
        for (var i = 0; i < points.Count; i++)
        {
            InsertBefore(body, points[i].Instruction, [il.Create(OpCodes.Ldc_I4, id), il.Create(OpCodes.Ldc_I4, i), il.Create(OpCodes.Call, _step)]);
        }

        // Local stores.
        foreach (var store in body.Instructions.Where(i => i.OpCode == OpCodes.Stloc).ToList())
        {
            var variable = (VariableDefinition)store.Operand;
            if (!locals.ContainsKey(variable.Index) || !CanBox(variable.VariableType)) continue;
            var code = new List<Instruction>
            {
                il.Create(OpCodes.Ldc_I4, id),
                il.Create(OpCodes.Ldc_I4, variable.Index),
                il.Create(OpCodes.Ldloc, variable),
            };
            if (NeedsBox(variable.VariableType)) code.Add(il.Create(OpCodes.Box, _module.ImportReference(variable.VariableType)));
            code.Add(il.Create(OpCodes.Call, _local));
            var after = store;
            foreach (var c in code)
            {
                il.InsertAfter(after, c);
                after = c;
            }
        }

        // Stores to hoisted locals: `stfld <x>5__N` → reload the field and report it.
        if (hoisted.Count > 0)
        {
            foreach (var store in body.Instructions.Where(userStores.Contains).ToList())
            {
                var field = (FieldReference)store.Operand;
                var definition = field.Resolve();
                if (definition is null || !hoisted.TryGetValue(definition, out var slot) || !CanBox(field.FieldType)) continue;
                var code = new List<Instruction>
                {
                    il.Create(OpCodes.Ldc_I4, id),
                    il.Create(OpCodes.Ldc_I4, slot),
                    il.Create(OpCodes.Ldarg_0),
                    il.Create(OpCodes.Ldfld, field),
                };
                if (NeedsBox(field.FieldType)) code.Add(il.Create(OpCodes.Box, field.FieldType));
                code.Add(il.Create(OpCodes.Call, _local));
                var after = store;
                foreach (var c in code)
                {
                    il.InsertAfter(after, c);
                    after = c;
                }
            }
        }

        // Returns.
        var returnType = method.ReturnType;
        var captureReturn = returnType.MetadataType != MetadataType.Void && CanBox(returnType) && kind != "resume";
        foreach (var ret in body.Instructions.Where(i => i.OpCode == OpCodes.Ret).ToList())
        {
            var code = new List<Instruction>();
            if (captureReturn)
            {
                code.Add(il.Create(OpCodes.Dup));
                if (NeedsBox(returnType)) code.Add(il.Create(OpCodes.Box, _module.ImportReference(returnType)));
                code.Add(il.Create(OpCodes.Ldc_I4, id));
                code.Add(il.Create(OpCodes.Call, _return));
            }
            else
            {
                code.Add(il.Create(OpCodes.Ldc_I4, id));
                code.Add(il.Create(OpCodes.Call, _exit));
            }
            InsertBefore(body, ret, code);
        }

        // Entry (not a branch target: loops back to the first statement skip it).
        var first = body.Instructions[0];
        var entry = new List<Instruction> { il.Create(OpCodes.Ldc_I4, id), il.Create(OpCodes.Call, _enter) };
        if (kind != "resume")
        {
            for (var p = 0; p < method.Parameters.Count; p++)
            {
                var parameter = method.Parameters[p];
                if (!CanBox(parameter.ParameterType)) continue;
                entry.Add(il.Create(OpCodes.Ldc_I4, id));
                entry.Add(il.Create(OpCodes.Ldc_I4, p));
                entry.Add(il.Create(OpCodes.Ldarg, method.Parameters[p]));
                if (NeedsBox(parameter.ParameterType)) entry.Add(il.Create(OpCodes.Box, _module.ImportReference(parameter.ParameterType)));
                entry.Add(il.Create(OpCodes.Call, _arg));
            }
        }
        // Handlers that start at `first` keep pointing at it, so Enter stays outside any try.
        foreach (var c in entry) il.InsertBefore(first, c);

        body.OptimizeMacros();
        return true;
    }

    /// Inserts `code` before `target` and moves every reference to `target` onto the new code.
    private static void InsertBefore(MethodBody body, Instruction target, List<Instruction> code)
    {
        var il = body.GetILProcessor();
        foreach (var c in code) il.InsertBefore(target, c);
        var head = code[0];
        foreach (var ins in body.Instructions)
        {
            if (ReferenceEquals(ins.Operand, target) && !code.Contains(ins)) ins.Operand = head;
            else if (ins.Operand is Instruction[] targets)
            {
                for (var i = 0; i < targets.Length; i++)
                {
                    if (ReferenceEquals(targets[i], target)) targets[i] = head;
                }
            }
        }
        foreach (var h in body.ExceptionHandlers)
        {
            if (h.TryStart == target) h.TryStart = head;
            if (h.TryEnd == target) h.TryEnd = head;
            if (h.HandlerStart == target) h.HandlerStart = head;
            if (h.HandlerEnd == target) h.HandlerEnd = head;
            if (h.FilterStart == target) h.FilterStart = head;
        }
    }

    private static void CollectLocals(ScopeDebugInformation scope, Dictionary<int, string> locals)
    {
        foreach (var v in scope.Variables)
        {
            if (!v.IsDebuggerHidden && !string.IsNullOrEmpty(v.Name) && !v.Name.StartsWith("CS$", StringComparison.Ordinal))
            {
                locals[v.Index] = v.Name;
            }
        }
        foreach (var inner in scope.Scopes) CollectLocals(inner, locals);
    }

    private static bool IsStateMachine(TypeDefinition type) =>
        type.Interfaces.Any(i => i.InterfaceType.FullName is "System.Runtime.CompilerServices.IAsyncStateMachine" or "System.Collections.IEnumerator");

    private static bool NeedsBox(TypeReference type) => type.IsValueType || type.IsGenericParameter;

    /// False for types that cannot be boxed: by-ref, pointers, pinned, and ref structs (Span&lt;T&gt;).
    private static bool CanBox(TypeReference type)
    {
        if (type.IsByReference || type.IsPointer || type.IsPinned || type.IsFunctionPointer) return false;
        if (!type.IsValueType || type.IsGenericParameter) return true;
        try
        {
            var definition = type.Resolve();
            if (definition is null) return false;
            return !definition.CustomAttributes.Any(a => a.AttributeType.FullName == "System.Runtime.CompilerServices.IsByRefLikeAttribute");
        }
        catch (AssemblyResolutionException)
        {
            return false;
        }
    }
}
