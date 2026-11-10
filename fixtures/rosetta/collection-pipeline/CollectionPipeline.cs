using System.Linq;

public static class CollectionPipeline
{
    public static int SumOfEvenSquares(int[] values) =>
        values.Where(value => value % 2 == 0)
            .Select(value => value * value)
            .Sum();
}
