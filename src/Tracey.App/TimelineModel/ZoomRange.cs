namespace Tracey.App.TimelineModel;

/// What a press on the overview bar grabs.
public enum BrushHit
{
    Outside,
    Body,
    LeftEdge,
    RightEdge,
}

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

    /// The width of each edge's grab area on the overview bar.
    public const double GrabPx = 8;

    /// Under this width on screen the grab areas move outside the rectangle,
    /// so a narrow one can still be dragged by its body.
    public const double NarrowPx = 24;

    private ZoomRange(TimeSpan start, TimeSpan length)
    {
        Start = start;
        Length = length;
    }

    public static ZoomRange FullDay { get; } = new(TimeSpan.Zero, MaxLength);

    public TimeSpan Start { get; }
    public TimeSpan Length { get; }
    public TimeSpan End => Start + Length;
    public TimeSpan Centre => Start + Length / 2;

    public bool IsFullDay => Length >= MaxLength;

    /// `HH:MM–HH:MM`, e.g. `08:00–13:30`.
    public string RangeText => $"{Clock(Start)}–{Clock(End)}";

    /// The length is limited to 15 min–24 h, then the start is moved so the
    /// range stays inside the day.
    public static ZoomRange Create(TimeSpan start, TimeSpan length)
    {
        if (length >= MaxLength - FullDaySnap) return FullDay;
        if (length < MinLength) length = MinLength;
        return new ZoomRange(Clamp(start, TimeSpan.Zero, MaxLength - length), length);
    }

    private static TimeSpan Clamp(TimeSpan value, TimeSpan min, TimeSpan max) =>
        value < min ? min : value > max ? max : value;

    /// Moved by <paramref name="delta"/>, keeping the length; stops at 00:00 and 24:00.
    public ZoomRange Pan(TimeSpan delta) => Create(Start + delta, Length);

    /// Moved so <paramref name="time"/> is in the middle, keeping the length;
    /// stops at 00:00 and 24:00.
    public ZoomRange CentreOn(TimeSpan time) => Create(time - Length / 2, Length);

    /// The start moved to <paramref name="start"/>, keeping the end; stops at
    /// 00:00 and 15 min before the end.
    public ZoomRange ResizeStart(TimeSpan start)
    {
        var end = End;
        start = Clamp(start, TimeSpan.Zero, end - MinLength);
        return Create(start, end - start);
    }

    /// The end moved to <paramref name="end"/>, keeping the start; stops at
    /// 24:00 and 15 min after the start.
    public ZoomRange ResizeEnd(TimeSpan end)
    {
        end = Clamp(end, Start + MinLength, MaxLength);
        return Create(Start, end - Start);
    }

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

    /// What a press at <paramref name="x"/> grabs on the overview rectangle
    /// spanning <paramref name="rectLeftPx"/> to <paramref name="rectLeftPx"/> +
    /// <paramref name="rectWidthPx"/>. Each edge's grab area is 8 px, centred on
    /// the edge, or fully outside the rectangle when it's under 24 px wide.
    public static BrushHit HitTest(double rectLeftPx, double rectWidthPx, double x)
    {
        var left = rectLeftPx;
        var right = rectLeftPx + rectWidthPx;
        if (rectWidthPx < NarrowPx)
        {
            if (x >= left - GrabPx && x < left) return BrushHit.LeftEdge;
            if (x > right && x <= right + GrabPx) return BrushHit.RightEdge;
            return x >= left && x <= right ? BrushHit.Body : BrushHit.Outside;
        }
        var half = GrabPx / 2;
        if (x >= left - half && x < left + half) return BrushHit.LeftEdge;
        if (x > right - half && x <= right + half) return BrushHit.RightEdge;
        return x >= left && x <= right ? BrushHit.Body : BrushHit.Outside;
    }

    /// The range after a press at <paramref name="fromPx"/> and a release at
    /// <paramref name="toPx"/> on an overview bar <paramref name="widthPx"/>
    /// wide: the body pans, an edge resizes that side, and outside the
    /// rectangle the press centres on its time, then pans.
    public ZoomRange Brush(double widthPx, double fromPx, double toPx)
    {
        var perPx = MaxLength / widthPx;
        var delta = perPx * (toPx - fromPx);
        return HitTest(Start / perPx, Length / perPx, fromPx) switch
        {
            BrushHit.Body => Pan(delta),
            BrushHit.LeftEdge => ResizeStart(Start + delta),
            BrushHit.RightEdge => ResizeEnd(End + delta),
            _ => CentreOn(perPx * fromPx).Pan(delta),
        };
    }

    /// Where <paramref name="offset"/> falls on the bar, 0–100 inside the range.
    public double PercentOf(TimeSpan offset) => (offset - Start) / Length * 100.0;

    /// The offset at <paramref name="percent"/> of the bar.
    public TimeSpan OffsetAt(double percent) => Start + Length * (percent / 100.0);

    /// The smallest of 15 min, 30 min, 1 h, 2 h, 3 h and 6 h that puts at most
    /// 12 labels on the bar.
    public TimeSpan LabelStep =>
        LabelSteps.FirstOrDefault(step => Length / step <= MaxLabels, LabelSteps[^1]);

    /// The label positions: multiples of <see cref="LabelStep"/> over the whole
    /// day, so the labels can follow a drag before the next render.
    public IEnumerable<TimeSpan> LabelTimes()
    {
        var step = LabelStep;
        for (var t = TimeSpan.Zero; t < MaxLength; t += step)
            yield return t;
    }

    /// `HH:MM`, rounded to the nearest minute; the end of the day is `24:00`.
    public static string Clock(TimeSpan offset)
    {
        var minutes = (int)Math.Round(offset.TotalMinutes);
        return $"{minutes / 60:D2}:{minutes % 60:D2}";
    }
}
