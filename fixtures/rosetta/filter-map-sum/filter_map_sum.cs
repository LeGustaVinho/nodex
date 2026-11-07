public static class Rosetta
{
    public static int SumOfEvenSquares(int[] numbers)
    {
        int total = 0;
        foreach (var n in numbers)
        {
            if (n % 2 == 0)
            {
                total += n * n;
            }
        }
        return total;
    }
}
