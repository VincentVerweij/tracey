using Tracey.App.Services;
using Tracey.App.TimelineModel;

namespace Tracey.Tests;

/// <summary>
/// The Timeline bar's rules for one viewed day (#124, decisions in #108, #113, #120).
/// The viewed day is 2026-10-15 in UTC, so offsets read as UTC clock times.
/// </summary>
public class TimelineDayTests
{
    private static readonly DateOnly Date = new(2026, 10, 15);
    private static readonly DateTimeOffset DayStart = new(2026, 10, 15, 0, 0, 0, TimeSpan.Zero);

    private static DateTimeOffset At(string hhmm) =>
        DayStart + TimeSpan.Parse(hhmm, System.Globalization.CultureInfo.InvariantCulture);

    private static string Iso(string hhmm) => At(hhmm).ToString("o");

    private static PresenceSpan Present(string from, string to, bool open = false) =>
        new(Iso(from), Iso(to), "Present", null, open);

    private static PresenceSpan Away(string from, string to, string cause, bool open = false) =>
        new(Iso(from), Iso(to), "Away", cause, open);

    private static ScreenshotItem Shot(string id, string hhmm) =>
        new(id, $"C:/shots/{id}.png", Iso(hhmm), "title", "proc.exe", "interval");

    private static TimelineDay Day(PresenceSpan[]? spans = null, ScreenshotItem[]? shots = null) =>
        TimelineDay.Create(Date, TimeZoneInfo.Utc, spans ?? [], shots ?? []);

    // ─── Presence at t ──────────────────────────────────────────────────────

    [Fact]
    public void PresenceAt_InsidePresentSpan_IsPresent()
    {
        var day = Day([Present("08:00", "09:00")]);
        Assert.Equal(Presence.Present, day.PresenceAt(At("08:30")));
    }

    [Fact]
    public void PresenceAt_TellsAwayCausesApart()
    {
        var day = Day([Away("08:00", "09:00", "Locked"), Away("09:00", "10:00", "Idle")]);
        Assert.Equal(Presence.AwayLocked, day.PresenceAt(At("08:30")));
        Assert.Equal(Presence.AwayIdle, day.PresenceAt(At("09:30")));
    }

    [Fact]
    public void PresenceAt_NoSpan_IsUntracked()
    {
        var day = Day([Present("08:00", "09:00")]);
        Assert.Equal(Presence.Untracked, day.PresenceAt(At("07:59")));
        Assert.Equal(Presence.Untracked, day.PresenceAt(At("12:00")));
    }

    [Fact]
    public void PresenceAt_EmptyDay_IsUntracked()
    {
        Assert.Equal(Presence.Untracked, Day().PresenceAt(At("12:00")));
    }

    [Fact]
    public void PresenceAt_OnSpanBoundary_BelongsToTheLaterSpan()
    {
        var day = Day([Present("08:00", "09:00"), Away("09:00", "10:00", "Locked")]);
        Assert.Equal(Presence.Present, day.PresenceAt(At("08:00")));
        Assert.Equal(Presence.AwayLocked, day.PresenceAt(At("09:00")));
        Assert.Equal(Presence.Untracked, day.PresenceAt(At("10:00")));
    }

    [Fact]
    public void PresenceAt_SpanStraddlingMidnight_CoversTheStartOfTheDay()
    {
        var straddling = new PresenceSpan("2026-10-14T23:00:00+00:00", Iso("01:00"), "Present", null, false);
        var day = Day([straddling]);
        Assert.Equal(Presence.Present, day.PresenceAt(At("00:00")));
        Assert.Equal(Presence.Untracked, day.PresenceAt(At("01:00")));
    }

    // ─── Hover pick ─────────────────────────────────────────────────────────

    [Fact]
    public void Pick_ReturnsTheLatestCaptureAtOrBeforeT()
    {
        var day = Day([Present("08:00", "12:00")], [Shot("a", "08:10"), Shot("b", "08:20")]);
        var pick = Assert.IsType<HoverPick.Shot>(day.Pick(At("08:19")));
        Assert.Equal("a", pick.Screenshot.Id);
    }

    [Fact]
    public void Pick_ExactlyOnACapture_ReturnsThatCapture()
    {
        var day = Day([Present("08:00", "12:00")], [Shot("a", "08:10"), Shot("b", "08:20")]);
        var pick = Assert.IsType<HoverPick.Shot>(day.Pick(At("08:20")));
        Assert.Equal("b", pick.Screenshot.Id);
    }

