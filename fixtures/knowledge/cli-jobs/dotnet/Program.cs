using System.CommandLine;
using Hangfire;

namespace Jobs;

public static class Program
{
    public static int Main(string[] args)
    {
        var root = new RootCommand("Inventory tool");
        var import = new Command("import", "Imports a CSV file");
        import.SetHandler(ImportFile);
        root.AddCommand(import);

        var report = new Command("report", "Prints the stock report");
        report.SetHandler(() =>
        {
            Console.WriteLine("report");
        });
        root.AddCommand(report);

        RecurringJob.AddOrUpdate<IStockReport>("nightly-stock", r => r.Send(), Cron.Daily);
        return root.Invoke(args);
    }

    static void ImportFile()
    {
        Console.WriteLine("importing");
    }
}

public interface IStockReport
{
    void Send();
}

public class StockReport : IStockReport
{
    public void Send() { }
}

public class CleanupWorker : BackgroundService
{
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        while (!stoppingToken.IsCancellationRequested)
        {
            await Task.Delay(1000, stoppingToken);
        }
    }
}

public record OrderPlaced(int Id);

public class OrderPlacedConsumer : IConsumer<OrderPlaced>
{
    public Task Consume(ConsumeContext<OrderPlaced> context) => Task.CompletedTask;
}
