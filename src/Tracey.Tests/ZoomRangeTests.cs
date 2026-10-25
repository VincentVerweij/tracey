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

    // ─── Pan ────────────────────────────────────────────────────────────────

    [Fact]
    public void Pan_MovesByTheDeltaKeepingTheLength()
    {
        var r = ZoomRange.Create(H(8), H(2)).Pan(M(-90));
        Assert.Equal(M(6 * 60 + 30), r.Start);
        Assert.Equal(H(2), r.Length);
    }

    [Fact]
    public void Pan_StopsAtTwentyFour()
    {
        var r = ZoomRange.Create(H(8), H(2)).Pan(H(15));
        Assert.Equal(H(22), r.Start);
        Assert.Equal(H(24), r.End);
    }

    [Fact]
    public void Pan_StopsAtMidnight()
    {
        var r = ZoomRange.Create(H(1), H(2)).Pan(H(-3));
        Assert.Equal(TimeSpan.Zero, r.Start);
        Assert.Equal(H(2), r.Length);
    }

    // ─── Resize one edge ────────────────────────────────────────────────────

    [Fact]
    public void ResizeStart_MovesTheStartKeepingTheEnd()
    {
        var r = ZoomRange.Create(H(8), H(4)).ResizeStart(H(6));
        Assert.Equal(H(6), r.Start);
        Assert.Equal(H(12), r.End);
    }

    [Fact]
    public void ResizeEnd_MovesTheEndKeepingTheStart()
    {
        var r = ZoomRange.Create(H(8), H(4)).ResizeEnd(H(9));
        Assert.Equal(H(8), r.Start);
        Assert.Equal(H(9), r.End);
    }

    [Fact]
    public void ResizeStart_StopsAtMidnight()
    {
        var r = ZoomRange.Create(H(2), H(4)).ResizeStart(H(-1));
        Assert.Equal(TimeSpan.Zero, r.Start);
        Assert.Equal(H(6), r.End);
    }

    [Fact]
    public void ResizeEnd_StopsAtTwentyFour()
    {
        var r = ZoomRange.Create(H(20), H(2)).ResizeEnd(H(25));
        Assert.Equal(H(20), r.Start);
        Assert.Equal(H(24), r.End);
    }

    [Fact]
    public void ResizeStart_PastTheEnd_StopsFifteenMinutesBeforeIt()
    {
        var r = ZoomRange.Create(H(8), H(4)).ResizeStart(H(13));
        Assert.Equal(M(11 * 60 + 45), r.Start);
        Assert.Equal(H(12), r.End);
    }

    [Fact]
    public void ResizeEnd_PastTheStart_StopsFifteenMinutesAfterIt()
    {
        var r = ZoomRange.Create(H(8), H(4)).ResizeEnd(H(7));
        Assert.Equal(H(8), r.Start);
        Assert.Equal(M(8 * 60 + 15), r.End);
    }

    [Fact]
    public void ResizeEnd_AtTheEndOfTheDayFromMidnight_IsTheFullDay()
    {
        Assert.Equal(ZoomRange.FullDay, ZoomRange.Create(TimeSpan.Zero, H(6)).ResizeEnd(H(24)));
    }

    // ─── Centre on ──────────────────────────────────────────────────────────

    [Fact]
    public void CentreOn_PutsTheTimeInTheMiddleKeepingTheLength()
    {
        var r = ZoomRange.Create(H(8), H(2)).CentreOn(H(15));
        Assert.Equal(H(14), r.Start);
        Assert.Equal(H(16), r.End);
    }

    [Fact]
    public void CentreOn_NearMidnight_StaysInTheDay()
    {
        Assert.Equal(TimeSpan.Zero, ZoomRange.Create(H(8), H(2)).CentreOn(M(20)).Start);
        Assert.Equal(H(24), ZoomRange.Create(H(8), H(2)).CentreOn(M(23 * 60 + 40)).End);
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

    [Fact]
    public void ZoomAroundTheCentre_KeepsTheCentre()
    {
        // 08:00–12:00, centre 10:00 → 6 h and 2 h 40 min around 10:00.
        var r = ZoomRange.Create(H(8), H(4));
        Assert.Equal(H(10), r.Centre);
        Assert.Equal(H(7), r.ZoomOut(r.Centre).Start);
        Assert.Equal(H(10), r.ZoomIn(r.Centre).Centre);
        Assert.Equal(M(160), r.ZoomIn(r.Centre).Length);
    }

    // ─── Hit-testing the overview rectangle ─────────────────────────────────

    // A 100 px rectangle from x = 200 to x = 300: each edge's 8 px grab area is centred on it.
    [Theory]
    [InlineData(150, BrushHit.Outside)]
    [InlineData(195.9, BrushHit.Outside)]
    [InlineData(196, BrushHit.LeftEdge)]
    [InlineData(200, BrushHit.LeftEdge)]
    [InlineData(203.9, BrushHit.LeftEdge)]
    [InlineData(204, BrushHit.Body)]
    [InlineData(250, BrushHit.Body)]
    [InlineData(296, BrushHit.Body)]
    [InlineData(296.1, BrushHit.RightEdge)]
    [InlineData(300, BrushHit.RightEdge)]
    [InlineData(304, BrushHit.RightEdge)]
    [InlineData(304.1, BrushHit.Outside)]
    public void HitTest_AWideRectangle_HasGrabAreasCentredOnItsEdges(double x, BrushHit expected)
    {
        Assert.Equal(expected, ZoomRange.HitTest(rectLeftPx: 200, rectWidthPx: 100, x));
    }

    // 15 min on an 800 px overview is about 8.3 px wide, under 24 px: the grab
    // areas sit outside it and the whole rectangle is the body.
    [Theory]
    [InlineData(-8.1, BrushHit.Outside)]
    [InlineData(-8, BrushHit.LeftEdge)]
    [InlineData(-0.1, BrushHit.LeftEdge)]
    [InlineData(0, BrushHit.Body)]
    [InlineData(4, BrushHit.Body)]
    [InlineData(8.3, BrushHit.Body)]
    [InlineData(8.4, BrushHit.RightEdge)]
    [InlineData(16.3, BrushHit.RightEdge)]
    [InlineData(16.4, BrushHit.Outside)]
    public void HitTest_ANarrowRectangle_HasGrabAreasOutsideIt(double dx, BrushHit expected)
    {
        var width = 800.0 * 15 / (24 * 60);
        Assert.Equal(expected, ZoomRange.HitTest(rectLeftPx: 400, rectWidthPx: width, 400 + dx));
    }

    [Fact]
    public void HitTest_At24Px_IsNoLongerNarrow()
    {
        Assert.Equal(BrushHit.LeftEdge, ZoomRange.HitTest(rectLeftPx: 100, rectWidthPx: 24, 102));
        Assert.Equal(BrushHit.Body, ZoomRange.HitTest(rectLeftPx: 100, rectWidthPx: 23.9, 102));
    }

    // ─── Brush gestures on the overview ─────────────────────────────────────

    // An 800 px overview: 1 px = 1.8 min, and 08:00–12:00 spans x ≈ 266.7–400.
    private const double OverviewPx = 800;
    private static double X(double hours) => hours / 24 * OverviewPx;
    private static readonly ZoomRange Morning = ZoomRange.Create(H(8), H(4));

    // Pixels don't divide minutes exactly.
    private static void AssertNear(TimeSpan expected, TimeSpan actual) =>
        Assert.Equal(expected.TotalMinutes, actual.TotalMinutes, tolerance: 0.01);

    [Fact]
    public void Brush_DraggingTheBody_Pans()
    {
        var r = Morning.Brush(OverviewPx, fromPx: 300, toPx: 350);
        Assert.Equal(M(9 * 60 + 30), r.Start);
        Assert.Equal(M(13 * 60 + 30), r.End);
    }

    [Fact]
    public void Brush_DraggingTheBodyPastTheEndOfTheDay_StopsAtTwentyFour()
    {
        var r = Morning.Brush(OverviewPx, fromPx: 300, toPx: 900);
        Assert.Equal(H(20), r.Start);
        Assert.Equal(H(24), r.End);
    }

    [Fact]
    public void Brush_DraggingTheLeftEdge_ResizesTheStart()
    {
        var r = Morning.Brush(OverviewPx, fromPx: X(8) + 2, toPx: X(7) + 2);
        AssertNear(H(7), r.Start);
        Assert.Equal(H(12), r.End);
    }

    [Fact]
    public void Brush_DraggingTheRightEdgePastTheLeft_StopsAtFifteenMinutes()
    {
        var r = Morning.Brush(OverviewPx, fromPx: X(12), toPx: 100);
        Assert.Equal(H(8), r.Start);
        Assert.Equal(M(15), r.Length);
    }

    [Fact]
    public void Brush_ClickingOutside_CentresOnThatTime()
    {
        var r = Morning.Brush(OverviewPx, fromPx: X(18), toPx: X(18));
        AssertNear(H(16), r.Start);
        AssertNear(H(20), r.End);
    }

    [Fact]
    public void Brush_PressingOutsideAndDragging_CentresThenPans()
    {
        var r = Morning.Brush(OverviewPx, fromPx: X(18), toPx: X(19.5));
        AssertNear(M(17 * 60 + 30), r.Start);
        Assert.Equal(H(4), r.Length);
    }

    [Fact]
    public void Brush_ANarrowRectangle_ResizesFromTheGrabAreaOutsideIt()
    {
        // 10:00–10:15 is ~8 px wide: a press 3 px left of it grabs the left edge, it doesn't centre.
        var r = ZoomRange.Create(H(10), M(15)).Brush(OverviewPx, fromPx: X(10) - 3, toPx: X(9) - 3);
        AssertNear(H(9), r.Start);
        Assert.Equal(M(10 * 60 + 15), r.End);
    }

    [Fact]
    public void Brush_ANarrowRectangle_PansByItsBody()
    {
        var r = ZoomRange.Create(H(10), M(15)).Brush(OverviewPx, fromPx: X(10) + 4, toPx: X(11) + 4);
        AssertNear(H(11), r.Start);
        Assert.Equal(M(15), r.Length);
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

    // The main bar draws the whole day, so a drag on the overview can pan
    // its labels into view before .NET re-renders.
    [Fact]
    public void LabelTimes_AreStepMultiplesAcrossTheWholeDay()
    {
        var times = ZoomRange.Create(M(8 * 60 + 10), M(80)).LabelTimes().ToList();
        Assert.Equal(96, times.Count);
        Assert.Equal(TimeSpan.Zero, times[0]);
        Assert.Equal(M(15), times[1]);
        Assert.Equal(M(23 * 60 + 45), times[^1]);
    }

    [Fact]
    public void LabelTimes_FullDay_AreEveryTwoHoursFromMidnight()
    {
        var times = ZoomRange.FullDay.LabelTimes().ToList();
        Assert.Equal(12, times.Count);
        Assert.Equal(TimeSpan.Zero, times[0]);
        Assert.Equal(H(22), times[^1]);
    }

    // ─── Following the main bar's cursor (#126) ─────────────────────────────

    [Fact]
    public void Follow_ACursorInside_KeepsTheRange()
    {
        var r = ZoomRange.Create(H(8), H(2));
        Assert.Same(r, r.Follow(H(9)));
        Assert.Same(r, r.Follow(H(8)));
        Assert.Same(r, r.Follow(H(10)));
    }

    [Fact]
    public void Follow_ACursorPastTheEnd_PansJustEnough()
    {
        var r = ZoomRange.Create(H(8), H(2)).Follow(H(10.5));
        Assert.Equal(H(8.5), r.Start);
        Assert.Equal(H(10.5), r.End);
    }

    [Fact]
    public void Follow_ACursorBeforeTheStart_PansJustEnough()
    {
        var r = ZoomRange.Create(H(8), H(2)).Follow(H(7.25));
        Assert.Equal(H(7.25), r.Start);
        Assert.Equal(H(2), r.Length);
    }

    [Fact]
    public void Follow_AtTheDaysEdges_StaysInsideTheDay()
    {
        var early = ZoomRange.Create(H(1), H(2)).Follow(TimeSpan.Zero);
        Assert.Equal(TimeSpan.Zero, early.Start);
        Assert.Equal(H(2), early.End);

        var late = ZoomRange.Create(H(20), H(2)).Follow(H(24));
        Assert.Equal(H(22), late.Start);
        Assert.Equal(H(24), late.End);
    }

    // ─── Overview keys (#126) ───────────────────────────────────────────────

    private static ZoomRange Key(ZoomRange r, string key, bool shift = false) =>
        r.AfterKey(key, shift) ?? throw new Xunit.Sdk.XunitException($"{key} not handled");

    [Fact]
    public void AfterKey_ArrowsPanFifteenMinutes()
    {
        var r = ZoomRange.Create(H(8), H(2));
        Assert.Equal(M(8 * 60 + 15), Key(r, "ArrowRight").Start);
        Assert.Equal(M(8 * 60 + 15), Key(r, "ArrowUp").Start);
        Assert.Equal(M(7 * 60 + 45), Key(r, "ArrowLeft").Start);
        Assert.Equal(M(7 * 60 + 45), Key(r, "ArrowDown").Start);
        Assert.Equal(H(2), Key(r, "ArrowRight").Length);
    }

    [Fact]
    public void AfterKey_ShiftArrowsAndPageKeysPanAnHour()
    {
        var r = ZoomRange.Create(H(8), H(2));
        Assert.Equal(H(9), Key(r, "ArrowRight", shift: true).Start);
        Assert.Equal(H(7), Key(r, "ArrowLeft", shift: true).Start);
        Assert.Equal(H(9), Key(r, "ArrowUp", shift: true).Start);
        Assert.Equal(H(7), Key(r, "ArrowDown", shift: true).Start);
        Assert.Equal(H(9), Key(r, "PageUp").Start);
        Assert.Equal(H(7), Key(r, "PageDown").Start);
    }

    [Fact]
    public void AfterKey_PansStopAtTheDaysEdges()
    {
        Assert.Equal(TimeSpan.Zero, Key(ZoomRange.Create(M(10), H(2)), "ArrowLeft").Start);
        Assert.Equal(H(24), Key(ZoomRange.Create(H(21.5), H(2)), "PageUp").End);
    }

    [Fact]
    public void AfterKey_HomeAndEndMoveToTheDaysEdges()
    {
        var r = ZoomRange.Create(H(8), H(2));
        Assert.Equal(ZoomRange.Create(TimeSpan.Zero, H(2)), Key(r, "Home"));
        Assert.Equal(ZoomRange.Create(H(22), H(2)), Key(r, "End"));
    }

    [Fact]
    public void AfterKey_PlusAndMinusZoomAroundTheCentre()
    {
        Assert.Equal("04:00–20:00", Key(ZoomRange.FullDay, "+").RangeText);
        Assert.Equal("04:00–20:00", Key(ZoomRange.FullDay, "=").RangeText);

        var r = ZoomRange.Create(H(10), H(2));
        Assert.Equal("09:30–12:30", Key(r, "-").RangeText);
        Assert.Equal("10:00–12:00", Key(Key(r, "-"), "+").RangeText);
    }

    [Fact]
    public void AfterKey_ZoomStaysWithinItsLimits()
    {
        Assert.Equal(M(15), Key(ZoomRange.Create(H(10), M(15)), "+").Length);
        Assert.Equal(ZoomRange.FullDay, Key(ZoomRange.Create(H(2), H(20)), "-"));
    }

    [Fact]
    public void AfterKey_ZeroShowsTheFullDay()
    {
        Assert.Equal(ZoomRange.FullDay, Key(ZoomRange.Create(H(8), H(2)), "0"));
    }

    [Fact]
    public void AfterKey_OtherKeys_AreNotTheOverviews()
    {
        Assert.Null(ZoomRange.FullDay.AfterKey("Enter", shift: false));
        Assert.Null(ZoomRange.FullDay.AfterKey("a", shift: false));
    }

    [Fact]
    public void RangeText_IsTheOverviewsValueText()
    {
        Assert.Equal("08:00–13:30", ZoomRange.Create(H(8), H(5.5)).RangeText);
    }
}
