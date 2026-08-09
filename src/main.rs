use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Cursor};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::{relative, FileServer};
use rocket::http::{Header, Status};
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::content;
use rocket::response::{self, Redirect, Responder, Response};
use rocket::{catch, catchers, get, routes, Build, Rocket, State};
use rocket_dyn_templates::Template;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use serde_json::json;
use subtle::ConstantTimeEq;

const DEFAULT_DB_PATH: &str = "./wapp_simple_stats_rust.db";
const PAGE_SIZE: i64 = 20;
const CHART_DAYS: i64 = 30;
const MAX_AUTH_HEADER_BYTES: usize = 8 * 1024;
const MAX_PATH_BYTES: usize = 255;

type HandlerResult<T> = Result<T, Status>;

#[derive(Deserialize, Debug, Clone)]
struct Site {
    title: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Theme {
    auto: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct Database {
    path: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Basic {
    username: String,
    password: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Auth {
    enabled: bool,
    basic: Basic,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
struct Privacy {
    anonymize_ip: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self { anonymize_ip: true }
    }
}

#[derive(Deserialize, Debug, Clone)]
struct AppConfig {
    site: Site,
    theme: Theme,
    auth: Auth,
    #[serde(default)]
    privacy: Privacy,
    database: Option<Database>,
}

fn load_config() -> io::Result<AppConfig> {
    let config_text = fs::read_to_string("config.yaml").map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot read config.yaml (copy config.example.yaml first): {error}"),
        )
    })?;

    let mut config: AppConfig = serde_yaml_ng::from_str(&config_text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid config.yaml: {error}"),
        )
    })?;

    if let Ok(username) = env::var("WAPP_STATS_USERNAME") {
        config.auth.basic.username = username;
    }
    if let Ok(password) = env::var("WAPP_STATS_PASSWORD") {
        config.auth.basic.password = password;
    }

    validate_config(&config)?;
    if !config.auth.enabled {
        eprintln!("WARNING: statistics authentication is disabled; do not expose the app publicly");
    }

    Ok(config)
}

fn validate_config(config: &AppConfig) -> io::Result<()> {
    if config.site.title.trim().is_empty() {
        return Err(invalid_config("site.title must not be empty"));
    }
    if get_db_path(config).trim().is_empty() {
        return Err(invalid_config("database.path must not be empty"));
    }
    if config.auth.enabled && config.auth.basic.username.trim().is_empty() {
        return Err(invalid_config("auth.basic.username must not be empty"));
    }
    if config.auth.enabled && config.auth.basic.password.len() < 12 {
        return Err(invalid_config(
            "auth.basic.password must contain at least 12 bytes; WAPP_STATS_PASSWORD can override it",
        ));
    }
    if config.auth.enabled && config.auth.basic.password == "replace-with-a-random-password" {
        return Err(invalid_config(
            "replace the example auth password or set WAPP_STATS_PASSWORD",
        ));
    }
    Ok(())
}

