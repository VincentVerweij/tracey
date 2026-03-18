using BlazorBlueprint.Components;
using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using Tracey.App;
using Tracey.App.Services;
using Tracey.App.Services.Notifications;

var builder = WebAssemblyHostBuilder.CreateDefault(args);
builder.RootComponents.Add<App>("#app");
builder.RootComponents.Add<HeadOutlet>("head::after");

builder.Services.AddBlazorBlueprintComponents();
builder.Services.AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) });
builder.Services.AddHttpClient(); // IHttpClientFactory for TelegramNotificationChannel
builder.Services.AddScoped<TauriIpcService>();
builder.Services.AddScoped<TauriEventService>();
builder.Services.AddScoped<ITimerStateService, TimerStateService>();
builder.Services.AddScoped<FuzzyMatchService>();

// Notification channels (FR-054: add new channels by registering here only)
builder.Services.AddSingleton<INotificationChannel, EmailNotificationChannel>();
builder.Services.AddSingleton<INotificationChannel, TelegramNotificationChannel>();
builder.Services.AddHostedService<NotificationOrchestrationService>();

await builder.Build().RunAsync();
