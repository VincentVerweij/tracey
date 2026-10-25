using Tracey.App.Services;

namespace Tracey.App.TimelineModel;

/// What the Timeline bar shows at an instant (GLOSSARY "Presence", ADR-0005).
public enum Presence
{
    Present,
    AwayLocked,
    AwayIdle,
    Untracked,
}

/// The UI words for presence (GLOSSARY "Presence span").
public static class PresenceText
{
    /// The hover label's state: "Active", "Away" or "Not tracked".
    public static string Word(this Presence presence) => presence switch
    {
        Presence.Present => "Active",
        Presence.AwayLocked or Presence.AwayIdle => "Away",
        _ => "Not tracked",
    };

    /// The state in the main-bar slider's value text, with the away cause:
    /// "Present", "Away (locked)", "Away (idle)" or "Not tracked" (#126).
    public static string Name(this Presence presence) => presence switch
    {
        Presence.Present => "Present",
        Presence.AwayLocked => "Away (locked)",
        Presence.AwayIdle => "Away (idle)",
        _ => "Not tracked",
    };

    /// The preview's state card for a gap, e.g. `10:31 · Away (locked) — no screenshots`.
    public static string GapCard(Presence presence, string clock) => presence switch
    {
        Presence.AwayLocked => $"{clock} · Away (locked) — no screenshots",
        _ => $"{clock} · Not tracked",
    };
}

/// A presence span with parsed instants, unclipped as `presence_list` returns it.
public sealed record PresenceInterval(DateTimeOffset Start, DateTimeOffset End, Presence Presence, bool Open);

/// A span clipped to the visible range, as offsets from the day's midnight.
public sealed record VisibleSpan(TimeSpan Start, TimeSpan End, Presence Presence);

/// A screenshot with its parsed capture instant.
public sealed record Capture(DateTimeOffset At, ScreenshotItem Screenshot);

/// What hovering the bar at an instant shows in the preview (#108).
public abstract record HoverPick
{
    private HoverPick() { }

    /// The latest capture at or before the instant.
    public sealed record Shot(Capture Capture) : HoverPick
    {
        public ScreenshotItem Screenshot => Capture.Screenshot;
    }

    /// Capture was stopped (`Untracked` or locked `Away`): a placeholder, no image.
    public sealed record Gap(Presence Presence) : HoverPick;

    /// Before the day's first capture.
    public sealed record Nothing : HoverPick;
}

/// One viewed day of the Timeline: its presence spans and captures, and the
/// rules the bar applies to them (#124). The bar shows the day's wall-clock
/// times 00:00–24:00, so on a daylight-saving change the repeated hour is
/// drawn once and the skipped hour is empty.
public sealed class TimelineDay
{
    private readonly DateTime _midnight;
    private readonly TimeZoneInfo _zone;

    private TimelineDay(
        DateTime midnight,
        TimeZoneInfo zone,
        IReadOnlyList<PresenceInterval> spans,
        IReadOnlyList<Capture> captures)
    {
        _midnight = midnight;
        _zone = zone;
        DayStart = ToInstant(zone, midnight);
        DayEnd = ToInstant(zone, midnight.AddDays(1));
        Spans = spans;
        Captures = captures;
    }

    /// Local midnight starting the day.
    public DateTimeOffset DayStart { get; }

    /// Local midnight ending the day: 23 h to 25 h after <see cref="DayStart"/>.
    public DateTimeOffset DayEnd { get; }

    /// Ascending, unclipped.
    public IReadOnlyList<PresenceInterval> Spans { get; }

    /// Ascending by capture instant.
    public IReadOnlyList<Capture> Captures { get; }

    /// Neither spans nor captures: the page shows its empty state.
    public bool IsEmpty => Spans.Count == 0 && Captures.Count == 0;

    /// The wall-clock time of <paramref name="t"/> as an offset from the
    /// day's midnight; <see cref="DayEnd"/> is 24:00.
    public TimeSpan OffsetOf(DateTimeOffset t) => TimeZoneInfo.ConvertTime(t, _zone).DateTime - _midnight;

    /// The instant showing wall-clock time <paramref name="offset"/>: the first
    /// pass through a repeated hour, the end of a skipped one.
    public DateTimeOffset At(TimeSpan offset) => ToInstant(_zone, _midnight + offset);

    private static DateTimeOffset ToInstant(TimeZoneInfo zone, DateTime wall)
    {
        if (zone.IsInvalidTime(wall))
        {
            // Skipped hours start and end on whole minutes.
            wall = new DateTime(wall.Year, wall.Month, wall.Day, wall.Hour, wall.Minute, 0);
            while (zone.IsInvalidTime(wall))
                wall = wall.AddMinutes(1);
        }
        var offset = zone.IsAmbiguousTime(wall)
            ? zone.GetAmbiguousTimeOffsets(wall).Max()
            : zone.GetUtcOffset(wall);
        return new DateTimeOffset(wall, offset);
    }

