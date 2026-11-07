// The same logic as operations.ts: every line must draw the same operation nodes.
using System;
using System.Collections.Generic;

public static class Operations
{
    public static string Run(int a, int b, string name, List<int> items)
    {
        var sum = a + b * 2;
        var big = Math.Max(a, b) >= 10 && !(a == b);
        var missing = name == null;
        var text = name.Trim().ToUpper();
        if (text.StartsWith("X") || text.Contains("Y"))
        {
            text = text.Replace("X", "Z");
        }
        var parts = text.Split(",");
        var joined = string.Join(";", parts);
        var count = items.Count + parts.Length;
        var parsed = int.Parse(joined);
        var root = Math.Sqrt(Math.Abs(a - b)) % 3;
        var i = 0;
        i++;
        i += count;
        items.Add(i);
        Console.WriteLine("Total: " + sum);
        return parsed.ToString();
    }
}
