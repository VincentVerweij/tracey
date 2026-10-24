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
/// rules the bar applies to them (#124). The day is taken as 24 h from its
/// local midnight.
public sealed class TimelineDay
{
    public static readonly TimeSpan Length = TimeSpan.FromHours(24);

    private TimelineDay(
        DateTimeOffset dayStart,
        IReadOnlyList<PresenceInterval> spans,
        IReadOnlyList<Capture> captures)
    {
        DayStart = dayStart;
        Spans = spans;
        Captures = captures;
    }

    public DateTimeOffset DayStart { get; }
    public DateTimeOffset DayEnd => DayStart + Length;

    /// Ascending, unclipped.
    public IReadOnlyList<PresenceInterval> Spans { get; }

    /// Ascending by capture instant.
    public IReadOnlyList<Capture> Captures { get; }

    /// Neither spans nor captures: the page shows its empty state.
    public bool IsEmpty => Spans.Count == 0 && Captures.Count == 0;

    public TimeSpan OffsetOf(DateTimeOffset t) => t - DayStart;

    public DateTimeOffset At(TimeSpan offset) => DayStart + offset;

    /// Spans and screenshots that fail to parse are dropped.
    public static TimelineDay Create(
        DateTimeOffset dayStart,
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

        return new TimelineDay(dayStart, intervals, captures);
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
    /// pick, or no pin when that is already pinned or <paramref name="t"/> is
    /// in a gap. Before the first capture the pin is left as it was.
    public ScreenshotItem? Click(DateTimeOffset t, ScreenshotItem? pinned) => Pick(t) switch
    {
        HoverPick.Shot shot when shot.Screenshot.Id == pinned?.Id => null,
        HoverPick.Shot shot => shot.Screenshot,
        HoverPick.Gap => null,
        _ => pinned,
    };

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
        return new TimelineDay(DayStart, spans, Captures);
    }

    private static Presence ToPresence(PresenceSpan s) => (s.State, s.AwayCause) switch
    {
        ("Present", _) => Presence.Present,
        ("Away", "Locked") => Presence.AwayLocked,
        ("Away", "Idle") => Presence.AwayIdle,
        _ => Presence.Untracked,
    };
}
