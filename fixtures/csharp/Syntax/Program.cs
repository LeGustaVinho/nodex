// Top-level statements: the program entry point without a Main method.
using System.Threading.Tasks;
using Syntax.Advanced.Inner;

var total = UsesHidden.Read();
if (args.Length > 0)
{
    total += args.Length;
}
Console.WriteLine(Describe(total));
await Task.Delay(1);
return total > 0 ? 0 : 1;

static string Describe(int value) => $"total = {value}";
