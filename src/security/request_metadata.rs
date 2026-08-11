use std::collections::BTreeMap;

use rocket::http::HeaderMap;
use rocket::request::{FromRequest, Outcome, Request};

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

        let headers = collect_headers(request.headers());

        Outcome::Success(Self { ip, headers })
    }
}

fn collect_headers(source: &HeaderMap<'_>) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::<String, String>::new();
    for header in source.iter() {
        let name = header.name().as_str().to_ascii_lowercase();
        headers
            .entry(name)
            .and_modify(|value| {
                value.push('\n');
                value.push_str(header.value());
            })
            .or_insert_with(|| header.value().to_owned());
    }

    headers
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

    #[rocket::get("/metadata-headers")]
    fn metadata_headers(metadata: RequestMetadata) -> String {
        serde_json::to_string(&metadata.headers).expect("request headers should serialize")
    }

    fn test_client() -> Client {
        Client::tracked(rocket::build().mount("/", rocket::routes![metadata_ip, metadata_headers]))
            .expect("valid Rocket test client")
    }

    fn captured_headers(header: Header<'static>) -> BTreeMap<String, String> {
        let client = test_client();
        let response = client.get("/metadata-headers").header(header).dispatch();
        let body = response.into_string().expect("metadata response body");
        serde_json::from_str(&body).expect("valid metadata JSON")
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

    #[test]
    fn request_metadata_collects_arbitrary_headers() {
        let headers = captured_headers(Header::new("X-Custom-Metadata", "custom value"));

        assert_eq!(
            headers.get("x-custom-metadata").map(String::as_str),
            Some("custom value")
        );
    }

    #[test]
    fn request_metadata_preserves_authorization_value() {
        let headers = captured_headers(Header::new("Authorization", "Bearer secret-token"));

        assert_eq!(
            headers.get("authorization").map(String::as_str),
            Some("Bearer secret-token")
        );
    }

    #[test]
    fn request_metadata_preserves_cookie_value() {
        let headers = captured_headers(Header::new("Cookie", "session=secret-cookie"));

        assert_eq!(
            headers.get("cookie").map(String::as_str),
            Some("session=secret-cookie")
        );
    }

    #[test]
    fn request_metadata_preserves_complete_referer_value() {
        let referer = "https://example.test/page?token=secret#section";
        let headers = captured_headers(Header::new("Referer", referer));

        assert_eq!(headers.get("referer").map(String::as_str), Some(referer));
    }

    #[test]
    fn request_metadata_preserves_repeated_header_values() {
        let mut source = HeaderMap::new();
        source.add(Header::new("X-Repeated", "first"));
        source.add(Header::new("X-Repeated", "second"));

        assert_eq!(
            collect_headers(&source)
                .get("x-repeated")
                .map(String::as_str),
            Some("first\nsecond")
        );
    }
}
