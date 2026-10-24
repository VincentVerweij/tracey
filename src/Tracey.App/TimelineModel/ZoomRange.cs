namespace Tracey.App.TimelineModel;

/// The visible part of the viewed day on the Timeline bar, as offsets from
/// local midnight. Always inside the day and 15 min–24 h long (#107).
public sealed record ZoomRange
{
    public static readonly TimeSpan MinLength = TimeSpan.FromMinutes(15);
    /// The whole day, 00:00–24:00 wall-clock.
    public static readonly TimeSpan MaxLength = TimeSpan.FromHours(24);

    private const double WheelFactor = 1.5;

    /// A length this close to the day snaps to it, so zooming in and back out
    /// lands on the full day despite rounding.
    private static readonly TimeSpan FullDaySnap = TimeSpan.FromMinutes(1);

    private static readonly TimeSpan[] LabelSteps =
        [.. new[] { 15, 30, 60, 120, 180, 360 }.Select(m => TimeSpan.FromMinutes(m))];

    private const int MaxLabels = 12;

    private ZoomRange(TimeSpan start, TimeSpan length)
    {
        Start = start;
        Length = length;
    }

    public static ZoomRange FullDay { get; } = new(TimeSpan.Zero, MaxLength);

    public TimeSpan Start { get; }
    public TimeSpan Length { get; }
    public TimeSpan End => Start + Length;

    public bool IsFullDay => Length >= MaxLength;

    /// `HH:MM–HH:MM`, e.g. `08:00–13:30`.
    public string RangeText => $"{Clock(Start)}–{Clock(End)}";

    /// The length is limited to 15 min–24 h, then the start is moved so the
    /// range stays inside the day.
    public static ZoomRange Create(TimeSpan start, TimeSpan length)
    {
        if (length >= MaxLength - FullDaySnap) return FullDay;
        if (length < MinLength) length = MinLength;
        var maxStart = MaxLength - length;
        start = start < TimeSpan.Zero ? TimeSpan.Zero : start > maxStart ? maxStart : start;
        return new ZoomRange(start, length);
    }

    public ZoomRange MoveTo(TimeSpan start) => Create(start, Length);

    /// One wheel step in: ÷1.5, keeping <paramref name="anchor"/> at the same
    /// place on the bar.
    public ZoomRange ZoomIn(TimeSpan anchor) => ZoomAround(anchor, 1 / WheelFactor);

    /// One wheel step out: ×1.5, keeping <paramref name="anchor"/> at the same
    /// place on the bar.
    public ZoomRange ZoomOut(TimeSpan anchor) => ZoomAround(anchor, WheelFactor);

    private ZoomRange ZoomAround(TimeSpan anchor, double factor)
    {
        var fraction = (anchor - Start) / Length;
        var length = Length * factor;
        if (length < MinLength) length = MinLength;
        return Create(anchor - length * fraction, length);
    }

    /// Where <paramref name="offset"/> falls on the bar, 0–100 inside the range.
    public double PercentOf(TimeSpan offset) => (offset - Start) / Length * 100.0;

    /// The offset at <paramref name="percent"/> of the bar.
    public TimeSpan OffsetAt(double percent) => Start + Length * (percent / 100.0);

    /// The smallest of 15 min, 30 min, 1 h, 2 h, 3 h and 6 h that puts at most
    /// 12 labels on the bar.
    public TimeSpan LabelStep =>
        LabelSteps.FirstOrDefault(step => Length / step <= MaxLabels, LabelSteps[^1]);

    /// The label positions: multiples of <see cref="LabelStep"/> in [Start, End).
    public IEnumerable<TimeSpan> LabelTimes()
    {
        var step = LabelStep;
        var first = TimeSpan.FromTicks((Start.Ticks + step.Ticks - 1) / step.Ticks * step.Ticks);
        for (var t = first; t < End; t += step)
            yield return t;
    }

    /// `HH:MM`, rounded to the nearest minute; the end of the day is `24:00`.
    public static string Clock(TimeSpan offset)
    {
        var minutes = (int)Math.Round(offset.TotalMinutes);
        return $"{minutes / 60:D2}:{minutes % 60:D2}";
    }
}
