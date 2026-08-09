use std::collections::BTreeMap;

use rusqlite::Row;
use serde::Serialize;

use crate::utils::{encode_path_segment, strip_url_query, truncate_chars};

pub(crate) const PAGE_SIZE: i64 = 20;

#[derive(Serialize, Debug)]
pub(crate) struct VisitView {
    timestamp: String,
    path: String,
    path_url: String,
    ip: String,
    user_agent: String,
    referer: String,
    language: String,
    headers_pretty: String,
}

impl VisitView {
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let timestamp = row.get(0)?;
        let path: String = row.get(1)?;
        let ip = row.get(2)?;
        let raw_metadata: String = row.get(3)?;
        let metadata: BTreeMap<String, String> =
            serde_json::from_str(&raw_metadata).unwrap_or_default();
        let user_agent = metadata.get("user-agent").map_or_else(
            || "Not provided".to_owned(),
            |value| truncate_chars(value, 240),
        );
        let referer = metadata.get("referer").map_or_else(
            || "Direct visit".to_owned(),
            |value| truncate_chars(strip_url_query(value), 240),
        );
        let language = metadata
            .get("accept-language")
            .map_or_else(|| "—".to_owned(), |value| truncate_chars(value, 80));
        let headers_pretty =
            serde_json::to_string_pretty(&metadata).unwrap_or_else(|_| "{}".to_owned());

        Ok(Self {
            timestamp,
            path_url: encode_path_segment(&path),
            path,
            ip,
            user_agent,
            referer,
            language,
            headers_pretty,
        })
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct ExportVisit {
    timestamp: String,
    path: String,
    ip: String,
    metadata: BTreeMap<String, String>,
}

impl ExportVisit {
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let raw_metadata: String = row.get(3)?;
        Ok(Self {
            timestamp: row.get(0)?,
            path: row.get(1)?,
            ip: row.get(2)?,
            metadata: serde_json::from_str(&raw_metadata).unwrap_or_default(),
        })
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct PathSummary {
    count: i64,
    path: String,
    path_url: String,
    share_percent: String,
}

impl PathSummary {
    pub(crate) fn new(count: i64, path: String, total: i64) -> Self {
        let share = if total == 0 {
            0.0
        } else {
            (count as f64 / total as f64) * 100.0
        };
        Self {
            count,
            path_url: encode_path_segment(&path),
            path,
            share_percent: format!("{share:.1}"),
        }
    }
}

#[derive(Serialize, Debug)]
pub(crate) struct DailyCount {
    pub(crate) date: String,
    pub(crate) count: i64,
    pub(crate) x: i64,
    pub(crate) y: i64,
}

impl DailyCount {
    pub(crate) fn from_values(values: Vec<(String, i64)>) -> Vec<Self> {
        let max_count = values
            .iter()
            .map(|(_, count)| *count)
            .max()
            .unwrap_or(1)
            .max(1);
        let last_index = i64::try_from(values.len().saturating_sub(1))
            .unwrap_or(1)
            .max(1);

        values
            .into_iter()
            .enumerate()
            .map(|(index, (date, count))| Self {
                date,
                count,
                x: 16 + (i64::try_from(index).unwrap_or_default() * 688 / last_index),
                y: 196 - (count * 164 / max_count),
            })
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct DashboardTotals {
    pub(crate) total: i64,
    pub(crate) unique_paths: i64,
    pub(crate) today: i64,
    pub(crate) week: i64,
}

#[derive(Debug)]
pub(crate) struct PathTotals {
    pub(crate) total: i64,
    pub(crate) unique_ips: i64,
    pub(crate) today: i64,
    pub(crate) week: i64,
}

#[derive(Debug)]
pub(crate) struct Pagination {
    pub(crate) current_page: i64,
    pub(crate) total_pages: i64,
    pub(crate) offset: i64,
    pub(crate) has_prev: bool,
    pub(crate) has_next: bool,
    pub(crate) prev_page: i64,
    pub(crate) next_page: i64,
    pub(crate) range_start: i64,
    pub(crate) range_end: i64,
}

impl Pagination {
    pub(crate) fn new(total_rows: i64, requested_page: Option<usize>) -> Self {
        let total_pages = ((total_rows.max(1) - 1) / PAGE_SIZE) + 1;
        let requested = requested_page
            .and_then(|page| i64::try_from(page).ok())
            .unwrap_or(1);
        let current_page = requested.clamp(1, total_pages);
        let offset = (current_page - 1) * PAGE_SIZE;
        let range_start = if total_rows == 0 { 0 } else { offset + 1 };
        let range_end = (offset + PAGE_SIZE).min(total_rows);

        Self {
            current_page,
            total_pages,
            offset,
            has_prev: current_page > 1,
            has_next: current_page < total_pages,
            prev_page: (current_page - 1).max(1),
            next_page: (current_page + 1).min(total_pages),
            range_start,
            range_end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_clamps_page_to_last_available_page() {
        assert_eq!(Pagination::new(45, Some(99)).current_page, 3);
    }
}
