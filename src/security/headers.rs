use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::request::Request;
use rocket::response::Response;

pub(crate) struct SecurityHeaders;

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
