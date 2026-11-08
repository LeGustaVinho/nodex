using Review.Handlers;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddScoped<IThing, Thing>();
builder.Services.AddScoped<IThing, Thing>();
var app = builder.Build();
app.UseExceptionHandler("/error");
app.MapGet("/api/health", HealthHandlers.Check);
app.Run();
