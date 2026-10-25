using Tracey.App.Services;

namespace Tracey.App.TimelineModel;

/// The main bar's cursor: the hover line, moved by the mouse or, as the
/// value of the bar's slider, by keys (#126, decisions in #121). <see cref="At"/>
/// is an offset from the day's midnight. A boundary jump sets
/// <see cref="ShowsExtent"/>, so the value text also says where that presence ends.
public sealed record BarCursor(TimeSpan At, bool ShowsExtent = false)
{
    /// Where the cursor starts when the bar gets focus: the pinned capture's
    /// time, else the first capture in <paramref name="range"/>, else its start.
    public static BarCursor OnFocus(TimelineDay day, ZoomRange range, ScreenshotItem? pinned)
    {
        var pin = day.Captures.FirstOrDefault(c => c.Screenshot.Id == pinned?.Id);
        if (pin is not null) return new(day.OffsetOf(pin.At));
        foreach (var c in day.Captures)
        {
            var at = day.OffsetOf(c.At);
            if (at >= range.Start && at <= range.End) return new(at);
        }
        return new(range.Start);
    }

    /// The cursor after <paramref name="key"/> (a DOM `KeyboardEvent.key`):
    /// ←/→ step captures, Shift+←/→ jump span boundaries. Null for any other key.
    public BarCursor? AfterKey(TimelineDay day, string key, bool shift) => (key, shift) switch
    {
        ("ArrowLeft", false) => PreviousCapture(day),
        ("ArrowRight", false) => NextCapture(day),
        ("ArrowLeft", true) => PreviousBoundary(day),
        ("ArrowRight", true) => NextBoundary(day),
        _ => null,
    };

    /// On the latest capture before the cursor; stays put when there is none.
    public BarCursor PreviousCapture(TimelineDay day) =>
        StepTo(Captures(day).LastOrDefault(t => t < At), extent: false);

    /// On the earliest capture after the cursor; stays put when there is none.
    public BarCursor NextCapture(TimelineDay day) =>
        StepTo(Captures(day).FirstOrDefault(t => t > At), extent: false);

    /// On the latest span boundary before the cursor; stays put when there is none.
    public BarCursor PreviousBoundary(TimelineDay day) =>
        StepTo(Boundaries(day).LastOrDefault(t => t < At), extent: true);

    /// On the earliest span boundary after the cursor; stays put when there is none.
    public BarCursor NextBoundary(TimelineDay day) =>
        StepTo(Boundaries(day).FirstOrDefault(t => t > At), extent: true);

    // Nullable, so "none" isn't mistaken for 00:00.
    private static IEnumerable<TimeSpan?> Captures(TimelineDay day) =>
        day.Captures.Select(c => (TimeSpan?)day.OffsetOf(c.At));

    private static IEnumerable<TimeSpan?> Boundaries(TimelineDay day) =>
        day.Boundaries().Select(t => (TimeSpan?)t);

    // Staying put still takes the step's kind, so a capture step never keeps
    // a boundary jump's extent.
    private BarCursor StepTo(TimeSpan? target, bool extent) =>
        new(target ?? At, extent);

    /// The slider's `aria-valuenow`: the minute of the day, 0–1440.
    public int Minute => (int)Math.Round(At.TotalMinutes);

    /// The slider's `aria-valuetext`, e.g. `10:31 · Present · <window title>`,
    /// `10:31 · Away (locked), no screenshots` or, after a boundary jump,
    /// `10:15 · Away (locked), until 10:40`.
    public string ValueText(TimelineDay day)
    {
        var t = day.At(At);
        var presence = day.PresenceAt(t);
        var text = $"{ZoomRange.Clock(At)} · {presence.Name()}";
        if (ShowsExtent)
            text += $", until {ZoomRange.Clock(day.OffsetOf(day.PresenceEnd(t)))}";
        else if (presence == Presence.AwayLocked)
            text += ", no screenshots";
        if (day.Pick(t) is HoverPick.Shot shot)
            text += $" · {shot.Screenshot.WindowTitle}";
        return text;
    }
}
