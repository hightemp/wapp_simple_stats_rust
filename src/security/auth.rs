use std::io::Cursor;

use base64::{engine::general_purpose, Engine as _};
use rocket::catch;
use rocket::http::{Header, Status};
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::{self, Responder, Response};
use subtle::ConstantTimeEq;

use crate::config::{AppConfig, BasicCredentials};

const MAX_AUTH_HEADER_BYTES: usize = 8 * 1024;

pub(crate) struct BasicAuthGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BasicAuthGuard {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let Some(config) = request.rocket().state::<AppConfig>() else {
            return Outcome::Error((Status::InternalServerError, ()));
        };

        if !config.auth_enabled() {
            return Outcome::Success(Self);
        }

        let authenticated = request
            .headers()
            .get_one("Authorization")
            .is_some_and(|header| credentials_match(header, config.credentials()));

        if authenticated {
            Outcome::Success(Self)
        } else {
            Outcome::Error((Status::Unauthorized, ()))
        }
    }
}

fn credentials_match(header: &str, expected: &BasicCredentials) -> bool {
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

pub(crate) struct Unauthorized;

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
pub(crate) fn unauthorized() -> Unauthorized {
    Unauthorized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_credentials() -> BasicCredentials {
        BasicCredentials {
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
}