    [Fact]
    public void Pick_HasNoMaximumLookBack()
    {
        var day = Day([Present("08:00", "12:00")], [Shot("a", "08:10")]);
        var pick = Assert.IsType<HoverPick.Shot>(day.Pick(At("11:59")));
        Assert.Equal("a", pick.Screenshot.Id);
    }

    [Fact]
    public void Pick_BeforeTheFirstCapture_ReturnsNothing()
    {
        var day = Day([Present("08:00", "12:00")], [Shot("a", "08:10")]);
        Assert.IsType<HoverPick.Nothing>(day.Pick(At("08:05")));
    }

    [Fact]
    public void Pick_InLockedAway_IsAGapEvenWithCapturesThere()
    {
        var day = Day([Present("08:00", "09:00"), Away("09:00", "10:00", "Locked")],
            [Shot("a", "08:10"), Shot("b", "09:30")]);
        var gap = Assert.IsType<HoverPick.Gap>(day.Pick(At("09:45")));
        Assert.Equal(Presence.AwayLocked, gap.Presence);
    }

    [Fact]
    public void Pick_InUntracked_IsAGapEvenWithCapturesThere()
    {
        // A day before presence recording: screenshots, no spans (#113).
        var day = Day(shots: [Shot("a", "08:10")]);
        var gap = Assert.IsType<HoverPick.Gap>(day.Pick(At("08:30")));
        Assert.Equal(Presence.Untracked, gap.Presence);
    }

    [Fact]
    public void Pick_InIdleAway_ReturnsTheLatestCapture()
    {
        var day = Day([Present("08:00", "09:00"), Away("09:00", "10:00", "Idle")],
            [Shot("a", "08:10"), Shot("b", "09:30")]);
        var pick = Assert.IsType<HoverPick.Shot>(day.Pick(At("09:45")));
        Assert.Equal("b", pick.Screenshot.Id);
    }

    [Fact]
    public void Pick_OnTheBoundaryIntoLockedAway_IsAGap()
    {
        var day = Day([Present("08:00", "09:00"), Away("09:00", "10:00", "Locked")], [Shot("a", "08:10")]);
        Assert.IsType<HoverPick.Gap>(day.Pick(At("09:00")));
        Assert.IsType<HoverPick.Shot>(day.Pick(At("08:59")));
    }

    [Fact]
    public void Pick_EmptyDay_IsAnUntrackedGap()
    {
        var gap = Assert.IsType<HoverPick.Gap>(Day().Pick(At("12:00")));
        Assert.Equal(Presence.Untracked, gap.Presence);
    }

    // ─── Click outcome ──────────────────────────────────────────────────────

    [Fact]
    public void PinAfterClick_PinsTheHoverPick()
    {
        var day = Day([Present("08:00", "12:00")], [Shot("a", "08:10"), Shot("b", "08:20")]);
        Assert.Equal("a", day.PinAfterClick(At("08:15"), pinned: null)?.Id);
    }

    [Fact]
    public void PinAfterClick_OnAnotherCapture_MovesThePin()
    {
        var a = Shot("a", "08:10");
        var day = Day([Present("08:00", "12:00")], [a, Shot("b", "08:20")]);
        Assert.Equal("b", day.PinAfterClick(At("08:20"), pinned: a)?.Id);
    }

    [Fact]
    public void PinAfterClick_WherePickIsThePinnedCapture_Unpins()
    {
        var a = Shot("a", "08:10");
        var day = Day([Present("08:00", "12:00")], [a, Shot("b", "08:20")]);
        Assert.Null(day.PinAfterClick(At("08:19"), pinned: a));
    }

    [Fact]
    public void PinAfterClick_InAGap_ClearsThePin()
    {
        var a = Shot("a", "08:10");
        var day = Day([Present("08:00", "09:00"), Away("09:00", "10:00", "Locked")], [a]);
        Assert.Null(day.PinAfterClick(At("09:30"), pinned: a));
    }

    [Fact]
    public void PinAfterClick_BeforeTheFirstCapture_KeepsThePin()
    {
        var b = Shot("b", "08:20");
        var day = Day([Present("08:00", "12:00")], [b]);
        Assert.Equal("b", day.PinAfterClick(At("08:05"), pinned: b)?.Id);
    }

    // ─── Visible spans ──────────────────────────────────────────────────────

