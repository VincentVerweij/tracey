using Tracey.App.Services;
using Tracey.App.TimelineModel;

namespace Tracey.Tests;

/// <summary>
/// The main bar's keyboard cursor (#126, decisions in #121).
/// The viewed day is 2026-10-15 in UTC, so offsets read as UTC clock times.
/// </summary>
public class BarCursorTests
{
    private static readonly DateOnly Date = new(2026, 10, 15);
    private static readonly DateTimeOffset DayStart = new(2026, 10, 15, 0, 0, 0, TimeSpan.Zero);

    private static TimeSpan T(string hhmm) => TimeSpan.Parse(hhmm, System.Globalization.CultureInfo.InvariantCulture);

    private static string Iso(string hhmm) => (DayStart + T(hhmm)).ToString("o");

    private static PresenceSpan Present(string from, string to) =>
        new(Iso(from), Iso(to), "Present", null, false);

    private static PresenceSpan Away(string from, string to, string cause) =>
        new(Iso(from), Iso(to), "Away", cause, false);

    private static ScreenshotItem Shot(string id, string hhmm, string title = "title") =>
        new(id, $"C:/shots/{id}.png", Iso(hhmm), title, "proc.exe", "interval");

    private static TimelineDay Day(PresenceSpan[]? spans = null, ScreenshotItem[]? shots = null) =>
        TimelineDay.Create(Date, TimeZoneInfo.Utc, spans ?? [], shots ?? []);

    private static BarCursor At(string hhmm) => new(T(hhmm));

    private static readonly TimelineDay ThreeShots =
        Day([Present("09:00", "12:00")], [Shot("a", "09:10"), Shot("b", "10:31"), Shot("c", "11:45")]);

    // ─── Previous/next capture ──────────────────────────────────────────────

    [Fact]
    public void NextCapture_LandsOnTheNextCapturesTime()
    {
        Assert.Equal(T("10:31"), At("09:10").NextCapture(ThreeShots).At);
        Assert.Equal(T("09:10"), At("08:00").NextCapture(ThreeShots).At);
    }

    [Fact]
    public void PreviousCapture_LandsOnThePreviousCapturesTime()
    {
        Assert.Equal(T("09:10"), At("10:31").PreviousCapture(ThreeShots).At);
        Assert.Equal(T("10:31"), At("11:00").PreviousCapture(ThreeShots).At);
    }

    [Fact]
    public void NextCapture_AfterTheLast_StaysPut()
    {
        Assert.Equal(T("11:45"), At("11:45").NextCapture(ThreeShots).At);
        Assert.Equal(T("23:00"), At("23:00").NextCapture(ThreeShots).At);
    }

    [Fact]
    public void PreviousCapture_BeforeTheFirst_StaysPut()
    {
        Assert.Equal(T("09:10"), At("09:10").PreviousCapture(ThreeShots).At);
        Assert.Equal(T("01:00"), At("01:00").PreviousCapture(ThreeShots).At);
    }

    [Fact]
    public void CaptureSteps_OnAnEmptyDay_StayPut()
    {
        Assert.Equal(T("12:00"), At("12:00").NextCapture(Day()).At);
        Assert.Equal(T("12:00"), At("12:00").PreviousCapture(Day()).At);
    }

    // ─── Previous/next span boundary ────────────────────────────────────────

    private static readonly TimelineDay Spans =
        Day([Present("09:00", "10:15"), Away("10:15", "10:40", "Locked"), Present("11:00", "12:00")]);

    [Fact]
    public void NextBoundary_StepsThroughStartsAndEnds()
    {
        Assert.Equal(T("09:00"), At("08:00").NextBoundary(Spans).At);
        Assert.Equal(T("10:15"), At("09:00").NextBoundary(Spans).At);
        Assert.Equal(T("10:40"), At("10:15").NextBoundary(Spans).At);
        Assert.Equal(T("11:00"), At("10:40").NextBoundary(Spans).At);
        Assert.Equal(T("12:00"), At("11:30").NextBoundary(Spans).At);
    }

    [Fact]
    public void PreviousBoundary_StepsBack()
    {
        Assert.Equal(T("11:00"), At("11:30").PreviousBoundary(Spans).At);
        Assert.Equal(T("10:15"), At("10:40").PreviousBoundary(Spans).At);
        Assert.Equal(T("09:00"), At("09:30").PreviousBoundary(Spans).At);
    }

    [Fact]
    public void Boundaries_WithNothingInThatDirection_StayPut()
    {
        Assert.Equal(T("12:00"), At("12:00").NextBoundary(Spans).At);
        Assert.Equal(T("09:00"), At("09:00").PreviousBoundary(Spans).At);
        Assert.Equal(T("12:00"), At("12:00").NextBoundary(Day()).At);
    }

    [Fact]
    public void Boundaries_OfASpanFromYesterday_StartAtItsEnd()
    {
        var straddling = new PresenceSpan("2026-10-14T23:00:00+00:00", Iso("01:00"), "Present", null, false);
        var day = Day([straddling]);
        Assert.Equal(T("01:00"), At("00:00").NextBoundary(day).At);
        Assert.Equal(T("00:30"), At("00:30").PreviousBoundary(day).At);
    }

    [Fact]
    public void BoundaryJump_ShowsTheExtent_ACaptureStepDoesNot()
    {
        Assert.True(At("08:00").NextBoundary(Spans).ShowsExtent);
        Assert.False(At("08:00").NextBoundary(Spans).NextCapture(ThreeShots).ShowsExtent);
    }

