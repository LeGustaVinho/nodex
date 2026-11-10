using System.Collections.Generic;

namespace BlueprintTypesExample
{
    public sealed class UniversalTypes
    {
        public Dictionary<string, List<int[]>> Buckets { get; } = new();

        public int Collections()
        {
            var items = new List<int>();
            var own = new Custom.List<int>();
            var rectangle = new double[2, 3];
            var fifo = new LinkedList<int>();
            fifo.AddLast(1);
            fifo.RemoveFirst();
            var captured = new LinkedList<int>(new[] { 0 });
            captured.AddLast(1);
            captured.RemoveFirst();
            void RemoveLast() { captured.RemoveLast(); }
            RemoveLast();
            var loopUpdate = new LinkedList<int>(new[] { 0 });
            loopUpdate.AddLast(1);
            loopUpdate.RemoveFirst();
            for (int i = 0; i < 1; i++, loopUpdate.RemoveLast()) { }
            var (count, title) = (42L, "items");
            var anonymous = new { Count = 3 };
            int updated;
            Set(out updated);
            return items.Count + own.Count + rectangle.Rank + (int)count + title.Length + anonymous.Count + updated;
        }

        private static void Set(out int value) => value = 1;
    }
}

namespace Custom
{
    public sealed class List<T>
    {
        public int Count => 0;
    }
}
