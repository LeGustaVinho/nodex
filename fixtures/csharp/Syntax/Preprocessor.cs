namespace Syntax.Preprocessor;

public class Conditional
{
#if DEBUG
    public int DebugOnly() => 1;
#else
    public int ReleaseOnly() => 2;
#endif

    #region Helpers
    public void Run(int x, Dictionary<string, int> map, object o)
    {
#if DEBUG
        Console.WriteLine("debug");
#elif TRACE
        Console.WriteLine("trace");
#else
        Console.WriteLine("release");
#endif
        x++;
#pragma warning disable CS0168
        int unused;
#pragma warning restore CS0168
        if (map.TryGetValue("a", out var found))
        {
            Console.WriteLine(found);
        }
        if (o is string text && text.Length > 0)
        {
            Console.WriteLine(text);
        }
#if UNITY_EDITOR
        Console.WriteLine("editor only");
#endif
    }
    #endregion
}