    [Fact]
    public void CaptureStep_WithNothingThatWay_DropsABoundaryJumpsExtent()
    {
        var jumped = new BarCursor(T("12:00"), ShowsExtent: true);
        var stayed = jumped.NextCapture(ThreeShots);
        Assert.Equal(T("12:00"), stayed.At);
        Assert.False(stayed.ShowsExtent);
    }

    // ─── Keys ───────────────────────────────────────────────────────────────

    [Fact]
    public void AfterKey_ArrowsStepCaptures_ShiftArrowsJumpBoundaries()
    {
        var day = Day(
            [Present("09:00", "10:15"), Away("10:15", "10:40", "Locked")],
            [Shot("a", "09:10"), Shot("b", "09:50")]);
        Assert.Equal(T("09:50"), At("09:10").AfterKey(day, "ArrowRight", shift: false)!.At);
        Assert.Equal(T("09:10"), At("09:50").AfterKey(day, "ArrowLeft", shift: false)!.At);
        Assert.Equal(T("10:15"), At("09:10").AfterKey(day, "ArrowRight", shift: true)!.At);
        Assert.Equal(T("09:00"), At("09:10").AfterKey(day, "ArrowLeft", shift: true)!.At);
    }

    [Fact]
    public void AfterKey_OtherKeys_AreNotTheCursors()
    {
        Assert.Null(At("09:10").AfterKey(ThreeShots, "ArrowUp", shift: false));
        Assert.Null(At("09:10").AfterKey(ThreeShots, "Enter", shift: false));
    }

    // ─── Start position on focus ────────────────────────────────────────────

    [Fact]
    public void OnFocus_StartsAtThePinnedCapture()
    {
        var pinned = ThreeShots.Captures[2].Screenshot;
        var cursor = BarCursor.OnFocus(ThreeShots, ZoomRange.Create(T("09:00"), T("01:00")), pinned);
        Assert.Equal(T("11:45"), cursor.At);
    }

    [Fact]
    public void OnFocus_WithoutAPin_StartsAtTheFirstCaptureInTheRange()
    {
        var cursor = BarCursor.OnFocus(ThreeShots, ZoomRange.Create(T("10:00"), T("02:00")), null);
        Assert.Equal(T("10:31"), cursor.At);
    }

    [Fact]
    public void OnFocus_WithNoCaptureInTheRange_StartsAtTheRangesStart()
    {
        var cursor = BarCursor.OnFocus(ThreeShots, ZoomRange.Create(T("14:00"), T("01:00")), null);
        Assert.Equal(T("14:00"), cursor.At);
        Assert.False(cursor.ShowsExtent);
    }

    // ─── aria-valuenow and aria-valuetext ───────────────────────────────────

    [Fact]
    public void Minute_IsTheCursorsMinuteOfTheDay()
    {
        Assert.Equal(631, At("10:31").Minute);
        Assert.Equal(1440, At("1.00:00").Minute);
    }

    [Fact]
    public void ValueText_Present_NamesTheWindowTitle()
    {
        var day = Day([Present("10:00", "11:00")], [Shot("a", "10:31", "Inbox — Mail")]);
        Assert.Equal("10:31 · Present · Inbox — Mail", At("10:31").ValueText(day));
    }

    [Fact]
    public void ValueText_PresentBeforeTheFirstCapture_HasNoTitle()
    {
        var day = Day([Present("10:00", "11:00")], [Shot("a", "10:45")]);
        Assert.Equal("10:31 · Present", At("10:31").ValueText(day));
    }

    [Fact]
    public void ValueText_IdleAway_NamesTheCauseAndTheWindowTitle()
    {
        var day = Day([Away("10:00", "11:00", "Idle")], [Shot("a", "09:58", "Editor")]);
        Assert.Equal("10:31 · Away (idle) · Editor", At("10:31").ValueText(day));
    }

    [Fact]
    public void ValueText_LockedAway_HasNoScreenshots()
    {
        var day = Day([Away("10:00", "11:00", "Locked")], [Shot("a", "09:58")]);
        Assert.Equal("10:31 · Away (locked), no screenshots", At("10:31").ValueText(day));
    }

    [Fact]
    public void ValueText_Untracked_IsNotTracked()
    {
        Assert.Equal("10:31 · Not tracked", At("10:31").ValueText(Day()));
    }

    [Fact]
    public void ValueText_AfterABoundaryJump_AddsTheSpansEnd()
    {
        var cursor = At("09:30").NextBoundary(Spans);
        Assert.Equal("10:15 · Away (locked), until 10:40", cursor.ValueText(Spans));
    }

    [Fact]
    public void ValueText_AfterABoundaryJumpIntoPresent_AddsTheEndThenTheTitle()
    {
        var day = Day([Present("11:00", "12:00")], [Shot("a", "11:00", "Editor")]);
        var cursor = At("10:00").NextBoundary(day);
        Assert.Equal("11:00 · Present, until 12:00 · Editor", cursor.ValueText(day));
    }

    [Fact]
    public void ValueText_AfterABoundaryJumpIntoUntracked_RunsToTheNextSpan()
    {
        var cursor = At("10:20").NextBoundary(Spans);
        Assert.Equal("10:40 · Not tracked, until 11:00", cursor.ValueText(Spans));
    }

    [Fact]
    public void ValueText_AfterTheLastSpan_IsNotTrackedUntilTheEndOfTheDay()
    {
        var cursor = At("11:30").NextBoundary(Spans);
        Assert.Equal("12:00 · Not tracked, until 24:00", cursor.ValueText(Spans));
    }
}
