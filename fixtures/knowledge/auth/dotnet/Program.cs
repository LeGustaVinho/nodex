using Auth.Services;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddScoped<IReportService, ReportService>();
builder.Services.AddSingleton<IClock, SystemClock>();
builder.Services.AddControllers();

var app = builder.Build();

var ops = app.MapGroup("/ops").RequireAuthorization("Operators");
ops.MapGet("/status", Handlers.Status);
ops.MapPost("/restart", Handlers.Restart).RequireAuthorization(p => p.RequireRole("Admin"));

app.MapGet("/ping", () => "pong").AllowAnonymous();
app.MapControllers();
app.Run();

class SystemClock : IClock
{
    public System.DateTime Now => System.DateTime.UtcNow;
}

static class Handlers
{
    public static string Status() => "ok";
    public static string Restart() => "restarting";
}
