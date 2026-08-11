use std::collections::BTreeMap;

use rocket::request::{FromRequest, Outcome, Request};

use crate::utils::{strip_url_query, truncate_chars};

#[derive(Debug)]
pub(crate) struct RequestMetadata {
    pub(crate) ip: String,
    pub(crate) headers: BTreeMap<String, String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequestMetadata {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let ip = request
            .client_ip()
            .map_or_else(|| "unknown".to_owned(), |address| address.to_string());

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

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::http::Header;
    use rocket::local::blocking::Client;

    #[rocket::get("/metadata-ip")]
    fn metadata_ip(metadata: RequestMetadata) -> String {
        metadata.ip
    }

    fn test_client() -> Client {
        Client::tracked(rocket::build().mount("/", rocket::routes![metadata_ip]))
            .expect("valid Rocket test client")
    }

    #[test]
    fn request_metadata_preserves_client_ipv4_address() {
        let client = test_client();
        let response = client
            .get("/metadata-ip")
            .header(Header::new("X-Real-IP", "192.0.2.42"))
            .dispatch();

        assert_eq!(response.into_string().as_deref(), Some("192.0.2.42"));
    }

    #[test]
    fn request_metadata_preserves_client_ipv6_address() {
        let client = test_client();
        let response = client
            .get("/metadata-ip")
            .header(Header::new("X-Real-IP", "2001:db8:abcd:1234::1"))
            .dispatch();

        assert_eq!(
            response.into_string().as_deref(),
            Some("2001:db8:abcd:1234::1")
        );
    }
}
