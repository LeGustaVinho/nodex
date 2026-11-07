using Pair = (int Left, int Right); // C# 12 alias any type
using System.Numerics;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace Syntax.Advanced
{
    namespace Inner
    {
        /// <summary>A type visible only in this file.</summary>
        file sealed class Hidden
        {
            public int Value;
        }

        public static class UsesHidden
        {
            public static int Read() => new Hidden { Value = 3 }.Value;
        }
    }

    /// <summary>
    /// A money amount with checked arithmetic.
    /// </summary>
    /// <remarks>Used by the syntax coverage fixture.</remarks>
    public struct Money : IAdditionOperators<Money, Money, Money>
    {
        public decimal Amount, Fee = 0m;
        public static readonly Money Zero = new(0);

        public Money(decimal amount) => Amount = amount;

        public static Money operator +(Money a, Money b) => new(a.Amount + b.Amount);
        public static Money operator checked +(Money a, Money b) => new(checked(a.Amount + b.Amount));
        public static bool operator true(Money m) => m.Amount != 0;
        public static bool operator false(Money m) => m.Amount == 0;
        public void operator +=(Money other) => Amount += other.Amount; // C# 14 compound assignment operator
        public readonly override string ToString() => $"{Amount:C}";
    }

    public class Grid<T> where T : unmanaged
    {
        private readonly T[] _cells = new T[16];
        public ref T this[int x, int y] => ref _cells[x * 4 + y];

        public ref readonly T First() => ref _cells[0];

        public static int Measure<TValue>(scoped TValue value) where TValue : allows ref struct
        {
            return 0;
        }
    }

    public partial class Widget
    {
        public partial Widget(int size); // C# 14 partial constructor
        public partial event Action? Resized; // C# 14 partial event
    }

    public partial class Widget
    {
        private int _size;
        private Action? _resized;

        public partial Widget(int size)
        {
            _size = size;
        }

        public partial event Action? Resized
        {
            add => _resized += value;
            remove => _resized -= value;
        }

        public event Action? Shown, Hidden;

        [DllImport("user32.dll")]
        public static extern int MessageBeep(uint type);

        public async void OnClick(object? sender, EventArgs e) => await Task.Yield();
    }

    public class Node
    {
        public Node? Next;
        public int Value;
        public List<int> Items { get; } = new();
    }

    public unsafe class Advanced
    {
        public int Statements(Node? node, int[] values, Span<int> span, (int, int) pair)
        {
            node?.Items.Add(1);
            node?.Next?.Items.Clear();
            node?.Value = 5; // C# 14 null-conditional assignment
            values[0] |= 0b1010;
            values[1] <<= 2;
            values[2] >>>= 1;
            var big = 1_000_000L + 0x1F + 10UL.GetHashCode() + (int)1e3;
            global::System.Console.WriteLine(big);

            switch (pair)
            {
                case (0, 0):
                    return 0;
                case (var a, > 10) when a > 0:
                    goto default;
                default:
                    break;
            }

            foreach (ref var cell in span)
            {
                cell++;
            }

            try
            {
                values[0] = 1;
            }
            finally
            {
                values[1] = 2;
            }

            Pair p = (1, 2);
            ref int pick = ref (p.Left > 0 ? ref values[0] : ref values[1]);
            Span<byte> buffer = stackalloc[] { (byte)1, (byte)2 };
            delegate*<int, int> twice = &Twice;
            Func<int, int> group = Twice;
            var attributed = [Obsolete] (int x) => x;
            dynamic loose = 3;
            object? maybe = default;
            int defaulted = default;
            return twice(pick) + group(buffer.Length) + loose + defaulted;
        }

        private static int Twice(int x) => x * 2;

        public IEnumerable<int> GuardedIterator()
        {
            try
            {
                yield return 1;
            }
            finally
            {
                Console.WriteLine("cleanup");
            }
        }
    }
}
