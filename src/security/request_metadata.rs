use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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

pub(crate) fn anonymize_ip(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymize_ip_masks_last_ipv4_octet() {
        assert_eq!(anonymize_ip("192.0.2.42"), "192.0.2.0");
    }

    #[test]
    fn anonymize_ip_masks_ipv6_after_48_bits() {
        assert_eq!(anonymize_ip("2001:db8:abcd:1234::1"), "2001:db8:abcd::");
    }
}
