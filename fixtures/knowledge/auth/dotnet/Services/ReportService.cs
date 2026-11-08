namespace Auth.Services;

public interface IReportService
{
    string Build();
    void RecordAccess(string what);
}

public interface IClock
{
    System.DateTime Now { get; }
}

public class ReportService : IReportService
{
    private readonly IClock _clock;

    public ReportService(IClock clock)
    {
        _clock = clock;
    }

    public string Build() => $"report at {_clock.Now}";

    private readonly System.Collections.Generic.List<string> _accesses = new();

    public void RecordAccess(string what)
    {
        _accesses.Add($"{_clock.Now}: {what}");
    }
}

/// Never registered: its constructor parameters are not injected by the container.
public class Formatter(IClock clock)
{
    public string Stamp() => clock.Now.ToString();
}
