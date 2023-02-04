use rusqlite::*;
// use serde_json::*;

use std::str::FromStr;

use rocket::serde::{Deserialize, Serialize};
use rocket::response::content;
use rocket::*;
use rocket::{
    request::{FromRequest, Outcome, self, Request},
    
};
use std::{convert::Infallible};
use rocket::Route;
use rocket::http::Method;

// use crate::request::Request;
use crate::response::{self, Response, Responder};
use crate::http::ContentType;
use std::collections::HashMap;

const DB_HOST: &str = "./wapp_simple_stats_rust.db";

macro_rules! ctrs {
    ($($name:ident: $ct:ident, $name_str:expr, $ct_str:expr),+) => {
        $(
            #[doc="Override the `Content-Type` of the response to <b>"]
            #[doc=$name_str]
            #[doc="</b>, or <i>"]
            #[doc=$ct_str]
            #[doc="</i>."]
            ///
            /// Delegates the remainder of the response to the wrapped responder.
            ///
            /// **Note:** Unlike types like [`Json`](crate::serde::json::Json)
            /// and [`MsgPack`](crate::serde::msgpack::MsgPack), this type _does
            /// not_ serialize data in any way. You should _always_ use those
            /// types to respond with serializable data. Additionally, you
            /// should _always_ use [`NamedFile`](crate::fs::NamedFile), which
            /// automatically sets a `Content-Type`, to respond with file data.
            #[derive(Debug, Clone, PartialEq)]
            pub struct $name<R>(pub R);

            /// Sets the Content-Type of the response then delegates the
            /// remainder of the response to the wrapped responder.
            impl<'r, 'o: 'r, R: Responder<'r, 'o>> Responder<'r, 'o> for $name<R> {
                fn respond_to(self, req: &'r Request<'_>) -> response::Result<'o> {
                    (ContentType::$ct, self.0).respond_to(req)
                }
            }
        )+
    }
}

ctrs! {
    RawSVG: SVG, "SVG", "image/svg"
}

#[macro_use] extern crate rocket;

#[get("/")]
async fn get_root() -> content::RawHtml<String> {
    content::RawHtml(String::from_str("<h1>Auth required</h1>").unwrap())
}