    [Fact]
    public void VisibleSpans_AreClippedToTheRange()
    {
        var day = Day([Present("07:00", "09:00"), Away("09:00", "10:00", "Locked"), Present("10:00", "13:00")]);
        var visible = day.VisibleSpans(ZoomRange.Create(TimeSpan.FromHours(8), TimeSpan.FromHours(3)));
        Assert.Equal(
            [
                new VisibleSpan(TimeSpan.FromHours(8), TimeSpan.FromHours(9), Presence.Present),
                new VisibleSpan(TimeSpan.FromHours(9), TimeSpan.FromHours(10), Presence.AwayLocked),
                new VisibleSpan(TimeSpan.FromHours(10), TimeSpan.FromHours(11), Presence.Present),
            ],
            visible);
    }

    [Fact]
    public void VisibleSpans_LeaveOutSpansOutsideTheRange()
    {
        var day = Day([Present("07:00", "08:00"), Present("11:00", "12:00")]);
        Assert.Empty(day.VisibleSpans(ZoomRange.Create(TimeSpan.FromHours(8), TimeSpan.FromHours(3))));
    }

    [Fact]
    public void VisibleSpans_ClipASpanStraddlingMidnightToTheDay()
    {
        var straddling = new PresenceSpan("2026-10-14T23:00:00+00:00", Iso("01:00"), "Present", null, false);
        var late = new PresenceSpan(Iso("23:00"), "2026-10-16T02:00:00+00:00", "Away", "Idle", false);
        var visible = Day([straddling, late]).VisibleSpans(ZoomRange.FullDay);
        Assert.Equal(
            [
                new VisibleSpan(TimeSpan.Zero, TimeSpan.FromHours(1), Presence.Present),
                new VisibleSpan(TimeSpan.FromHours(23), TimeSpan.FromHours(24), Presence.AwayIdle),
            ],
            visible);
    }

    [Fact]
    public void OffsetOf_And_At_MapBetweenInstantsAndTheDay()
    {
        var day = Day();
        Assert.Equal(TimeSpan.FromHours(9.5), day.OffsetOf(At("09:30")));
        Assert.Equal(At("09:30"), day.At(TimeSpan.FromHours(9.5)));
    }

    [Fact]
    public void OffsetOf_ReadsInstantsInAnyUtcOffset()
    {
        Assert.Equal(TimeSpan.FromHours(9.5), Day().OffsetOf(DateTimeOffset.Parse("2026-10-15T11:30:00+02:00")));
    }

    [Fact]
    public void IsEmpty_OnlyWithNeitherSpansNorCaptures()
    {
        Assert.True(Day().IsEmpty);
        Assert.False(Day([Present("08:00", "09:00")]).IsEmpty);
        Assert.False(Day(shots: [Shot("a", "08:10")]).IsEmpty);
    }

    // ─── Open span growth ───────────────────────────────────────────────────

    [Fact]
    public void GrowOpenSpan_ExtendsTheOpenSpanToNow()
    {
        var day = Day([Present("08:00", "09:00"), Away("09:00", "10:00", "Idle", open: true)])
            .GrowOpenSpan(At("10:20"));
        Assert.Equal(At("10:20"), day.Spans[1].End);
        Assert.Equal(Presence.AwayIdle, day.PresenceAt(At("10:10")));
        Assert.Equal(Presence.Untracked, day.PresenceAt(At("10:20")));
    }

    [Fact]
    public void GrowOpenSpan_LeavesClosedSpansAlone()
    {
        var day = Day([Present("08:00", "09:00")]).GrowOpenSpan(At("10:20"));
        Assert.Equal(At("09:00"), day.Spans[0].End);
    }

    [Fact]
    public void GrowOpenSpan_IsClippedToTheEndOfTheDay()
    {
        var day = Day([Present("23:00", "23:59:40", open: true)])
            .GrowOpenSpan(DayStart.AddDays(1).AddSeconds(25));
        Assert.Equal(DayStart.AddDays(1), day.Spans[0].End);
    }

    [Fact]
    public void GrowOpenSpan_NeverShrinksTheSpan()
    {
        var day = Day([Present("08:00", "09:00", open: true)]).GrowOpenSpan(At("08:30"));
        Assert.Equal(At("09:00"), day.Spans[0].End);
    }

    [Fact]
    public void GrowOpenSpan_KeepsCaptures()
    {
        var day = Day([Present("08:00", "09:00", open: true)], [Shot("a", "08:10")])
            .GrowOpenSpan(At("09:30"));
        Assert.Equal("a", Assert.IsType<HoverPick.Shot>(day.Pick(At("09:20"))).Screenshot.Id);
    }

    // ─── UI words ───────────────────────────────────────────────────────────

