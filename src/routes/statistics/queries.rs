use rusqlite::{params, Connection, Row};

use crate::database;
use crate::models::{
    DailyCount, DashboardTotals, ExportVisit, PathSummary, PathTotals, VisitView, PAGE_SIZE,
};
use crate::HandlerResult;

pub(super) fn dashboard_totals(connection: &Connection) -> rusqlite::Result<DashboardTotals> {
    connection.query_row(
        "SELECT COUNT(*),
                COUNT(DISTINCT path),
                COALESCE(SUM(CASE WHEN date(timestamp) = date('now', 'localtime') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN timestamp >= datetime('now', 'localtime', '-6 days', 'start of day') THEN 1 ELSE 0 END), 0)
         FROM visitors",
        [],
        |row| {
            Ok(DashboardTotals {
                total: row.get(0)?,
                unique_paths: row.get(1)?,
                today: row.get(2)?,
                week: row.get(3)?,
            })
        },
    )
}

pub(super) fn path_summaries(
    connection: &Connection,
    total: i64,
) -> HandlerResult<Vec<PathSummary>> {
    let mut statement = connection
        .prepare(
            "SELECT COUNT(*) AS count, path
             FROM visitors GROUP BY path ORDER BY count DESC, path ASC",
        )
        .map_err(database::map_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(PathSummary::new(row.get(0)?, row.get(1)?, total))
        })
        .map_err(database::map_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database::map_error)
}

pub(super) fn path_totals(
    connection: &Connection,
    path: &str,
    all_paths: bool,
) -> HandlerResult<PathTotals> {
    let query = if all_paths {
        "SELECT COUNT(*), COUNT(DISTINCT ip),
                COALESCE(SUM(CASE WHEN date(timestamp) = date('now', 'localtime') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN timestamp >= datetime('now', 'localtime', '-6 days', 'start of day') THEN 1 ELSE 0 END), 0)
         FROM visitors"
    } else {
        "SELECT COUNT(*), COUNT(DISTINCT ip),
                COALESCE(SUM(CASE WHEN date(timestamp) = date('now', 'localtime') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN timestamp >= datetime('now', 'localtime', '-6 days', 'start of day') THEN 1 ELSE 0 END), 0)
         FROM visitors WHERE path = ?1"
    };

    let map_row = |row: &Row<'_>| {
        Ok(PathTotals {
            total: row.get(0)?,
            unique_ips: row.get(1)?,
            today: row.get(2)?,
            week: row.get(3)?,
        })
    };
    let result = if all_paths {
        connection.query_row(query, [], map_row)
    } else {
        connection.query_row(query, [path], map_row)
    };
    result.map_err(database::map_error)
}

pub(super) fn daily_counts(
    connection: &Connection,
    path: &str,
    all_paths: bool,
) -> HandlerResult<Vec<DailyCount>> {
    let query = if all_paths {
        "WITH RECURSIVE days(day) AS (
             SELECT date('now', 'localtime', '-29 days')
             UNION ALL SELECT date(day, '+1 day') FROM days WHERE day < date('now', 'localtime')
         )
         SELECT days.day, COUNT(visitors.id)
         FROM days LEFT JOIN visitors
           ON visitors.timestamp >= days.day
          AND visitors.timestamp < datetime(days.day, '+1 day')
         GROUP BY days.day ORDER BY days.day"
    } else {
        "WITH RECURSIVE days(day) AS (
             SELECT date('now', 'localtime', '-29 days')
             UNION ALL SELECT date(day, '+1 day') FROM days WHERE day < date('now', 'localtime')
         )
         SELECT days.day, COUNT(visitors.id)
         FROM days LEFT JOIN visitors
           ON visitors.timestamp >= days.day
          AND visitors.timestamp < datetime(days.day, '+1 day')
          AND visitors.path = ?1
         GROUP BY days.day ORDER BY days.day"
    };

    let values = {
        let mut statement = connection.prepare(query).map_err(database::map_error)?;
        let rows = if all_paths {
            statement.query_map([], map_daily_row)
        } else {
            statement.query_map([path], map_daily_row)
        }
        .map_err(database::map_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database::map_error)?
    };
    Ok(DailyCount::from_values(values))
}

fn map_daily_row(row: &Row<'_>) -> rusqlite::Result<(String, i64)> {
    Ok((row.get(0)?, row.get(1)?))
}

pub(super) fn paginated_visits(
    connection: &Connection,
    path: &str,
    all_paths: bool,
    offset: i64,
) -> HandlerResult<Vec<VisitView>> {
    let query = if all_paths {
        "SELECT strftime('%Y-%m-%d %H:%M:%S', timestamp), path, ip, json
         FROM visitors ORDER BY timestamp DESC, id DESC LIMIT ?1 OFFSET ?2"
    } else {
        "SELECT strftime('%Y-%m-%d %H:%M:%S', timestamp), path, ip, json
         FROM visitors WHERE path = ?1 ORDER BY timestamp DESC, id DESC LIMIT ?2 OFFSET ?3"
    };
    let mut statement = connection.prepare(query).map_err(database::map_error)?;
    let rows = if all_paths {
        statement.query_map(params![PAGE_SIZE, offset], VisitView::from_row)
    } else {
        statement.query_map(params![path, PAGE_SIZE, offset], VisitView::from_row)
    }
    .map_err(database::map_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database::map_error)
}

pub(super) fn export_visits(connection: &Connection) -> HandlerResult<Vec<ExportVisit>> {
    let mut statement = connection
        .prepare("SELECT timestamp, path, ip, json FROM visitors ORDER BY timestamp DESC, id DESC")
        .map_err(database::map_error)?;
    let rows = statement
        .query_map([], ExportVisit::from_row)
        .map_err(database::map_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database::map_error)
}
