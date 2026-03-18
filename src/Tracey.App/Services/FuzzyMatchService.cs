namespace Tracey.App.Services;

/// <summary>
/// Pure C# fuzzy scorer used for the QuickEntryBar slash-notation dropdown.
/// Algorithm: subsequence matching with bonus for prefix, consecutive runs, and exact match.
/// </summary>
public class FuzzyMatchService
{
    // Score query against candidate. Returns 0.0 (no match) to 1.0 (exact match).
    // 0.0 means not all query chars are present as a subsequence in candidate.
    public double Score(string query, string candidate)
    {
        if (string.IsNullOrEmpty(query)) return 1.0;
        if (string.IsNullOrEmpty(candidate)) return 0.0;

        var q = query.ToLowerInvariant();
        var c = candidate.ToLowerInvariant();

        if (c == q) return 1.0;

        // Subsequence check + track first/last match index + max consecutive run
        var qi = 0;
        var firstMatch = -1;
        var lastMatch = -1;
        var maxCons = 0;
        var curCons = 0;
        var prevMatched = false;

        for (var ci = 0; ci < c.Length && qi < q.Length; ci++)
        {
            if (c[ci] == q[qi])
            {
                if (firstMatch < 0) firstMatch = ci;
                lastMatch = ci;
                curCons = prevMatched ? curCons + 1 : 1;
                if (curCons > maxCons) maxCons = curCons;
                prevMatched = true;
                qi++;
            }
            else
            {
                prevMatched = false;
            }
        }

        if (qi < q.Length) return 0.0; // not a subsequence

        var spread = (double)(lastMatch - firstMatch + 1);
        var spreadScore = q.Length / spread;          // 1.0 when all chars consecutive
        var consScore   = (double)maxCons / q.Length; // fraction of max run
        var prefixBonus = c.StartsWith(q) ? 1.0 : 0.0;

        return Math.Min(1.0, (prefixBonus * 0.35) + (spreadScore * 0.40) + (consScore * 0.25));
    }

    // Returns bool[] of length candidate.Length; true if that char participates in the match mask.
    // Used by the dropdown to highlight matching characters.
    public bool[] MatchMask(string query, string candidate)
    {
        var mask = new bool[candidate.Length];
        if (string.IsNullOrEmpty(query)) return mask;

        var q = query.ToLowerInvariant();
        var c = candidate.ToLowerInvariant();

        var qi = 0;
        for (var ci = 0; ci < c.Length && qi < q.Length; ci++)
        {
            if (c[ci] == q[qi]) { mask[ci] = true; qi++; }
        }
        return mask;
    }

    // Filter + sort a list of items by fuzzy score.
    public IEnumerable<(T Item, double Score)> RankMatches<T>(
        string query, IEnumerable<T> items, Func<T, string> nameSelector, int maxResults = 10)
    {
        if (string.IsNullOrEmpty(query))
            return items.Take(maxResults).Select(x => (x, 1.0));

        return items
            .Select(x => (Item: x, Score: Score(query, nameSelector(x))))
            .Where(x => x.Score > 0)
            .OrderByDescending(x => x.Score)
            .Take(maxResults);
    }
}
