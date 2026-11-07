using System.Threading;
using System.Threading.Tasks;

namespace Syntax.Statements
{
    public class Statements
    {
        private readonly object _gate = new();

        public int Branching(int x)
        {
            if (x < 0)
            {
                return -1;
            }
            else if (x == 0)
                return 0;
            else
            {
                x--;
            }

            switch (x)
            {
                case 1:
                case 2:
                    x += 10;
                    break;
                case > 100 and < 200 when x % 2 == 0:
                    goto case 1;
                case int n when n > 1000:
                    return n;
                default:
                    goto default_label;
            }

        default_label:
            return x;
        }

        public int Loops(int[] items, Dictionary<string, int> map)
        {
            int total = 0;
            for (int i = 0, j = items.Length - 1; i < j; i++, j--)
            {
                total += items[i] * items[j];
            }
            for (;;)
            {
                if (total > 10) break;
                total++;
            }
            foreach (var (key, value) in map)
            {
                total += key.Length + value;
            }
            foreach (int item in items) total += item;
            while (total > 100) total /= 2;
            do
            {
                total--;
                if (total % 3 == 0) continue;
            } while (total > 50);
            return total;
        }

        public async Task<int> Asynchrony(IAsyncEnumerable<int> stream, CancellationToken token)
        {
            var sum = 0;
            await foreach (var value in stream.WithCancellation(token))
            {
                sum += value;
            }
            await using var res = new AsyncResource();
            await Task.Yield();
            var (a, b) = await TwoAsync();
            return sum + a + b;
        }

        private static Task<(int, int)> TwoAsync() => Task.FromResult((1, 2));

        public async IAsyncEnumerable<int> Produce()
        {
            for (var i = 0; i < 3; i++)
            {
                await Task.Delay(1);
                yield return i;
            }
            yield break;
        }

        public IEnumerable<int> Iterate()
        {
            yield return 1;
            yield return 2;
        }

        public void Scopes()
        {
            lock (_gate)
            {
                Console.WriteLine("locked");
            }
            using (var a = new Resource())
            using (var b = new Resource())
            {
                Console.WriteLine("nested using");
            }
            using var c = new Resource();
            checked
            {
                int big = int.MaxValue;
                big++;
            }
            unchecked { var wrap = int.MaxValue + 1; }
            try
            {
                throw new InvalidOperationException("boom");
            }
            catch (InvalidOperationException ex) when (ex.Message.Length > 0)
            {
                Console.WriteLine(ex.Message);
                throw;
            }
            catch (Exception)
            {
            }
            catch
            {
            }
            finally
            {
                Console.WriteLine("finally");
            }
        }

        public unsafe int Pointers(int[] values)
        {
            fixed (int* p = values)
            {
                int* q = p + 1;
                *q = 42;
                return *p + q[0];
            }
        }

        public void Locals()
        {
            const int Limit = 10;
            int[] arr = { 1, 2, 3 };
            ref int first = ref arr[0];
            ref readonly int second = ref arr[1];
            first = 5;
            (int x, int y) = (1, 2);
            (x, y) = (y, x);
            var (_, z) = (3, 4);
            Span<int> stack = stackalloc int[Limit];
            int localFunction(int v) => v * 2;
            static int StaticLocal(int v) => v + 1;
            async Task AsyncLocal() => await Task.Delay(1);
            x = localFunction(x) + StaticLocal(y);
            ;
        }
    }

    public sealed class Resource : IDisposable
    {
        public void Dispose() { }
    }

    public sealed class AsyncResource : IAsyncDisposable
    {
        public ValueTask DisposeAsync() => ValueTask.CompletedTask;
    }
}
