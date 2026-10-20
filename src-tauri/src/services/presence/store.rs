//! `presence_spans`: applying the presence recorder's writes, and reading,
//! pruning and clearing spans (ADR-0006 decisions 1, 5 and 7).
//!
//! Timestamps are stored as UTC RFC 3339 with millisecond precision and a `Z`
//! suffix, one fixed width, so text comparison in SQL is time order.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;
use serde::Serialize;

use super::Presence;

/// A stored span as `presence_list` returns it: unclipped, and `open` only
/// for the span the presence recorder is still extending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresenceSpan {
    pub started_at: String,
    pub ended_at: String,
    pub state: String,
    pub away_cause: Option<String>,
    pub open: bool,
}

/// The stored form of an instant.
pub fn timestamp(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Insert a span.
pub fn insert(
    conn: &Connection,
    id: &str,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    presence: Presence,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO presence_spans (id, started_at, ended_at, state, away_cause) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            id,
            timestamp(started_at),
            timestamp(ended_at),
            presence.state(),
            presence.away_cause()
        ],
    )?;
    Ok(())
}

/// Move a span's end. Returns whether the span still exists.
pub fn set_end(conn: &Connection, id: &str, ended_at: DateTime<Utc>) -> rusqlite::Result<bool> {
    let updated = conn.execute(
        "UPDATE presence_spans SET ended_at = ?2 WHERE id = ?1",
        rusqlite::params![id, timestamp(ended_at)],
    )?;
    Ok(updated > 0)
}

/// Delete one span.
pub fn delete(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM presence_spans WHERE id = ?1", [id])?;
    Ok(())
}

/// The spans overlapping `[from, to)`, ascending by start. `open_span_id` is
/// the recorder's open span, if any.
pub fn list(
    conn: &Connection,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    open_span_id: Option<&str>,
) -> rusqlite::Result<Vec<PresenceSpan>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at, state, away_cause FROM presence_spans \
         WHERE started_at < ?2 AND ended_at > ?1 \
         ORDER BY started_at",
    )?;
    let spans = stmt.query_map(rusqlite::params![timestamp(from), timestamp(to)], |row| {
        let id: String = row.get(0)?;
        Ok(PresenceSpan {
            started_at: row.get(1)?,
            ended_at: row.get(2)?,
            state: row.get(3)?,
            away_cause: row.get(4)?,
            open: Some(id.as_str()) == open_span_id,
        })
    })?;
    spans.collect()
}

/// Delete the spans that ended before `cutoff`, the screenshot retention
/// cutoff. A span straddling it is kept whole (ADR-0006 decision 7).
pub fn prune_expired(conn: &Connection, cutoff: DateTime<Utc>) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM presence_spans WHERE ended_at < ?1",
        [timestamp(cutoff)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::presence::AwayCause;
    use chrono::Duration;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn seed(conn: &Connection, id: &str, started_at: &str, ended_at: &str, presence: Presence) {
        insert(conn, id, at(started_at), at(ended_at), presence).unwrap();
    }

    /// One day, 20 October, with spans before, across and after its edges.
    fn a_day_of_spans() -> Connection {
        let conn = crate::db::open_in_memory();
        let idle = Presence::Away(AwayCause::Idle);
        let locked = Presence::Away(AwayCause::Locked);
        seed(
            &conn,
            "before",
            "2026-10-19T20:00:00Z",
            "2026-10-19T21:00:00Z",
            Presence::Present,
        );
        seed(
            &conn,
            "midnight",
            "2026-10-19T23:30:00Z",
            "2026-10-20T00:30:00Z",
            idle,
        );
        seed(
            &conn,
            "noon",
            "2026-10-20T12:00:00Z",
            "2026-10-20T12:30:00Z",
            locked,
        );
        seed(
            &conn,
            "morning",
            "2026-10-20T09:00:00Z",
            "2026-10-20T11:00:00Z",
            Presence::Present,
        );
        seed(
            &conn,
            "late",
            "2026-10-20T23:00:00Z",
            "2026-10-21T01:00:00Z",
            Presence::Present,
        );
        seed(
            &conn,
            "after",
            "2026-10-21T08:00:00Z",
            "2026-10-21T09:00:00Z",
            Presence::Present,
        );
        conn
    }

    fn the_day(conn: &Connection, open_span_id: Option<&str>) -> Vec<PresenceSpan> {
        let from = at("2026-10-20T00:00:00Z");
        list(conn, from, from + Duration::days(1), open_span_id).unwrap()
    }

    #[test]
    fn spans_overlapping_the_range_are_listed_in_start_order_and_unclipped() {
        let spans = the_day(&a_day_of_spans(), None);
        let starts: Vec<&str> = spans.iter().map(|s| s.started_at.as_str()).collect();
        assert_eq!(
            starts,
            [
                "2026-10-19T23:30:00.000Z",
                "2026-10-20T09:00:00.000Z",
                "2026-10-20T12:00:00.000Z",
                "2026-10-20T23:00:00.000Z",
            ]
        );
        assert_eq!(spans[0].ended_at, "2026-10-20T00:30:00.000Z");
        assert_eq!(spans[3].ended_at, "2026-10-21T01:00:00.000Z");
    }

    #[test]
    fn a_span_touching_the_range_only_at_an_edge_is_not_listed() {
        let conn = crate::db::open_in_memory();
        seed(
            &conn,
            "ends",
            "2026-10-19T23:00:00Z",
            "2026-10-20T00:00:00Z",
            Presence::Present,
        );
        seed(
            &conn,
            "starts",
            "2026-10-21T00:00:00Z",
            "2026-10-21T01:00:00Z",
            Presence::Present,
        );
        assert_eq!(the_day(&conn, None), []);
    }

    #[test]
    fn listed_spans_carry_their_state_and_away_cause() {
        let spans = the_day(&a_day_of_spans(), None);
        let states: Vec<(&str, Option<&str>)> = spans
            .iter()
            .map(|s| (s.state.as_str(), s.away_cause.as_deref()))
            .collect();
        assert_eq!(
            states,
            [
                ("Away", Some("Idle")),
                ("Present", None),
                ("Away", Some("Locked")),
                ("Present", None),
            ]
        );
    }

    #[test]
    fn only_the_open_span_is_marked_open() {
        let spans = the_day(&a_day_of_spans(), Some("late"));
        let open: Vec<bool> = spans.iter().map(|s| s.open).collect();
        assert_eq!(open, [false, false, false, true]);
    }

    #[test]
    fn pruning_deletes_spans_that_ended_before_the_cutoff_and_keeps_a_straddling_one_whole() {
        let conn = a_day_of_spans();
        let pruned = prune_expired(&conn, at("2026-10-20T00:00:00Z")).unwrap();
        assert_eq!(pruned, 1, "only the span that ended on the 19th");
        let spans = the_day(&conn, None);
        assert_eq!(spans[0].started_at, "2026-10-19T23:30:00.000Z");
    }
}
