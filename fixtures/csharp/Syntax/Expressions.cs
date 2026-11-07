using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace Syntax.Expressions;

public class Expressions
{
    public record Person(string Name, int Age, Address? Home);
    public record Address(string City);

    public string Patterns(object o, int[] values, Person p)
    {
        var a = o is string { Length: > 3 } s ? s : "short";
        var b = o is not null and not int;
        var c = values is [1, .., var last] ? last : -1;
        var d = p switch
        {
            { Age: < 18 } => "minor",
            { Home.City: "Paris" } => "parisian",
            (_, >= 65, _) => "senior",
            var other when other.Name.Length == 0 => "anonymous",
            _ => "adult",
        };
        var e = o switch
        {
            int i and > 0 => i,
            int or long => 0,
            null => -1,
            _ => throw new ArgumentException(nameof(o)),
        };
        return a + b + c + d + e;
    }

    public object Operators(int[] arr, string? text, Person? p)
    {
        var last = arr[^1];
        var slice = arr[1..^1];
        var all = arr[..];
        var len = text?.Length ?? 0;
        var first = p?.Home?.City;
        var element = arr?[0];
        var forced = text!.Trim();
        text ??= "default";
        var shifted = len << 2 >> 1 >>> 1;
        var cond = len > 3 ? "long" : "short";
        var cast = (long)len;
        var safe = p as object;
        var size = sizeof(int);
        var type = typeof(List<>);
        var def = default(DateTime);
        var name = nameof(Operators);
        var check = checked(len + 1);
        var idx = Index.FromEnd(1);
        return (last, slice, all, first, element, forced, shifted, cond, cast, safe, size, type, def, name, check, idx);
    }

    public void Creation(Person p)
    {
        var list = new List<int> { 1, 2, 3 };
        var dict = new Dictionary<string, int> { ["a"] = 1, ["b"] = 2 };
        List<int> spread = [.. list, 4, 5];
        int[] empty = [];
        var anon = new { p.Name, Total = 3 };
        Person q = new("Ann", 30, null);
        var older = p with { Age = p.Age + 1 };
        var tuple = (Name: "x", Count: 2);
        var arr2 = new int[,] { { 1, 2 }, { 3, 4 } };
        var jagged = new int[2][];
        var implicitArray = new[] { 1, 2 };
        var sb = new StringBuilder().Append("x").Append(1);
    }

    public string Strings(string name, int count)
    {
        var interp = $"Hello {name}, you have {count,5:N0} items";
        var verbatim = @"C:\path\to";
        var raw = """
            {"name": "raw"}
            """;
        var rawInterp = $$"""{"name": "{{name}}"}""";
        var utf8 = "bytes"u8.ToArray();
        var ch = 'x';
        var escaped = "tab\there";
        return interp + verbatim + raw + rawInterp + utf8.Length + ch + escaped;
    }

    public IEnumerable<string> Linq(IEnumerable<Person> people, IEnumerable<Address> addresses)
    {
        var query =
            from person in people
            let city = person.Home?.City ?? "?"
            join address in addresses on city equals address.City into matches
            where person.Age > 18
            orderby person.Name descending, person.Age
            group person by city into byCity
            select $"{byCity.Key}: {byCity.Count()}";
        var method = people.Where(x => x.Age > 1).Select(x => x.Name).OrderBy(n => n).ToList();
        return query.Concat(method);
    }

    public void Lambdas()
    {
        Func<int, int> square = x => x * x;
        Func<int, int, int> add = (a, b) => a + b;
        var parse = static (string s) => int.Parse(s);
        var typed = int (string s) => s.Length;
        var withDefault = (int x = 2) => x + 1;
        Func<int, Task<int>> asyncLambda = async x => { await Task.Delay(x); return x; };
        Action block = () =>
        {
            Console.WriteLine("block");
        };
        Action<int> anonymous = delegate (int v) { Console.WriteLine(v); };
        block();
        anonymous(square(add(1, 2)));
    }

    public unsafe void PointerExpressions()
    {
        int value = 1;
        int* p = &value;
        *p = 2;
        var point = new Point2();
        Point2* pp = &point;
        pp->X = 3;
    }

    public struct Point2
    {
        public int X;
    }
}
