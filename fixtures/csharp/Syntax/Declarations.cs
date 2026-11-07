global using System;
global using System.Collections.Generic;
using Alias = System.Collections.Generic.Dictionary<string, int>;
using static System.Math;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;

[assembly: System.Reflection.AssemblyMetadata("nodex", "syntax")]

namespace Syntax.Declarations;

[Flags]
public enum Permissions : byte
{
    None = 0,
    Read = 1 << 0,
    Write = 1 << 1,
    All = Read | Write,
}

public delegate TResult Transformer<in T, out TResult>(T input);

public interface IShape
{
    double Area { get; }
    string Name => GetType().Name; // default interface member
    static abstract IShape Unit { get; } // static abstract member
    static virtual string Describe(IShape s) => $"{s.Name}: {s.Area}";
}

public readonly record struct Point(double X, double Y)
{
    public double Length => Sqrt(X * X + Y * Y);
    public static Point operator +(Point a, Point b) => new(a.X + b.X, a.Y + b.Y);
    public static implicit operator (double, double)(Point p) => (p.X, p.Y);
    public static explicit operator Point(double v) => new(v, v);
}

public abstract record Animal(string Name)
{
    public abstract string Sound();
}

public sealed record Dog(string Name, int Age) : Animal(Name)
{
    public override string Sound() => "woof";
}

public sealed class Square(double side) : IShape
{
    public double Area => side * side;
    public static IShape Unit => new Square(1);
}

public ref struct Buffer
{
    public Span<byte> Data;
    public Buffer(Span<byte> data) => Data = data;
}

public unsafe struct Fixed
{
    public fixed byte Bytes[16];
}

public class Container<T> : IEnumerable<T> where T : class, IComparable<T>, new()
{
    private readonly List<T> _items = [];
    private static readonly object s_lock = new();
    private volatile int _version;
    public const int Capacity = 32;

    public required string Label { get; init; }
    public int Count { get; private set; }
    public string Title
    {
        get => field ?? "untitled"; // C# 14 field keyword
        set => field = value?.Trim();
    }

    public T this[int index]
    {
        get => _items[index];
        set
        {
            _items[index] = value;
            _version++;
        }
    }

    public event EventHandler? Changed
    {
        add { lock (s_lock) { _changed += value; } }
        remove { lock (s_lock) { _changed -= value; } }
    }
    private EventHandler? _changed;

    public Container() { }

    static Container() { }

    ~Container() { }

    public void Add(T item) => _items.Add(item);

    public partial class Nested
    {
        public partial void Hook(int value);
        public partial void Hook(int value) { Console.WriteLine(value); }
    }

    public IEnumerator<T> GetEnumerator() => _items.GetEnumerator();
    System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => GetEnumerator();

    public TOut Map<TOut>(Func<T, TOut> map) where TOut : notnull => map(_items[0]);

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    [return: System.Diagnostics.CodeAnalysis.NotNull]
    public static string Name(in int id, ref int counter, out string text, params int[] rest)
    {
        counter++;
        text = id.ToString();
        return text;
    }
}

public static class Extensions
{
    public static bool IsBlank(this string? text) => string.IsNullOrWhiteSpace(text);

    // C# 14 extension members
    extension(string text)
    {
        public int WordCount => text.Split(' ').Length;
        public string Shout() => text.ToUpperInvariant() + "!";
    }
}

public partial class Split
{
    public partial string Part { get; set; } // C# 13 partial property
}

public partial class Split
{
    private string _part = "";
    public partial string Part { get => _part; set => _part = value; }
}
