using Tracey.App.Services;

namespace Tracey.Tests;

/// <summary>
/// xUnit tests for FuzzyMatchService — T050 Phase 7 US5.
/// Written BEFORE implementation (TDD gate).
/// </summary>
public class FuzzyMatchTests
{
    private readonly FuzzyMatchService _sut = new();

    // ─── Score: basic ──────────────────────────────────────────────────────

    [Fact]
    public void Score_EmptyQuery_ReturnsOne()
    {
        Assert.Equal(1.0, _sut.Score("", "anything"));
    }

    [Fact]
    public void Score_ExactMatch_ReturnsOne()
    {
        Assert.Equal(1.0, _sut.Score("acme", "acme"));
    }

    [Fact]
    public void Score_CaseInsensitive_ExactMatch()
    {
        Assert.Equal(1.0, _sut.Score("ACME", "acme"));
    }

    [Fact]
    public void Score_CandidateEmpty_ReturnsZero()
    {
        Assert.Equal(0.0, _sut.Score("acme", ""));
    }

    [Fact]
    public void Score_QueryNotSubsequence_ReturnsZero()
    {
        Assert.Equal(0.0, _sut.Score("xyz", "acme"));
    }

    [Fact]
    public void Score_MissingOneChar_ReturnsZero()
    {
        // 'z' not in "acme"
        Assert.Equal(0.0, _sut.Score("acmez", "acme"));
    }

    // ─── Score: ordering ───────────────────────────────────────────────────

    [Fact]
    public void Score_PrefixScoresHigherThanSpread()
    {
        // "acm" matches "Acme" (prefix) and "Application Manager" (spread)
        var prefixScore = _sut.Score("acm", "Acme");
        var spreadScore = _sut.Score("acm", "Application Manager");
        Assert.True(prefixScore > spreadScore,
            $"prefix '{prefixScore}' should beat spread '{spreadScore}'");
    }

    [Fact]
    public void Score_ConsecutiveRunHigherThanDisjoint()
    {
        // "abc" matches "abcdef" (all consecutive) vs "axbxcdef" (disjoint)
        var consScore    = _sut.Score("abc", "abcdef");
        var disjointScore = _sut.Score("abc", "axbxcdef");
        Assert.True(consScore > disjointScore,
            $"consecutive '{consScore}' should beat disjoint '{disjointScore}'");
    }

    [Fact]
    public void Score_ExactBeatsPrefix()
    {
        var exact  = _sut.Score("acme", "acme");
        var prefix = _sut.Score("acme", "acme corp");
        Assert.True(exact >= prefix, $"exact '{exact}' should be >= prefix '{prefix}'");
    }

    // ─── Score: real-world examples ─────────────────────────────────────

    [Theory]
    [InlineData("acm",   "Acme")]
    [InlineData("bug",   "Bug Fix")]
    [InlineData("inv",   "Investigate login crash")]
    public void Score_PartialQuery_NonZero(string query, string candidate)
    {
        Assert.True(_sut.Score(query, candidate) > 0,
            $"'{query}' should match '{candidate}'");
    }

    [Theory]
    [InlineData("zzz", "Acme")]
    [InlineData("xyz", "Bug Fix")]
    public void Score_UnrelatedQuery_Zero(string query, string candidate)
    {
        Assert.Equal(0.0, _sut.Score(query, candidate));
    }

    // ─── MatchMask ─────────────────────────────────────────────────────────

    [Fact]
    public void MatchMask_ReturnsCorrectLength()
    {
        var mask = _sut.MatchMask("ac", "Acme");
        Assert.Equal(4, mask.Length);
    }

    [Fact]
    public void MatchMask_MarksMatchedChars()
    {
        // "ac" against "Acme": A(0) matches 'a', c(1) matches 'c'
        var mask = _sut.MatchMask("ac", "Acme");
        Assert.True(mask[0], "index 0 should be matched");  // A
        Assert.True(mask[1], "index 1 should be matched");  // c
    }

    [Fact]
    public void MatchMask_NoMatch_AllFalse()
    {
        var mask = _sut.MatchMask("zzz", "Acme");
        Assert.All(mask, bit => Assert.False(bit));
    }

    [Fact]
    public void MatchMask_EmptyQuery_AllFalse()
    {
        var mask = _sut.MatchMask("", "Acme");
        Assert.All(mask, bit => Assert.False(bit));
    }

    // ─── RankMatches ───────────────────────────────────────────────────────

    [Fact]
    public void RankMatches_OrdersByScoreDescending()
    {
        var items = new[] { "Application Manager", "Acme Corp", "Acme" };
        var ranked = _sut.RankMatches("acm", items, x => x).Select(r => r.Item).ToList();
        // "Acme" (exact prefix) should rank first or second, before "Application Manager"
        var acmeIdx    = ranked.IndexOf("Acme");
        var appMgrIdx  = ranked.IndexOf("Application Manager");
        Assert.True(acmeIdx < appMgrIdx || appMgrIdx == -1,
            "Acme should appear before Application Manager");
    }

    [Fact]
    public void RankMatches_FiltersZeroScores()
    {
        var items = new[] { "Acme", "Totally Unrelated" };
        var ranked = _sut.RankMatches("acm", items, x => x).ToList();
        Assert.DoesNotContain(ranked, r => r.Item == "Totally Unrelated");
        Assert.All(ranked, r => Assert.True(r.Score > 0));
    }

    [Fact]
    public void RankMatches_RespectsMaxResults()
    {
        var items = Enumerable.Range(1, 20).Select(i => $"Project{i}");
        var ranked = _sut.RankMatches("proj", items, x => x, maxResults: 5).ToList();
        Assert.True(ranked.Count <= 5);
    }

    [Fact]
    public void RankMatches_EmptyQuery_ReturnsUpToMax()
    {
        var items = new[] { "A", "B", "C", "D", "E" };
        var ranked = _sut.RankMatches("", items, x => x, maxResults: 3).ToList();
        Assert.True(ranked.Count <= 3);
    }
}
