using System;
using System.Collections.Generic;
using System.Linq;

public static class LanguageDetails
{
    public static readonly List<int> History = new();

    public static object ReadDynamic(dynamic input) => input.Get();

    public static IEnumerable<object> Explain(Count source, Settings settings, int limit)
    {
        const int step = 1;
        Count copy;
        copy = ((source));
        var shared = (History);
        var renamed = settings with { Name = "example" };
        int number = copy.Value;
        Update(ref number, in limit, out var total);
        try { total = checked(total + step); }
        catch (OverflowException) when (total > 0) { total = limit; }
        var items = new[] { total, number }.Where(n => n > 0).ToList();
        shared.Add(items.Count);
        object boxed = total;
        yield return boxed;
        yield return renamed;
    }

    public static unsafe int ReadPointer(int* value) => *value;

    private static void Update(ref int value, in int limit, out int result)
    {
        value += 1;
        result = Math.Min(value, limit);
    }
}