fn invalid_config(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn get_db_path(config: &AppConfig) -> &str {
    config
        .database
        .as_ref()
        .map_or(DEFAULT_DB_PATH, |database| database.path.as_str())
}

fn open_database(config: &AppConfig) -> HandlerResult<Connection> {
    let connection = Connection::open(get_db_path(config)).map_err(database_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(database_error)?;
    Ok(connection)
}

fn database_error(error: rusqlite::Error) -> Status {
    eprintln!("database operation failed: {error}");
    Status::InternalServerError
}

pub struct BasicAuthGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BasicAuthGuard {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let Some(config) = request.rocket().state::<AppConfig>() else {
            return Outcome::Error((Status::InternalServerError, ()));
        };

        if !config.auth.enabled {
            return Outcome::Success(Self);
        }

        let authenticated = request
            .headers()
            .get_one("Authorization")
            .is_some_and(|header| credentials_match(header, &config.auth.basic));

        if authenticated {
            Outcome::Success(Self)
        } else {
            Outcome::Error((Status::Unauthorized, ()))
        }
    }
}

fn credentials_match(header: &str, expected: &Basic) -> bool {
    if header.len() > MAX_AUTH_HEADER_BYTES {
        return false;
    }

    let Some((scheme, encoded)) = header.split_once(' ') else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("basic") || encoded.is_empty() {
        return false;
    }

    let Ok(decoded) = general_purpose::STANDARD.decode(encoded) else {
        return false;
    };
    let Some(separator) = decoded.iter().position(|byte| *byte == b':') else {
        return false;
    };

    let username = &decoded[..separator];
    let password = &decoded[separator + 1..];
    bool::from(
        username.ct_eq(expected.username.as_bytes()) & password.ct_eq(expected.password.as_bytes()),
    )
}

struct Unauthorized;

impl<'r> Responder<'r, 'static> for Unauthorized {
    fn respond_to(self, _: &Request<'_>) -> response::Result<'static> {
        Response::build()
            .status(Status::Unauthorized)
            .header(Header::new(
                "WWW-Authenticate",
                "Basic realm=\"Simple Stats\", charset=\"UTF-8\"",
            ))
            .header(Header::new("Cache-Control", "no-store"))
            .sized_body(0, Cursor::new(String::new()))
            .ok()
    }
}

#[catch(401)]
fn unauthorized() -> Unauthorized {
    Unauthorized
}

struct SecurityHeaders;

#[rocket::async_trait]
impl Fairing for SecurityHeaders {
    fn info(&self) -> Info {
        Info {
            name: "Security response headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("X-Content-Type-Options", "nosniff"));
        response.set_header(Header::new("X-Frame-Options", "DENY"));
        response.set_header(Header::new("Referrer-Policy", "no-referrer"));
        response.set_header(Header::new(
            "Permissions-Policy",
            "camera=(), microphone=(), geolocation=()",
        ));
        response.set_header(Header::new(
            "Content-Security-Policy",
            "default-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'",
        ));

        let path = request.uri().path().as_str();
        if path.starts_with("/statistics") {
            response.set_header(Header::new("Cache-Control", "no-store"));
            response.set_header(Header::new("Vary", "Authorization"));
        } else if path.starts_with("/assets/") {
            response.set_header(Header::new(
                "Cache-Control",
                "public, max-age=604800, immutable",
            ));
        }
    }
}

#[derive(Debug)]
struct RequestData {
    ip: String,
    headers: BTreeMap<String, String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequestData {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ip = request
            .remote()
            .map(|address| address.ip().to_string())
            .unwrap_or_else(|| "unknown".to_owned());

        let mut headers = BTreeMap::new();
        copy_safe_header(request, &mut headers, "user-agent", 512);
        copy_safe_header(request, &mut headers, "accept-language", 128);
        if let Some(referer) = request.headers().get_one("referer") {
            headers.insert(
                "referer".to_owned(),
                truncate_chars(strip_url_query(referer), 512),
            );
        }

        Outcome::Success(Self { ip, headers })
    }
}

fn copy_safe_header(
    request: &Request<'_>,
    target: &mut BTreeMap<String, String>,
    name: &str,
    max_chars: usize,
) {
    if let Some(value) = request.headers().get_one(name) {
        target.insert(name.to_owned(), truncate_chars(value, max_chars));
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn strip_url_query(value: &str) -> &str {
    value
        .split_once(['?', '#'])
        .map_or(value, |(safe_part, _)| safe_part)
}

fn anonymize_ip(value: &str) -> String {
    match value.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            let [a, b, c, _] = address.octets();
            Ipv4Addr::new(a, b, c, 0).to_string()
        }
        Ok(IpAddr::V6(address)) => {
            let masked = u128::from(address) & (!0_u128 << 80);
            Ipv6Addr::from(masked).to_string()
        }
        Err(_) => "unknown".to_owned(),
    }
}

struct SvgCounter {
    value: String,
}

impl<'r> Responder<'r, 'static> for SvgCounter {
    fn respond_to(self, _: &Request<'_>) -> response::Result<'static> {
        Response::build()
            .header(Header::new(
                "Cache-Control",
                "max-age=0, no-cache, no-store, must-revalidate",
            ))
            .header(Header::new("Content-Type", "image/svg+xml; charset=utf-8"))
            .header(Header::new("Cross-Origin-Resource-Policy", "cross-origin"))
            .sized_body(self.value.len(), Cursor::new(self.value))
            .ok()
    }
}

#[get("/")]
fn get_root(config: &State<AppConfig>) -> Template {
    Template::render(
        "landing",
        json!({
            "site_title": &config.site.title,
            "page_title": "Главная",
            "active_page": "home",
            "theme_auto": config.theme.auto,
        }),
    )
}

#[get("/counter/<path>")]
fn get_counter(
    request_data: RequestData,
    path: &str,
    config: &State<AppConfig>,
) -> HandlerResult<SvgCounter> {
    validate_counter_path(path)?;
    let connection = open_database(config)?;
    let headers_json = serde_json::to_string(&request_data.headers).map_err(|error| {
        eprintln!("cannot serialize request metadata: {error}");
        Status::InternalServerError
    })?;
    let ip = if config.privacy.anonymize_ip {
        anonymize_ip(&request_data.ip)
    } else {
        request_data.ip
    };

    connection
        .execute(
            "INSERT INTO visitors (path, ip, json) VALUES (?1, ?2, ?3)",
            params![path, ip, headers_json],
        )
        .map_err(database_error)?;

    let total_for_path: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM visitors WHERE path = ?1",
            [path],
            |row| row.get(0),
        )
        .map_err(database_error)?;

    let counter_formatted = format!("{total_for_path:0>6}");
    let svg = r###"<svg xmlns="http://www.w3.org/2000/svg" width="110" height="20" role="img" aria-label="statistics: {NUMBER}">
<title>statistics: {NUMBER}</title><linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient><clipPath id="r"><rect width="110" height="20" rx="3" fill="#fff"/></clipPath><g clip-path="url(#r)"><rect width="59" height="20" fill="#334155"/><rect x="59" width="51" height="20" fill="#4f46e5"/><rect width="110" height="20" fill="url(#s)"/></g><g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110"><text aria-hidden="true" x="305" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="490">statistics</text><text x="305" y="140" transform="scale(.1)" textLength="490">statistics</text><text aria-hidden="true" x="835" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="410">{NUMBER}</text><text x="835" y="140" transform="scale(.1)" textLength="410">{NUMBER}</text></g></svg>"###
        .replace("{NUMBER}", &counter_formatted);

    Ok(SvgCounter { value: svg })
}

fn validate_counter_path(path: &str) -> Result<(), Status> {
    let is_invalid = path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.chars().any(char::is_control)
        || path == "."
        || path == "..";
    if is_invalid {
        Err(Status::BadRequest)
    } else {
        Ok(())
    }
}

#[derive(Serialize, Debug)]
struct VisitView {
    timestamp: String,
    path: String,
    path_url: String,
    ip: String,
    user_agent: String,
    referer: String,
    language: String,
    headers_pretty: String,
}

#[derive(Serialize, Debug)]
struct ExportVisit {
    timestamp: String,
    path: String,
    ip: String,
    metadata: BTreeMap<String, String>,
}

#[derive(Serialize, Debug)]
struct PathSummary {
    count: i64,
    path: String,
    path_url: String,
    share_percent: String,
}

#[derive(Serialize, Debug)]
struct DailyCount {
    date: String,
    count: i64,
    x: i64,
    y: i64,
}

#[derive(Debug)]
struct Pagination {
    current_page: i64,
    total_pages: i64,
    offset: i64,
    has_prev: bool,
    has_next: bool,
    prev_page: i64,
    next_page: i64,
    range_start: i64,
    range_end: i64,
}

impl Pagination {
    fn new(total_rows: i64, requested_page: Option<usize>) -> Self {
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

fn map_row_to_visit(row: &Row<'_>) -> rusqlite::Result<VisitView> {
    let timestamp = row.get(0)?;
    let path: String = row.get(1)?;
    let ip = row.get(2)?;
    let raw_metadata: String = row.get(3)?;
    let metadata: BTreeMap<String, String> =
        serde_json::from_str(&raw_metadata).unwrap_or_default();
    let user_agent = metadata.get("user-agent").map_or_else(
        || "Не указан".to_owned(),
        |value| truncate_chars(value, 240),
    );
    let referer = metadata.get("referer").map_or_else(
        || "Прямой переход".to_owned(),
        |value| truncate_chars(strip_url_query(value), 240),
    );
    let language = metadata
        .get("accept-language")
        .map_or_else(|| "—".to_owned(), |value| truncate_chars(value, 80));
    let headers_pretty =
        serde_json::to_string_pretty(&metadata).unwrap_or_else(|_| "{}".to_owned());

    Ok(VisitView {
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

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

fn dashboard_totals(connection: &Connection) -> rusqlite::Result<(i64, i64, i64, i64)> {
    connection.query_row(
        "SELECT COUNT(*),
                COUNT(DISTINCT path),
                COALESCE(SUM(CASE WHEN date(timestamp) = date('now', 'localtime') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN timestamp >= datetime('now', 'localtime', '-6 days', 'start of day') THEN 1 ELSE 0 END), 0)
         FROM visitors",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
}

#[get("/statistics")]
fn get_statistics(_auth: BasicAuthGuard, config: &State<AppConfig>) -> HandlerResult<Template> {
    let connection = open_database(config)?;
    let (total, unique_paths, today, week) =
        dashboard_totals(&connection).map_err(database_error)?;
    let mut statement = connection
        .prepare("SELECT COUNT(*) AS count, path FROM visitors GROUP BY path ORDER BY count DESC, path ASC")
        .map_err(database_error)?;
    let entry_rows = statement
        .query_map([], |row| {
            let count: i64 = row.get(0)?;
            let path: String = row.get(1)?;
            let share = if total == 0 {
                0.0
            } else {
                (count as f64 / total as f64) * 100.0
            };
            Ok(PathSummary {
                count,
                path_url: encode_path_segment(&path),
                path,
                share_percent: format!("{share:.1}"),
            })
        })
        .map_err(database_error)?;
    let entries = entry_rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database_error)?;

    Ok(Template::render(
        "statistics/index",
        json!({
            "site_title": &config.site.title,
            "page_title": "Статистика",
            "active_page": "overview",
            "theme_auto": config.theme.auto,
            "total": total,
            "unique_paths": unique_paths,
            "today": today,
            "week": week,
            "entries": entries,
        }),
    ))
}

#[get("/statistics/<path>?<page>")]
fn get_statistics_path(
    _auth: BasicAuthGuard,
    path: &str,
    page: Option<usize>,
    config: &State<AppConfig>,
) -> HandlerResult<Template> {
    let connection = open_database(config)?;
    let all_paths = path == "__all__";
    let (total, unique_ips, today, week) = path_totals(&connection, path, all_paths)?;
    let pagination = Pagination::new(total, page);
    let daily = daily_counts(&connection, path, all_paths)?;
    let max_daily = daily.iter().map(|day| day.count).max().unwrap_or(0);
    let chart_points = daily
        .iter()
        .map(|point| format!("{},{}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(" ");
    let visits = paginated_visits(&connection, path, all_paths, pagination.offset)?;
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
            "site_title": &config.site.title,
            "page_title": if all_paths { "Все посещения" } else { "Статистика пути" },
            "active_page": if all_paths { "visits" } else { "overview" },
            "theme_auto": config.theme.auto,
            "path": path,
            "all_paths": all_paths,
            "total": total,
            "unique_ips": unique_ips,
            "today": today,
            "week": week,
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

fn path_totals(
    connection: &Connection,
    path: &str,
    all_paths: bool,
) -> HandlerResult<(i64, i64, i64, i64)> {
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

    let result = if all_paths {
        connection.query_row(query, [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
    } else {
        connection.query_row(query, [path], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
    };
    result.map_err(database_error)
}

fn daily_counts(
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

    let values: Vec<(String, i64)> = {
        let mut statement = connection.prepare(query).map_err(database_error)?;
        let mapped = if all_paths {
            statement.query_map([], map_row_to_daily_value)
        } else {
            statement.query_map([path], map_row_to_daily_value)
        }
        .map_err(database_error)?;
        mapped
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(database_error)?
    };

    let max_count = values
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(1)
        .max(1);
    let last_index = i64::try_from(values.len().saturating_sub(1))
        .unwrap_or(1)
        .max(1);
    Ok(values
        .into_iter()
        .enumerate()
        .map(|(index, (date, count))| DailyCount {
            date,
            count,
            x: 16 + (i64::try_from(index).unwrap_or_default() * 688 / last_index),
            y: 196 - (count * 164 / max_count),
        })
        .collect())
}

fn map_row_to_daily_value(row: &Row<'_>) -> rusqlite::Result<(String, i64)> {
    Ok((row.get(0)?, row.get(1)?))
}

fn paginated_visits(
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
    let mut statement = connection.prepare(query).map_err(database_error)?;
    let mapped = if all_paths {
        statement.query_map(params![PAGE_SIZE, offset], map_row_to_visit)
    } else {
        statement.query_map(params![path, PAGE_SIZE, offset], map_row_to_visit)
    }
    .map_err(database_error)?;
    mapped
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database_error)
}

#[get("/statistics/recent?<page>")]
fn get_statistics_recent(
    _auth: BasicAuthGuard,
    page: Option<usize>,
    config: &State<AppConfig>,
) -> HandlerResult<Template> {
    let connection = open_database(config)?;
    let (total, unique_paths, today, week) =
        dashboard_totals(&connection).map_err(database_error)?;
    let pagination = Pagination::new(total, page);
    let visits = paginated_visits(&connection, "", true, pagination.offset)?;

    Ok(Template::render(
        "statistics/recent",
        json!({
            "site_title": &config.site.title,
            "page_title": "Последние посещения",
            "active_page": "recent",
            "theme_auto": config.theme.auto,
            "total": total,
            "unique_paths": unique_paths,
            "today": today,
            "week": week,
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
fn get_statistics_self(_auth: BasicAuthGuard) -> Redirect {
    Redirect::to("/statistics")
}

#[get("/statistics_self_full_json")]
fn get_statistics_self_full_json(
    _auth: BasicAuthGuard,
    config: &State<AppConfig>,
) -> HandlerResult<content::RawJson<String>> {
    let connection = open_database(config)?;
    let mut statement = connection
        .prepare("SELECT timestamp, path, ip, json FROM visitors ORDER BY timestamp DESC, id DESC")
        .map_err(database_error)?;
    let mapped = statement
        .query_map([], |row| {
            let raw_metadata: String = row.get(3)?;
            Ok(ExportVisit {
                timestamp: row.get(0)?,
                path: row.get(1)?,
                ip: row.get(2)?,
                metadata: serde_json::from_str(&raw_metadata).unwrap_or_default(),
            })
        })
        .map_err(database_error)?;
    let visitors = mapped
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(database_error)?;
    let body = serde_json::to_string(&visitors).map_err(|error| {
        eprintln!("cannot serialize statistics export: {error}");
        Status::InternalServerError
    })?;
    Ok(content::RawJson(body))
}

fn create_database(config: &AppConfig) -> rusqlite::Result<()> {
    let connection = Connection::open(get_db_path(config))?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS visitors (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             path TEXT NOT NULL CHECK(length(path) BETWEEN 1 AND 255),
             timestamp TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
             ip TEXT NOT NULL,
             json TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_visitors_path_timestamp
             ON visitors(path, timestamp DESC);
         CREATE INDEX IF NOT EXISTS idx_visitors_timestamp
             ON visitors(timestamp DESC);",
    )?;
    Ok(())
}

fn rocket(config: AppConfig) -> Rocket<Build> {
    rocket::build()
        .manage(config)
        .attach(SecurityHeaders)
        .attach(Template::fairing())
        .mount("/assets", FileServer::from(relative!("static")))
        .mount(
            "/",
            routes![
                get_counter,
                get_root,
                get_statistics,
                get_statistics_path,
                get_statistics_recent,
                get_statistics_self,
                get_statistics_self_full_json,
            ],
        )
        .register("/", catchers![unauthorized])
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = load_config()?;
    create_database(&config)?;
    rocket(config)
        .launch()
        .await
        .map_err(|error| Box::new(error) as Box<dyn Error>)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_credentials() -> Basic {
        Basic {
            username: "admin".to_owned(),
            password: "correct horse battery staple".to_owned(),
        }
    }

    #[test]
    fn credentials_match_accepts_valid_basic_credentials() {
        let encoded = general_purpose::STANDARD.encode("admin:correct horse battery staple");
        assert!(credentials_match(
            &format!("Basic {encoded}"),
            &test_credentials()
        ));
    }

    #[test]
    fn credentials_match_rejects_invalid_password() {
        let encoded = general_purpose::STANDARD.encode("admin:wrong password");
        assert!(!credentials_match(
            &format!("Basic {encoded}"),
            &test_credentials()
        ));
    }

    #[test]
    fn anonymize_ip_masks_last_ipv4_octet() {
        assert_eq!(anonymize_ip("192.0.2.42"), "192.0.2.0");
    }

    #[test]
    fn anonymize_ip_masks_ipv6_after_48_bits() {
        assert_eq!(anonymize_ip("2001:db8:abcd:1234::1"), "2001:db8:abcd::");
    }

    #[test]
    fn strip_url_query_removes_sensitive_query_parameters() {
        assert_eq!(
            strip_url_query("https://example.com/page?token=secret"),
            "https://example.com/page"
        );
    }

    #[test]
    fn encode_path_segment_escapes_reserved_characters() {
        assert_eq!(encode_path_segment("docs & help"), "docs%20%26%20help");
    }

    #[test]
    fn pagination_clamps_page_to_last_available_page() {
        assert_eq!(Pagination::new(45, Some(99)).current_page, 3);
    }

    #[test]
    fn validate_counter_path_rejects_oversized_values() {
        assert_eq!(
            validate_counter_path(&"a".repeat(MAX_PATH_BYTES + 1)),
            Err(Status::BadRequest)
        );
    }
}
