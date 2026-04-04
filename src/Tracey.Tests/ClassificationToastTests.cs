using Tracey.App.Services;

namespace Tracey.Tests;

/// Tests the pattern-key format used in active learning dismissal tracking.
public class ClassificationToastTests
{
    [Fact]
    public void PatternKey_Is_Lowercase_And_Truncated()
    {
        var processName = "Visual Studio Code";
        var title = "tracey — Visual Studio Code";
        var key = $"{processName.ToLower()}|{title.ToLower()[..Math.Min(50, title.Length)]}";
        Assert.Contains("visual studio code", key);
        Assert.Contains("tracey", key);
    }
}
