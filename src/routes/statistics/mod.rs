mod queries;

use rocket::http::Status;
use rocket::response::content;
use rocket::response::Redirect;
use rocket::{get, State};
use rocket_dyn_templates::Template;
use serde_json::json;

use crate::config::AppConfig;
use crate::database;
use crate::models::Pagination;
use crate::security::BasicAuthGuard;
use crate::utils::encode_path_segment;
use crate::HandlerResult;

const CHART_DAYS: i64 = 30;

#[get("/statistics")]
pub(crate) fn get_statistics(
    _auth: BasicAuthGuard,
    config: &State<AppConfig>,
) -> HandlerResult<Template> {
    let connection = database::open(config)?;
    let totals = queries::dashboard_totals(&connection).map_err(database::map_error)?;
    let entries = queries::path_summaries(&connection, totals.total)?;

    Ok(Template::render(
        "statistics/index",
        json!({
            "site_title": config.site_title(),
            "page_title": "Статистика",
            "active_page": "overview",
            "theme_auto": config.theme_auto(),
            "total": totals.total,
            "unique_paths": totals.unique_paths,
            "today": totals.today,
            "week": totals.week,
            "entries": entries,
        }),
    ))
}

#[get("/statistics/<path>?<page>")]
pub(crate) fn get_statistics_path(
    _auth: BasicAuthGuard,
    path: &str,
    page: Option<usize>,
    config: &State<AppConfig>,
) -> HandlerResult<Template> {
    let connection = database::open(config)?;
    let all_paths = path == "__all__";
    let totals = queries::path_totals(&connection, path, all_paths)?;
    let pagination = Pagination::new(totals.total, page);
    let daily = queries::daily_counts(&connection, path, all_paths)?;
    let max_daily = daily.iter().map(|day| day.count).max().unwrap_or(0);
    let chart_points = daily
        .iter()
        .map(|point| format!("{},{}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(" ");
    let visits = queries::paginated_visits(&connection, path, all_paths, pagination.offset)?;
    let path_url = encode_path_segment(path);
    let page_base = if all_paths {
        "/statistics/__all__".to_owned()
    } else {
        format!("/statistics/{path_url}")
    };
    let average_daily = daily.iter().map(|day| day.count).sum::<i64>() as f64 / CHART_DAYS as f64;

    Ok(Template::render(
        "statistics/path",
        json!({
            "site_title": config.site_title(),
            "page_title": if all_paths { "Все посещения" } else { "Статистика пути" },
            "active_page": if all_paths { "visits" } else { "overview" },
            "theme_auto": config.theme_auto(),
            "path": path,
            "all_paths": all_paths,
            "total": totals.total,
            "unique_ips": totals.unique_ips,
            "today": totals.today,
            "week": totals.week,
            "average_daily": format!("{average_daily:.1}"),
            "max_daily": max_daily,
            "daily": daily,
            "chart_points": chart_points,
            "visits": visits,
            "page_base": page_base,
            "current_page": pagination.current_page,
            "total_pages": pagination.total_pages,
            "has_prev": pagination.has_prev,
            "has_next": pagination.has_next,
            "prev_page": pagination.prev_page,
            "next_page": pagination.next_page,
            "range_start": pagination.range_start,
            "range_end": pagination.range_end,
        }),
    ))
}

#[get("/statistics/recent?<page>")]
pub(crate) fn get_statistics_recent(
    _auth: BasicAuthGuard,
    page: Option<usize>,
    config: &State<AppConfig>,
) -> HandlerResult<Template> {
    let connection = database::open(config)?;
    let totals = queries::dashboard_totals(&connection).map_err(database::map_error)?;
    let pagination = Pagination::new(totals.total, page);
    let visits = queries::paginated_visits(&connection, "", true, pagination.offset)?;

    Ok(Template::render(
        "statistics/recent",
        json!({
            "site_title": config.site_title(),
            "page_title": "Последние посещения",
            "active_page": "recent",
            "theme_auto": config.theme_auto(),
            "total": totals.total,
            "unique_paths": totals.unique_paths,
            "today": totals.today,
            "week": totals.week,
            "visits": visits,
            "page_base": "/statistics/recent",
            "current_page": pagination.current_page,
            "total_pages": pagination.total_pages,
            "has_prev": pagination.has_prev,
            "has_next": pagination.has_next,
            "prev_page": pagination.prev_page,
            "next_page": pagination.next_page,
            "range_start": pagination.range_start,
            "range_end": pagination.range_end,
        }),
    ))
}

#[get("/statistics_self")]
pub(crate) fn get_statistics_self(_auth: BasicAuthGuard) -> Redirect {
    Redirect::to("/statistics")
}

#[get("/statistics_self_full_json")]
pub(crate) fn get_statistics_self_full_json(
    _auth: BasicAuthGuard,
    config: &State<AppConfig>,
) -> HandlerResult<content::RawJson<String>> {
    let connection = database::open(config)?;
    let visitors = queries::export_visits(&connection)?;
    let body = serde_json::to_string(&visitors).map_err(|error| {
        eprintln!("cannot serialize statistics export: {error}");
        Status::InternalServerError
    })?;
    Ok(content::RawJson(body))
}