    [Theory]
    [InlineData(Presence.Present, "Active")]
    [InlineData(Presence.AwayLocked, "Away")]
    [InlineData(Presence.AwayIdle, "Away")]
    [InlineData(Presence.Untracked, "Not tracked")]
    public void Word_IsTheHoverLabelState(Presence presence, string expected)
    {
        Assert.Equal(expected, presence.Word());
    }

    [Fact]
    public void GapCard_NamesTheCaptureStoppedState()
    {
        Assert.Equal("10:31 · Away (locked) — no screenshots", PresenceText.GapCard(Presence.AwayLocked, "10:31"));
        Assert.Equal("10:31 · Not tracked", PresenceText.GapCard(Presence.Untracked, "10:31"));
    }

    // ─── Daylight saving: offsets are wall-clock times ─────────────────────

    // CET/CEST: +01:00, +02:00 from the last Sunday of March 02:00 to the last
    // Sunday of October 03:00. In 2026 those are 29 March and 25 October.
    private static readonly TimeZoneInfo Cet = TimeZoneInfo.CreateCustomTimeZone(
        "Test CET", TimeSpan.FromHours(1), "Test CET", "Test CET", "Test CEST",
        [
            TimeZoneInfo.AdjustmentRule.CreateAdjustmentRule(
                DateTime.MinValue.Date, DateTime.MaxValue.Date, TimeSpan.FromHours(1),
                TimeZoneInfo.TransitionTime.CreateFloatingDateRule(new DateTime(1, 1, 1, 2, 0, 0), 3, 5, DayOfWeek.Sunday),
                TimeZoneInfo.TransitionTime.CreateFloatingDateRule(new DateTime(1, 1, 1, 3, 0, 0), 10, 5, DayOfWeek.Sunday)),
        ]);

    private static TimelineDay CetDay(int month, int day, PresenceSpan[]? spans = null) =>
        TimelineDay.Create(new DateOnly(2026, month, day), Cet, spans ?? [], []);

    [Fact]
    public void FallBackDay_RunsFromLocalMidnightToLocalMidnight()
    {
        var day = CetDay(10, 25);
        Assert.Equal(DateTimeOffset.Parse("2026-10-25T00:00:00+02:00"), day.DayStart);
        Assert.Equal(DateTimeOffset.Parse("2026-10-26T00:00:00+01:00"), day.DayEnd);
    }

    [Fact]
    public void FallBackDay_OffsetsAfterTheChangeAreWallClockTimes()
    {
        var day = CetDay(10, 25);
        var noon = DateTimeOffset.Parse("2026-10-25T12:00:00+01:00");
        Assert.Equal(TimeSpan.FromHours(12), day.OffsetOf(noon));
        Assert.Equal(noon, day.At(TimeSpan.FromHours(12)));
        Assert.Equal(TimeSpan.FromHours(24), day.OffsetOf(day.DayEnd));
    }

    [Fact]
    public void FallBackDay_TheRepeatedHourMapsToItsFirstPass()
    {
        Assert.Equal(DateTimeOffset.Parse("2026-10-25T02:30:00+02:00"), CetDay(10, 25).At(TimeSpan.FromHours(2.5)));
    }

    [Fact]
    public void FallBackDay_OpenSpanGrowthIsClippedToTheRealMidnight()
    {
        var open = new PresenceSpan("2026-10-25T23:00:00+01:00", "2026-10-25T23:59:40+01:00", "Present", null, true);
        var day = CetDay(10, 25, [open]).GrowOpenSpan(DateTimeOffset.Parse("2026-10-26T00:00:25+01:00"));
        Assert.Equal(DateTimeOffset.Parse("2026-10-26T00:00:00+01:00"), day.Spans[0].End);
        Assert.Equal(
            [new VisibleSpan(TimeSpan.FromHours(23), TimeSpan.FromHours(24), Presence.Present)],
            day.VisibleSpans(ZoomRange.FullDay));
    }

    [Fact]
    public void SpringForwardDay_OffsetsAfterTheChangeAreWallClockTimes()
    {
        var day = CetDay(3, 29);
        Assert.Equal(DateTimeOffset.Parse("2026-03-30T00:00:00+02:00"), day.DayEnd);
        Assert.Equal(TimeSpan.FromHours(3), day.OffsetOf(DateTimeOffset.Parse("2026-03-29T03:00:00+02:00")));
    }

    [Fact]
    public void SpringForwardDay_TheSkippedHourMapsToTheEndOfTheGap()
    {
        Assert.Equal(DateTimeOffset.Parse("2026-03-29T03:00:00+02:00"), CetDay(3, 29).At(TimeSpan.FromHours(2.5)));
    }
}