    /// Spans and screenshots that fail to parse are dropped.
    public static TimelineDay Create(
        DateOnly date,
        TimeZoneInfo zone,
        IEnumerable<PresenceSpan> spans,
        IEnumerable<ScreenshotItem> screenshots)
    {
        var intervals = new List<PresenceInterval>();
        foreach (var s in spans)
        {
            if (!DateTimeOffset.TryParse(s.StartedAt, out var start)
                || !DateTimeOffset.TryParse(s.EndedAt, out var end))
                continue;
            intervals.Add(new PresenceInterval(start, end, ToPresence(s), s.Open));
        }
        intervals.Sort((a, b) => a.Start.CompareTo(b.Start));

        var captures = new List<Capture>();
        foreach (var shot in screenshots)
        {
            if (DateTimeOffset.TryParse(shot.CapturedAt, out var at))
                captures.Add(new Capture(at, shot));
        }
        captures.Sort((a, b) => a.At.CompareTo(b.At));

        return new TimelineDay(date.ToDateTime(TimeOnly.MinValue), zone, intervals, captures);
    }

    /// `Untracked` wherever no span covers <paramref name="t"/>. Spans are
    /// half-open, so a boundary belongs to the later span.
    public Presence PresenceAt(DateTimeOffset t)
    {
        foreach (var s in Spans)
        {
            if (s.Start <= t && t < s.End)
                return s.Presence;
        }
        return Presence.Untracked;
    }

    /// Where the presence at <paramref name="t"/> ends: the end of the span
    /// covering it, or in `Untracked` time the next span's start; at most 24:00.
    public DateTimeOffset PresenceEnd(DateTimeOffset t)
    {
        foreach (var s in Spans)
        {
            if (s.Start <= t && t < s.End)
                return s.End < DayEnd ? s.End : DayEnd;
            if (s.Start > t)
                return s.Start < DayEnd ? s.Start : DayEnd;
        }
        return DayEnd;
    }

    /// Every span start and end inside the day, ascending, as offsets from
    /// the day's midnight.
    public IReadOnlyList<TimeSpan> Boundaries() =>
        [.. Spans
            .SelectMany(s => new[] { OffsetOf(s.Start), OffsetOf(s.End) })
            .Where(b => b >= TimeSpan.Zero && b <= ZoomRange.MaxLength)
            .Distinct()
            .Order()];

    /// The preview for hovering at <paramref name="t"/>: a gap where capture
    /// was stopped, else the latest capture at or before <paramref name="t"/>,
    /// with no maximum look-back.
    public HoverPick Pick(DateTimeOffset t)
    {
        var presence = PresenceAt(t);
        if (presence is Presence.Untracked or Presence.AwayLocked)
            return new HoverPick.Gap(presence);

        Capture? latest = null;
        foreach (var c in Captures)
        {
            if (c.At > t) break;
            latest = c;
        }
        return latest is null ? new HoverPick.Nothing() : new HoverPick.Shot(latest);
    }

    /// The pin after clicking the bar at <paramref name="t"/> (#108): the hover
    /// pick, or no pin when that is already pinned or there is nothing to pin
    /// (a gap, or before the first capture).
    public ScreenshotItem? PinAfterClick(DateTimeOffset t, ScreenshotItem? pinned) =>
        Pick(t) is HoverPick.Shot shot && shot.Screenshot.Id != pinned?.Id ? shot.Screenshot : null;

    /// The spans overlapping <paramref name="range"/>, clipped to it.
    public IReadOnlyList<VisibleSpan> VisibleSpans(ZoomRange range)
    {
        var visible = new List<VisibleSpan>();
        foreach (var s in Spans)
        {
            var start = OffsetOf(s.Start);
            var end = OffsetOf(s.End);
            if (end <= range.Start || start >= range.End) continue;
            visible.Add(new VisibleSpan(
                start < range.Start ? range.Start : start,
                end > range.End ? range.End : end,
                s.Presence));
        }
        return visible;
    }

    /// This day with the open span's end grown to <paramref name="now"/>,
    /// clipped to 24:00 (#120). Local only: nothing is refetched.
    public TimelineDay GrowOpenSpan(DateTimeOffset now)
    {
        var grownEnd = now < DayEnd ? now : DayEnd;
        var spans = Spans
            .Select(s => s.Open && s.End < grownEnd ? s with { End = grownEnd } : s)
            .ToList();
        return new TimelineDay(_midnight, _zone, spans, Captures);
    }

    private static Presence ToPresence(PresenceSpan s) => (s.State, s.AwayCause) switch
    {
        ("Present", _) => Presence.Present,
        ("Away", "Locked") => Presence.AwayLocked,
        ("Away", "Idle") => Presence.AwayIdle,
        _ => Presence.Untracked,
    };
}
