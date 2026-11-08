namespace States;

/// <summary>A local variable that drives a loop (a small lexer).</summary>
public static class Lexer
{
    private enum Mode { Start, Number, Word }

    public static int CountTokens(string text)
    {
        var mode = Mode.Start;
        var count = 0;
        foreach (var c in text)
        {
            switch (mode)
            {
                case Mode.Start:
                    if (char.IsDigit(c))
                    {
                        mode = Mode.Number;
                        count++;
                    }
                    else if (char.IsLetter(c))
                    {
                        mode = Mode.Word;
                        count++;
                    }
                    break;
                case Mode.Number:
                    if (!char.IsDigit(c))
                        mode = Mode.Start;
                    break;
                case Mode.Word:
                    if (!char.IsLetterOrDigit(c))
                        mode = Mode.Start;
                    break;
            }
        }
        return count;
    }
}
