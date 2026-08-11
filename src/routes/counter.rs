use std::io::Cursor;

use rocket::http::{Header, Status};
use rocket::response::{self, Responder, Response};
use rocket::{get, Request, State};
use rusqlite::params;

use crate::config::AppConfig;
use crate::database;
use crate::security::RequestMetadata;
use crate::HandlerResult;

const MAX_PATH_BYTES: usize = 255;
const SVG_TEMPLATE: &str = r###"<svg xmlns="http://www.w3.org/2000/svg" width="110" height="20" role="img" aria-label="statistics: {NUMBER}">
<title>statistics: {NUMBER}</title><linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient><clipPath id="r"><rect width="110" height="20" rx="3" fill="#fff"/></clipPath><g clip-path="url(#r)"><rect width="59" height="20" fill="#334155"/><rect x="59" width="51" height="20" fill="#4f46e5"/><rect width="110" height="20" fill="url(#s)"/></g><g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110"><text aria-hidden="true" x="305" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="490">statistics</text><text x="305" y="140" transform="scale(.1)" textLength="490">statistics</text><text aria-hidden="true" x="835" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="410">{NUMBER}</text><text x="835" y="140" transform="scale(.1)" textLength="410">{NUMBER}</text></g></svg>"###;

pub(crate) struct SvgCounter {
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

#[get("/counter/<path>")]
pub(crate) fn get_counter(
    request_metadata: RequestMetadata,
    path: &str,
    config: &State<AppConfig>,
) -> HandlerResult<SvgCounter> {
    validate_path(path)?;
    let connection = database::open(config)?;
    let headers_json = serde_json::to_string(&request_metadata.headers).map_err(|error| {
        eprintln!("cannot serialize request metadata: {error}");
        Status::InternalServerError
    })?;
    connection
        .execute(
            "INSERT INTO visitors (path, ip, json) VALUES (?1, ?2, ?3)",
            params![path, request_metadata.ip, headers_json],
        )
        .map_err(database::map_error)?;

    let total_for_path: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM visitors WHERE path = ?1",
            [path],
            |row| row.get(0),
        )
        .map_err(database::map_error)?;

    let counter_formatted = format!("{total_for_path:0>6}");
    Ok(SvgCounter {
        value: SVG_TEMPLATE.replace("{NUMBER}", &counter_formatted),
    })
}

fn validate_path(path: &str) -> Result<(), Status> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_path_rejects_oversized_values() {
        assert_eq!(
            validate_path(&"a".repeat(MAX_PATH_BYTES + 1)),
            Err(Status::BadRequest)
        );
    }
}