#[derive(Debug)]
enum RequestDataError {
    Missing,
    Invalid,
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
async fn get_counter(o_request_data: RequestData, path: String) -> RawSVG<String> {
    // let s_ip = o_request_data.s_ip;
    let conn = Connection::open(DB_HOST).unwrap();
    let s_json = serde_json::to_string(&o_request_data.v_headers).unwrap();
    // serde_json::to_string()
    conn.execute(
        "INSERT INTO visitors (path, ip, json) VALUES (?, ?, ?)",
        (
            path, 
            o_request_data.s_ip, 
            s_json
        ),
    );

    let i_row_count: i64 = conn.query_row("SELECT COUNT(*) as c FROM visitors ORDER BY timestamp DESC", [], |row| { row.get(0) }).unwrap();
    
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
    RawSVG(s_counter)
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

#[get("/statistics/<path>")]
async fn get_statistics_path(path: String) -> content::RawHtml<String> {
    let conn = Connection::open(DB_HOST).unwrap();

    let mut s_html = r###"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta http-equiv="X-UA-Compatible" content="IE=edge">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Statistics</title>
</head>
<body>
"###.to_string();
    


s_html = s_html + r###"
<h4>Count</h4>
<div class="stat-table">
    <div class="raw-header">
        <div class="cell">Timestamp</div>
        <div class="cell">Count</div>
        <div class="cell">Path</div>
        <div class="cell"></div>
    </div>
"###;

let mut stmt;
let visitors_iter;
if path=="__all__" {
    stmt = conn.prepare("SELECT strftime('%d-%m-%Y',timestamp) AS t, COUNT(*) AS c, path, ip, json FROM visitors GROUP BY strftime('%d-%m-%Y',timestamp) ORDER BY timestamp DESC LIMIT 30").unwrap();
    visitors_iter = stmt.query_map(
        [], 
        fn_row_to_visitor
    ).unwrap();
} else {
    stmt = conn.prepare("SELECT strftime('%d-%m-%Y',timestamp) AS t, COUNT(*) AS c, path, ip, json FROM visitors WHERE path=? GROUP BY strftime('%d-%m-%Y',timestamp) ORDER BY timestamp DESC LIMIT 30").unwrap();
    visitors_iter = stmt.query_map(
        [path.clone()], 
        fn_row_to_visitor
    ).unwrap();
}

for visitor_result in visitors_iter {
    s_html = s_html + r#"<div class="raw">"#;
    let v = visitor_result.unwrap();
    s_html = s_html + r#"<div class="cell">"#+v.timestamp.as_str()+r#"</div>"#;
    s_html = s_html + r#"<div class="cell">"#+v.count.to_string().as_str()+r#"</div>"#;
    s_html = s_html + r#"<div class="cell">"#+v.path.as_str()+r#"</div>"#;
    s_html = s_html + r#"<div class="cell"></div>"#;
    s_html = s_html + "</div>";
}

s_html = s_html + r###"
</div>"###;


    s_html = s_html + r###"
<h4>Last 10</h4>
<div class="stat-table">
    <div class="raw-header">
        <div class="cell">Timestamp</div>
        <div class="cell">Path</div>
        <div class="cell">IP</div>
        <div class="cell">JSON</div>
    </div>
"###;

    let mut stmt_last;
    let visitors_iter_last;
    if path=="__all__" {
        stmt_last = conn.prepare("SELECT strftime('%d-%m-%Y',timestamp) AS t, 1, path, ip, json FROM visitors ORDER BY timestamp DESC LIMIT 10").unwrap();
        visitors_iter_last = stmt_last.query_map(
            [], 
            fn_row_to_visitor
        ).unwrap();
    } else {
        stmt_last = conn.prepare("SELECT strftime('%d-%m-%Y',timestamp) AS t, 1, path, ip, json FROM visitors WHERE path=? ORDER BY timestamp DESC LIMIT 10").unwrap();
        visitors_iter_last = stmt_last.query_map(
            [path.clone()], 
            fn_row_to_visitor
        ).unwrap();
    }

    for visitor_result in visitors_iter_last {
        s_html = s_html + r#"<div class="raw">"#;
        let v = visitor_result.unwrap();
        s_html = s_html + r#"<div class="cell">"#+v.timestamp.as_str()+r#"</div>"#;
        s_html = s_html + r#"<div class="cell">"#+v.path.as_str()+r#"</div>"#;
        s_html = s_html + r#"<div class="cell">"#+v.ip.as_str()+r#"</div>"#;
        s_html = s_html + r#"<div class="cell">"#+v.json.as_str()+"</div>";
        s_html = s_html + "</div>";
    }
    s_html = s_html + r###"
</div>"###;


    s_html = s_html + r###"
<style>
.stat-table { border-bottom: 1px solid rgba(0,0,0,0.1); border-right: 1px solid rgba(0,0,0,0.1); }
.raw-header, .raw {
display: grid;
grid-template-columns: 120px 120px 120px 1fr;
}
.raw-header .cell {
font-weight: bold;
background: #eee;
}
.cell { border-top: 1px solid rgba(0,0,0,0.1); border-left: 1px solid rgba(0,0,0,0.1); }
.cell { padding: 5px; word-break: break-all; }
</style>
</body>
</html>
"###;

return content::RawHtml(s_html);
}

#[get("/statistics")]
async fn get_statistics() -> content::RawHtml<String> {
    let mut s_html = r###"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta http-equiv="X-UA-Compatible" content="IE=edge">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Statistics</title>
</head>
<body>
    <div class="stat-table">
        <div class="raw-header">
            <div class="cell">Path</div>
            <div class="cell">Visitors</div>
        </div>
"###.to_string();

    let conn = Connection::open(DB_HOST).unwrap();

    let mut i_all: i64 = conn.query_row("SELECT COUNT(*) AS c FROM visitors ORDER BY timestamp DESC", [], |o| { o.get(0) }).unwrap();

    let mut stmt = conn.prepare("SELECT COUNT(*) AS c, path FROM visitors GROUP BY path ORDER BY timestamp DESC").unwrap();
    let visitors_iter = stmt.query_map([], |row| {
        Ok(GroupedVisitors {
            count: row.get(0).unwrap(),
            path: row.get(1).unwrap()
        })
    }).unwrap();

    s_html = s_html + r#"<div class="raw">"#;
        s_html = s_html + r#"<div class="cell"><a href="/statistics/__all__">ALL</a></div>"#;
        s_html = s_html + r#"<div class="cell">"#+i_all.to_string().as_str()+"</div>";
    s_html = s_html + "</div>";
    
    for visitor_result in visitors_iter {
        s_html = s_html + r#"<div class="raw">"#;
        let v = visitor_result.unwrap();
        s_html = s_html + r#"<div class="cell"><a href="/statistics/"#+v.path.as_str()+r#"">"#+v.path.as_str()+r#"</a></div>"#;
        s_html = s_html + r#"<div class="cell">"#+v.count.to_string().as_str()+"</div>";
        s_html = s_html + "</div>";
    }

    s_html = s_html + r###"
</div>
<style>
.stat-table { border-bottom: 1px solid rgba(0,0,0,0.1); border-right: 1px solid rgba(0,0,0,0.1); }
.raw-header, .raw {
    display: grid;
    grid-template-columns: 120px 1fr;
}
.raw-header .cell {
    font-weight: bold;
    background: #eee;
}
.cell { border-top: 1px solid rgba(0,0,0,0.1); border-left: 1px solid rgba(0,0,0,0.1); }
.cell { padding: 5px; word-break: break-all; }
</style>
</body>
</html>
"###;

    return content::RawHtml(s_html);
}

#[get("/statistics_self_full_json")]
async fn get_statistics_self_full_json() -> content::RawJson<String> {
    let conn = Connection::open(DB_HOST).unwrap();

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

fn create_database() {
    let conn = Connection::open(DB_HOST).unwrap();

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
    create_database();
    let _rocket = rocket::build()
        .mount(
            "/",
            routes![
                get_counter,
                get_root,
                get_statistics,
                get_statistics_path,
                get_statistics_self,
                get_statistics_self_full_json
            ]
        )
        .launch().await?;
    Ok(())
}