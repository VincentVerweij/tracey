using Tracey.App.TimelineModel;

namespace Tracey.Tests;

/// <summary>
/// The visible part of the viewed day on the Timeline bar (#124, limits from #107).
/// </summary>
public class ZoomRangeTests
{
    private static TimeSpan H(double hours) => TimeSpan.FromHours(hours);
    private static TimeSpan M(double minutes) => TimeSpan.FromMinutes(minutes);

    // ─── Clamping ───────────────────────────────────────────────────────────

    [Fact]
    public void FullDay_IsMidnightToMidnight()
    {
        Assert.Equal(TimeSpan.Zero, ZoomRange.FullDay.Start);
        Assert.Equal(H(24), ZoomRange.FullDay.Length);
        Assert.True(ZoomRange.FullDay.IsFullDay);
    }

    [Fact]
    public void Create_KeepsAValidRange()
    {
        var r = ZoomRange.Create(H(8), H(5.5));
        Assert.Equal(H(8), r.Start);
        Assert.Equal(H(13.5), r.End);
        Assert.False(r.IsFullDay);
    }

    [Fact]
    public void Create_LimitsLengthToFifteenMinutes()
    {
        Assert.Equal(M(15), ZoomRange.Create(H(8), M(5)).Length);
    }

    [Fact]
    public void Create_LimitsLengthToTheDay()
    {
        var r = ZoomRange.Create(H(3), H(30));
        Assert.Equal(ZoomRange.FullDay, r);
    }

    [Fact]
    public void Create_ClampsStartIntoTheDay()
    {
        Assert.Equal(TimeSpan.Zero, ZoomRange.Create(H(-2), H(4)).Start);
        Assert.Equal(H(20), ZoomRange.Create(H(22), H(4)).Start);
    }

    [Fact]
    public void Create_ALengthWithinAMinuteOfTheDay_IsTheFullDay()
    {
        Assert.True(ZoomRange.Create(TimeSpan.Zero, H(24) - TimeSpan.FromSeconds(20)).IsFullDay);
    }

    [Fact]
    public void MoveTo_KeepsTheLengthAndStaysInTheDay()
    {
        var r = ZoomRange.Create(H(8), H(2)).MoveTo(H(23));
        Assert.Equal(H(22), r.Start);
        Assert.Equal(H(2), r.Length);
    }

    // ─── Wheel zoom ─────────────────────────────────────────────────────────

    [Fact]
    public void ZoomIn_DividesTheLengthByOneAndAHalfAroundTheAnchor()
    {
        // 08:00–14:00, anchor 11:00 (the middle) → 4 h around 11:00.
        var r = ZoomRange.Create(H(8), H(6)).ZoomIn(H(11));
        Assert.Equal(H(9), r.Start);
        Assert.Equal(H(4), r.Length);
    }

    [Fact]
    public void ZoomOut_MultipliesTheLengthByOneAndAHalfKeepingTheAnchorInPlace()
    {
        // 08:00–12:00, anchor 09:00 is a quarter in → 6 h with 09:00 still a quarter in.
        var r = ZoomRange.Create(H(8), H(4)).ZoomOut(H(9));
        Assert.Equal(H(7.5), r.Start);
        Assert.Equal(H(6), r.Length);
    }

    [Fact]
    public void ZoomIn_StopsAtFifteenMinutes()
    {
        Assert.Equal(M(15), ZoomRange.Create(H(8), M(20)).ZoomIn(H(8)).Length);
    }

    [Fact]
    public void ZoomOut_StopsAtTheFullDay()
    {
        Assert.Equal(ZoomRange.FullDay, ZoomRange.Create(H(8), H(20)).ZoomOut(H(10)));
    }

    [Fact]
    public void ZoomOut_NearTheEdge_StaysInTheDay()
    {
        var r = ZoomRange.Create(H(20), H(4)).ZoomOut(H(23));
        Assert.Equal(H(24), r.End);
        Assert.Equal(H(6), r.Length);
    }

    [Fact]
    public void ZoomInThenOut_ReturnsToTheFullDay()
    {
        var r = ZoomRange.FullDay;
        for (var i = 0; i < 7; i++) r = r.ZoomIn(M(10 * 60 + 7));
        for (var i = 0; i < 9; i++) r = r.ZoomOut(M(10 * 60 + 7));
        Assert.True(r.IsFullDay);
    }

    // ─── Range text ─────────────────────────────────────────────────────────

    [Fact]
    public void RangeText_IsStartAndEndAsClockTimes()
    {
        Assert.Equal("08:00–13:30", ZoomRange.Create(H(8), H(5.5)).RangeText);
    }

    [Fact]
    public void RangeText_FullDay_EndsAtTwentyFour()
    {
        Assert.Equal("00:00–24:00", ZoomRange.FullDay.RangeText);
    }

    [Fact]
    public void RangeText_RoundsToTheNearestMinute()
    {
        Assert.Equal("08:01–08:16", ZoomRange.Create(H(8) + TimeSpan.FromSeconds(40), M(15)).RangeText);
    }

    // ─── Bar position ───────────────────────────────────────────────────────

    [Fact]
    public void PercentOf_And_OffsetAt_MapBetweenTimeAndBarPosition()
    {
        var r = ZoomRange.Create(H(8), H(4));
        Assert.Equal(25.0, r.PercentOf(H(9)), precision: 6);
        Assert.Equal(H(11), r.OffsetAt(75.0));
    }

    // ─── Hour-label step ────────────────────────────────────────────────────

    [Theory]
    [InlineData(24 * 60, 120)]
    [InlineData(18 * 60, 120)]
    [InlineData(12 * 60, 60)]
    [InlineData(10 * 60, 60)]
    [InlineData(6 * 60, 30)]
    [InlineData(3 * 60, 15)]
    [InlineData(15, 15)]
    public void LabelStep_IsTheSmallestStepWithAtMostTwelveLabels(int lengthMinutes, int expectedStepMinutes)
    {
        var r = ZoomRange.Create(TimeSpan.Zero, M(lengthMinutes));
        Assert.Equal(M(expectedStepMinutes), r.LabelStep);
    }

    [Fact]
    public void LabelTimes_AreStepMultiplesInsideTheRange()
    {
        var r = ZoomRange.Create(M(8 * 60 + 10), M(80));
        Assert.Equal([M(8 * 60 + 15), M(8 * 60 + 30), M(8 * 60 + 45), M(9 * 60), M(9 * 60 + 15)], r.LabelTimes());
    }

    [Fact]
    public void LabelTimes_FullDay_AreEveryTwoHoursFromMidnight()
    {
        var times = ZoomRange.FullDay.LabelTimes().ToList();
        Assert.Equal(12, times.Count);
        Assert.Equal(TimeSpan.Zero, times[0]);
        Assert.Equal(H(22), times[^1]);
    }
}
