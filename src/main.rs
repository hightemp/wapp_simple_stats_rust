use rusqlite::*;
// use serde_json::*;

use std::str::FromStr;

use rocket::serde::{Deserialize, Serialize};
use rocket::response::content;
use rocket::*;
use rocket::{
    request::{FromRequest, Outcome, self, Request},
};
use rocket::response::{self, Response, Responder};
use rocket::http::{ContentType, Status};
use rocket::State;
use rocket_dyn_templates::Template;
// use crate::request::Request;
use std::collections::HashMap;
use std::fs;
use base64::{engine::general_purpose, Engine as _};

use serde_json::json;
const DEFAULT_DB_PATH: &str = "./wapp_simple_stats_rust.db";
const PAGE_SIZE: i64 = 10;

// ----------------------------------------------------------------------------
// App Config (loaded from config.yaml)
// ----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Site {
    title: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Theme {
    auto: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Database {
    path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Basic {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Auth {
    enabled: bool,
    basic: Basic,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppConfig {
    site: Site,
    theme: Theme,
    auth: Auth,
    database: Option<Database>,
}

fn load_config() -> AppConfig {
    let path = "config.yaml";
    if let Ok(s) = fs::read_to_string(path) {
        if let Ok(cfg) = serde_yaml::from_str::<AppConfig>(&s) {
            return cfg;
        }
    }
    // Defaults if no config.yaml
    AppConfig {
        site: Site {
            title: "Simple Stats".to_string(),
        },
        theme: Theme { auto: true },
        auth: Auth {
            enabled: false,
            basic: Basic {
                username: "admin".to_string(),
                password: "password".to_string(),
            },
        },
        database: Some(Database {
            path: DEFAULT_DB_PATH.to_string(),
        }),
    }
}

fn get_db_path(cfg: &AppConfig) -> &str {
    if let Some(db) = &cfg.database {
        db.path.as_str()
    } else {
        DEFAULT_DB_PATH
    }
}

// ----------------------------------------------------------------------------
// Basic Auth Guard and 401 catcher
// ----------------------------------------------------------------------------

pub struct BasicAuthGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BasicAuthGuard {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let cfg = req.rocket().state::<AppConfig>();
        if let Some(cfg) = cfg {
            if !cfg.auth.enabled {
                return Outcome::Success(BasicAuthGuard);
            }

            if let Some(header) = req.headers().get_one("Authorization") {
                let prefix = "Basic ";
                if header.starts_with(prefix) {
                    let b64 = &header[prefix.len()..];
                    if let Ok(bytes) = general_purpose::STANDARD.decode(b64) {
                        if let Ok(creds) = String::from_utf8(bytes) {
                            let mut it = creds.splitn(2, ':');
                            let u = it.next().unwrap_or("");
                            let p = it.next().unwrap_or("");
                            if u == cfg.auth.basic.username && p == cfg.auth.basic.password {
                                return Outcome::Success(BasicAuthGuard);
                            }
                        }
                    }
                }
            }
            return Outcome::Failure((Status::Unauthorized, ()));
        }
        // If no config stored, allow by default
        Outcome::Success(BasicAuthGuard)
    }
}

struct Unauthorized;

impl<'r> Responder<'r, 'static> for Unauthorized {
    fn respond_to(self, _: &Request<'_>) -> response::Result<'static> {
        Response::build()
            .status(Status::Unauthorized)
            .raw_header("WWW-Authenticate", "Basic realm=\"Restricted\"")
            .sized_body(0, std::io::Cursor::new(String::new()))
            .ok()
    }
}

#[catch(401)]
fn unauthorized() -> Unauthorized {
    Unauthorized
}

#[macro_use] extern crate rocket;

#[get("/")]
async fn get_root(cfg: &State<AppConfig>) -> Template {
    let ctx = json!({
        "site_title": cfg.site.title,
        "page_title": "Welcome",
    });
    Template::render("landing", &ctx)
}

#[derive(Debug)]
enum RequestDataError {
    Missing,
    Invalid,
}

use std::io::Cursor;

struct Wrapper {
    value: String
}

impl<'a> Responder<'a, 'a> for Wrapper {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        Response::build()
            .raw_header("Cache-Control", "max-age=0, no-cache, no-store, must-revalidate")
            .raw_header("Content-Type", "image/svg+xml; charset=utf-8")
            .sized_body(self.value.len(), Cursor::new(self.value))
            .ok()
    }
}

pub mod vectorize {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::iter::FromIterator;

    pub fn serialize<'a, T, K, V, S>(target: T, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: IntoIterator<Item = (&'a K, &'a V)>,
        K: Serialize + 'a,
        V: Serialize + 'a,
    {
        let container: Vec<_> = target.into_iter().collect();
        serde::Serialize::serialize(&container, ser)
    }

    pub fn deserialize<'de, T, K, V, D>(des: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromIterator<(K, V)>,
        K: Deserialize<'de>,
        V: Deserialize<'de>,
    {
        let container: Vec<_> = serde::Deserialize::deserialize(des)?;
        Ok(T::from_iter(container.into_iter()))
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct RequestData {
    s_ip: String,
    #[serde(with = "vectorize")]
    v_headers: HashMap<String, String>
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequestData {
    type Error = RequestDataError;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let mut o_request_data = RequestData { s_ip: String::from(""), v_headers: HashMap::new() };

        o_request_data.s_ip = req.remote().unwrap().to_string();        
        let v_ip: Vec<&str> = o_request_data.s_ip.split(":").collect();
        o_request_data.s_ip = v_ip[0].to_string();
        for h in req.headers().iter() {
            println!("HEADER: {} {}", h.name, h.value);
            let s_h = String::from(h.name.as_str());
            let s_v = String::from(h.value);
            o_request_data.v_headers.insert(s_h, s_v);
        }
        if (o_request_data.v_headers.contains_key("x-real-ip")) {
            o_request_data.s_ip = o_request_data.v_headers.get("x-real-ip").unwrap().clone();
        }

        Outcome::Success(o_request_data)
    }
}

#[get("/counter/<path>")]
async fn get_counter(o_request_data: RequestData, path: String, cfg: &State<AppConfig>) -> Wrapper {
    // let s_ip = o_request_data.s_ip;
    let conn = Connection::open(get_db_path(&cfg)).unwrap();
    let s_json = serde_json::to_string(&o_request_data.v_headers).unwrap();
    // serde_json::to_string()
    conn.execute(
        "INSERT INTO visitors (path, ip, json) VALUES (?, ?, ?)",
        (
            path.clone(), 
            o_request_data.s_ip, 
            s_json
        ),
    );

    let i_row_count: i64 = conn.query_row("SELECT COUNT(*) as c FROM visitors WHERE path = ? ORDER BY timestamp DESC", [path.clone()], |row| { row.get(0) }).unwrap();
    
    let s_temp = i_row_count.to_string();
    let s_count = s_temp.as_str();
    let s_format = "0".repeat(6-s_count.len());
    let s_counter_fromated = s_format+s_count;

    let s_counter = r###"
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="110" height="20" role="img" aria-label="statistics: {NUMBER}">
    <title>statistics: {NUMBER}</title>
    <linearGradient id="s" x2="0" y2="100%">
        <stop offset="0" stop-color="#bbb" stop-opacity=".1" />
        <stop offset="1" stop-opacity=".1" />
    </linearGradient>
    <clipPath id="r">
        <rect width="110" height="20" rx="3" fill="#fff" />
    </clipPath>
    <g clip-path="url(#r)">
        <rect width="59" height="20" fill="#555" />
        <rect x="59" width="51" height="20" fill="#a4a61d" />
        <rect width="110" height="20" fill="url(#s)" />
    </g>
    <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">
        <text aria-hidden="true" x="305" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="490">statistics</text>
        <text x="305" y="140" transform="scale(.1)" fill="#fff" textLength="490">statistics</text>
        <text aria-hidden="true" x="835" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="410">{NUMBER}</text>
        <text x="835" y="140" transform="scale(.1)" fill="#fff" textLength="410">{NUMBER}</text>
    </g>
</svg>
"###.replace("{NUMBER}", s_counter_fromated.as_str());

    let oW = Wrapper { value: s_counter };

    return oW;
}


#[get("/statistics_self")]
async fn get_statistics_self() -> content::RawHtml<String> {
    content::RawHtml(String::from_str("").unwrap())
}

#[derive(Serialize, Deserialize, Debug)]
struct Visitors {
    timestamp: String,
    path: String,
    ip: String,
    json: String
}

#[derive(Serialize, Deserialize, Debug)]
struct VisitorsGroupedByTime {
    timestamp: String,
    count: i64,
    path: String,
    ip: String,
    json: String
}

#[derive(Serialize, Deserialize, Debug)]
struct GroupedVisitors {
    count: i64,
    path: String
}

fn fn_row_to_visitor(row: &Row) -> Result<VisitorsGroupedByTime> {
    Ok(VisitorsGroupedByTime {
        timestamp: row.get(0).unwrap(),
        count: row.get(1).unwrap(),
        path: row.get(2).unwrap(),
        ip: row.get(3).unwrap(),
        json: row.get(4).unwrap(),
    })
}

#[get("/statistics/<path>?<page>")]
async fn get_statistics_path(
    _auth: BasicAuthGuard,
    path: String,
    page: Option<usize>,
    cfg: &State<AppConfig>
) -> Template {
    let conn = Connection::open(get_db_path(&cfg)).unwrap();

    // Daily aggregation (last 30 days), optionally filtered by path
    let mut rows: Vec<VisitorsGroupedByTime> = Vec::new();
    if path == "__all__" {
        let mut stmt = conn.prepare(
            "SELECT strftime('%Y-%m-%d',timestamp) AS t, COUNT(*) AS c, path, ip, json
             FROM visitors
             GROUP BY strftime('%Y-%m-%d',timestamp)
             ORDER BY timestamp DESC
             LIMIT 30"
        ).unwrap();
        let iter = stmt.query_map([], fn_row_to_visitor).unwrap();
        for r in iter { rows.push(r.unwrap()); }
    } else {
        let mut stmt = conn.prepare(
            "SELECT strftime('%Y-%m-%d',timestamp) AS t, COUNT(*) AS c, path, ip, json
             FROM visitors
             WHERE path=?
             GROUP BY strftime('%Y-%m-%d',timestamp)
             ORDER BY timestamp DESC
             LIMIT 30"
        ).unwrap();
        let iter = stmt.query_map([path.clone()], fn_row_to_visitor).unwrap();
        for r in iter { rows.push(r.unwrap()); }
    }

    // Reverse to chronological order for chart X axis
    rows.reverse();
    let labels: Vec<String> = rows.iter().map(|r| r.timestamp.clone()).collect();
    let counts: Vec<i64> = rows.iter().map(|r| r.count).collect();

    // Pagination
    let current_page: i64 = page.unwrap_or(1).max(1) as i64;
    let offset: i64 = (current_page - 1) * PAGE_SIZE;

    let total_rows: i64 = if path == "__all__" {
        conn.query_row("SELECT COUNT(*) FROM visitors", [], |o| o.get(0)).unwrap()
    } else {
        conn.query_row("SELECT COUNT(*) FROM visitors WHERE path = ?", [path.clone()], |o| o.get(0)).unwrap()
    };
    let total_pages: i64 = if total_rows == 0 { 1 } else { ((total_rows - 1) / PAGE_SIZE) + 1 };
    let has_prev = current_page > 1;
    let has_next = current_page < total_pages;
    let prev_page = if has_prev { Some(current_page - 1) } else { None };
    let next_page = if has_next { Some(current_page + 1) } else { None };

    // Paginated visits list (10 per page)
    let mut last: Vec<VisitorsGroupedByTime> = Vec::new();
    if path == "__all__" {
        let mut stmt_last = conn.prepare(
            "SELECT strftime('%Y-%m-%d %H:%M:%S',timestamp) AS t, 1, path, ip, json
             FROM visitors
             ORDER BY timestamp DESC
             LIMIT ? OFFSET ?"
        ).unwrap();
        let iter_last = stmt_last.query_map((PAGE_SIZE, offset), fn_row_to_visitor).unwrap();
        for r in iter_last { last.push(r.unwrap()); }
    } else {
        let mut stmt_last = conn.prepare(
            "SELECT strftime('%Y-%m-%d %H:%M:%S',timestamp) AS t, 1, path, ip, json
             FROM visitors
             WHERE path=?
             ORDER BY timestamp DESC
             LIMIT ? OFFSET ?"
        ).unwrap();
        let iter_last = stmt_last.query_map((path.clone(), PAGE_SIZE, offset), fn_row_to_visitor).unwrap();
        for r in iter_last { last.push(r.unwrap()); }
    }

    let ctx = json!({
        "site_title": cfg.site.title,
        "page_title": "Statistics",
        "path": path,
        "labels": labels,
        "counts": counts,
        "last": last,
        "current_page": current_page,
        "total_pages": total_pages,
        "has_prev": has_prev,
        "has_next": has_next,
        "prev_page": prev_page,
        "next_page": next_page
    });

    Template::render("statistics/path", &ctx)
}

#[get("/statistics/recent")]
async fn get_statistics_recent(_auth: BasicAuthGuard, cfg: &State<AppConfig>) -> Template {
    let conn = Connection::open(get_db_path(&cfg)).unwrap();

    let mut items: Vec<Visitors> = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m-%d %H:%M:%S',timestamp) AS t, path, ip, json
         FROM visitors
         ORDER BY timestamp DESC
         LIMIT 20"
    ).unwrap();
    let iter = stmt.query_map([], |row| {
        Ok(Visitors {
            timestamp: row.get(0).unwrap(),
            path: row.get(1).unwrap(),
            ip: row.get(2).unwrap(),
            json: row.get(3).unwrap(),
        })
    }).unwrap();
    for r in iter { items.push(r.unwrap()); }

    let ctx = json!({
        "site_title": cfg.site.title,
        "page_title": "Recent visits",
        "items": items
    });

    Template::render("statistics/recent", &ctx)
}

#[get("/statistics")]
async fn get_statistics(_auth: BasicAuthGuard, cfg: &State<AppConfig>) -> Template {
    let conn = Connection::open(get_db_path(&cfg)).unwrap();

    let total: i64 = conn.query_row(
        "SELECT COUNT(*) AS c FROM visitors",
        [],
        |o| o.get(0)
    ).unwrap();

    let mut entries: Vec<GroupedVisitors> = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT COUNT(*) AS c, path FROM visitors GROUP BY path ORDER BY c DESC"
    ).unwrap();
    let iter = stmt.query_map([], |row| {
        Ok(GroupedVisitors {
            count: row.get(0).unwrap(),
            path: row.get(1).unwrap()
        })
    }).unwrap();
    for r in iter {
        entries.push(r.unwrap());
    }

    let ctx = json!({
        "site_title": cfg.site.title,
        "page_title": "Statistics",
        "total": total,
        "entries": entries
    });

    Template::render("statistics/index", &ctx)
}

#[get("/statistics_self_full_json")]
async fn get_statistics_self_full_json(cfg: &State<AppConfig>) -> content::RawJson<String> {
    let conn = Connection::open(get_db_path(&cfg)).unwrap();

    let mut stmt = conn.prepare("SELECT timestamp, path, ip, json FROM visitors ORDER BY timestamp DESC").unwrap();
    let visitors_iter = stmt.query_map([], |row| {
        Ok(Visitors {
            timestamp: row.get(0).unwrap(),
            path: row.get(1).unwrap(),
            ip: row.get(2).unwrap(),
            json: row.get(3).unwrap(),
        })
    }).unwrap();

    let mut visitors = vec![];
    for visitor_result in visitors_iter {
        visitors.push(visitor_result.unwrap());
    }
    let s_json = serde_json::to_string(&visitors).unwrap();
    return content::RawJson(s_json);
}

fn create_database(cfg: &AppConfig) {
    let conn = Connection::open(get_db_path(cfg)).unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS visitors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path VARCHAR(255) NOT NULL,
            timestamp DATE DEFAULT (datetime('now','localtime')),
            ip VARCHAR(50) NOT NULL,
            json VARCHAR(4000) NOT NULL
        )",
        (), // empty list of parameters.
    ).unwrap();
}

#[rocket::main]
pub async fn main() -> Result<(), rocket::Error> {
    let cfg = load_config();
    create_database(&cfg);
    let _rocket = rocket::build()
        .manage(cfg)
        .attach(Template::fairing())
        .mount(
            "/",
            routes![
                get_counter,
                get_root,
                get_statistics,
                get_statistics_path,
                get_statistics_recent,
                get_statistics_self,
                get_statistics_self_full_json
            ]
        )
        .register("/", catchers![unauthorized])
        .launch().await?;
    Ok(())
}