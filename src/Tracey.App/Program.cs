using BlazorBlueprint.Components;
using Microsoft.AspNetCore.Components.Web;
using Microsoft.AspNetCore.Components.WebAssembly.Hosting;
using Tracey.App;
using Tracey.App.Services;

var builder = WebAssemblyHostBuilder.CreateDefault(args);
builder.RootComponents.Add<App>("#app");
builder.RootComponents.Add<HeadOutlet>("head::after");

builder.Services.AddBlazorBlueprintComponents();
builder.Services.AddScoped(sp => new HttpClient { BaseAddress = new Uri(builder.HostEnvironment.BaseAddress) });
builder.Services.AddScoped<TauriIpcService>();
builder.Services.AddScoped<TauriEventService>();
builder.Services.AddScoped<ITimerStateService, TimerStateService>();
builder.Services.AddScoped<FuzzyMatchService>();

await builder.Build().RunAsync();
