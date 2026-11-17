//! Trusted-source admission for public preview leases.
//!
//! A deployment may name source networks (`VK_PREVIEW_TRUSTED_SOURCE_CIDRS`)
//! whose requests open a live lease without the capability cookie. The client
//! address is a claim, so it is accepted only from the edge this origin sits
//! behind: Cloudflare's `CF-Connecting-IP`, on a connection from the tunnel
//! connector on loopback. Anything else is untrusted and falls back to the
//! capability flow.

use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    sync::{LazyLock, Mutex},
};

use axum::http::HeaderMap;
use ipnet::IpNet;
use uuid::Uuid;

pub const TRUSTED_SOURCE_CIDRS_ENV: &str = "VK_PREVIEW_TRUSTED_SOURCE_CIDRS";

const CLIENT_IP_HEADER: &str = "cf-connecting-ip";

/// A trusted source is one site, never a provider-sized range. Shorter
/// prefixes are refused so a typo such as `/0` cannot open every lease.
const MIN_IPV4_PREFIX: u8 = 24;
const MIN_IPV6_PREFIX: u8 = 48;

const MAX_LOGGED_ADMISSIONS: usize = 1024;

static CONFIGURED_CIDRS: LazyLock<Vec<IpNet>> = LazyLock::new(|| {
    std::env::var(TRUSTED_SOURCE_CIDRS_ENV)
        .map(|value| parse_trusted_cidrs(&value))
        .unwrap_or_default()
});

static LOGGED_ADMISSIONS: LazyLock<Mutex<HashSet<(Uuid, IpAddr)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// The deployment's trusted networks, read from the environment once.
pub fn configured_cidrs() -> &'static [IpNet] {
    &CONFIGURED_CIDRS
}

/// Parses a comma-separated list of CIDRs or bare addresses. Entries that do
/// not parse or are too broad are logged and skipped, never widened.
pub fn parse_trusted_cidrs(value: &str) -> Vec<IpNet> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let network = entry
                .parse::<IpNet>()
                .ok()
                .or_else(|| entry.parse::<IpAddr>().ok().map(IpNet::from));
            let Some(network) = network else {
                tracing::error!(
                    entry,
                    "{TRUSTED_SOURCE_CIDRS_ENV}: ignoring an entry that is not a CIDR or IP address"
                );
                return None;
            };
            let min_prefix = match network {
                IpNet::V4(_) => MIN_IPV4_PREFIX,
                IpNet::V6(_) => MIN_IPV6_PREFIX,
            };
            if network.prefix_len() < min_prefix {
                tracing::error!(
                    entry,
                    min_prefix,
                    "{TRUSTED_SOURCE_CIDRS_ENV}: ignoring a range broader than one site"
                );
                return None;
            }
            Some(network.trunc())
        })
        .collect()
}

/// The client address, when it can be believed: the peer is the tunnel
/// connector on loopback and Cloudflare supplied exactly one address.
/// `X-Forwarded-For` and similar client-controlled headers are never read.
pub fn trusted_client_ip(peer: Option<SocketAddr>, headers: &HeaderMap) -> Option<IpAddr> {
    if !peer?.ip().to_canonical().is_loopback() {
        return None;
    }
    let mut values = headers.get_all(CLIENT_IP_HEADER).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let address = value.to_str().ok()?.trim().parse::<IpAddr>().ok()?;
    Some(address.to_canonical())
}

/// The client address when the request comes from a trusted network.
pub fn trusted_source(
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
    cidrs: &[IpNet],
) -> Option<IpAddr> {
    if cidrs.is_empty() {
        return None;
    }
    let address = trusted_client_ip(peer, headers)?;
    cidrs
        .iter()
        .any(|network| network.contains(&address))
        .then_some(address)
}

/// Records a trusted admission. The first request for a lease from an address
/// is logged at info; later ones at debug, since one page load is many requests.
pub fn note_admission(lease_id: Uuid, source: IpAddr) {
    let first = {
        let mut seen = LOGGED_ADMISSIONS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if seen.len() >= MAX_LOGGED_ADMISSIONS {
            seen.clear();
        }
        seen.insert((lease_id, source))
    };
    if first {
        tracing::info!(%lease_id, %source, "preview lease admitted from trusted source");
    } else {
        tracing::debug!(%lease_id, %source, "preview lease admitted from trusted source");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAMERO: &str = "107.193.224.69";
    const SCOTT: &str = "70.235.241.95";

    fn loopback() -> Option<SocketAddr> {
        Some("127.0.0.1:51234".parse().unwrap())
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, value.parse().unwrap());
        }
        headers
    }

    fn camero_only() -> Vec<IpNet> {
        parse_trusted_cidrs("107.193.224.69/32")
    }

    #[test]
    fn parses_cidrs_and_bare_addresses() {
        let parsed = parse_trusted_cidrs(" 107.193.224.69/32 , 198.51.100.7,2001:db8:1::/48,");
        assert_eq!(
            parsed,
            vec![
                "107.193.224.69/32".parse::<IpNet>().unwrap(),
                "198.51.100.7/32".parse().unwrap(),
                "2001:db8:1::/48".parse().unwrap(),
            ]
        );
        assert!(parse_trusted_cidrs("").is_empty());
        assert!(parse_trusted_cidrs(" , ").is_empty());
    }

    #[test]
    fn skips_invalid_and_overbroad_entries() {
        let parsed = parse_trusted_cidrs(
            "0.0.0.0/0, ::/0, 107.193.0.0/16, 2001:db8::/32, camero, 107.193.224.69/33, 107.193.224.0/24",
        );
        assert_eq!(parsed, vec!["107.193.224.0/24".parse::<IpNet>().unwrap()]);
    }

    #[test]
    fn host_bits_are_normalised() {
        assert_eq!(
            parse_trusted_cidrs("107.193.224.69/24"),
            vec!["107.193.224.0/24".parse::<IpNet>().unwrap()]
        );
    }

    #[test]
    fn tunnel_request_from_trusted_network_is_trusted() {
        let headers = headers(&[("cf-connecting-ip", CAMERO)]);
        assert_eq!(
            trusted_source(loopback(), &headers, &camero_only()),
            Some(CAMERO.parse().unwrap())
        );
        let v6_loopback = Some("[::1]:40000".parse().unwrap());
        assert!(trusted_source(v6_loopback, &headers, &camero_only()).is_some());
        let mapped_loopback = Some("[::ffff:127.0.0.1]:40000".parse().unwrap());
        assert!(trusted_source(mapped_loopback, &headers, &camero_only()).is_some());
    }

    #[test]
    fn tunnel_request_from_another_network_is_untrusted() {
        let headers = headers(&[("cf-connecting-ip", SCOTT)]);
        assert_eq!(trusted_source(loopback(), &headers, &camero_only()), None);
    }

    #[test]
    fn header_from_a_non_tunnel_peer_is_ignored() {
        let headers = headers(&[("cf-connecting-ip", CAMERO)]);
        for peer in [
            "172.16.100.3:40000",
            "107.193.224.69:40000",
            "[2001:db8::1]:40000",
        ] {
            assert_eq!(
                trusted_source(Some(peer.parse().unwrap()), &headers, &camero_only()),
                None,
                "{peer}"
            );
        }
        assert_eq!(trusted_source(None, &headers, &camero_only()), None);
    }

    #[test]
    fn forwarded_headers_are_never_read() {
        let headers = headers(&[
            ("x-forwarded-for", CAMERO),
            ("x-real-ip", CAMERO),
            ("true-client-ip", CAMERO),
            ("forwarded", "for=107.193.224.69"),
        ]);
        assert_eq!(trusted_source(loopback(), &headers, &camero_only()), None);

        let with_untrusted_cf =
            self::headers(&[("x-forwarded-for", CAMERO), ("cf-connecting-ip", SCOTT)]);
        assert_eq!(
            trusted_source(loopback(), &with_untrusted_cf, &camero_only()),
            None
        );
    }

    #[test]
    fn ambiguous_or_malformed_client_address_is_untrusted() {
        let repeated = headers(&[("cf-connecting-ip", CAMERO), ("cf-connecting-ip", SCOTT)]);
        assert_eq!(trusted_source(loopback(), &repeated, &camero_only()), None);
        let list = headers(&[("cf-connecting-ip", "107.193.224.69, 70.235.241.95")]);
        assert_eq!(trusted_source(loopback(), &list, &camero_only()), None);
        let garbage = headers(&[("cf-connecting-ip", "camero")]);
        assert_eq!(trusted_source(loopback(), &garbage, &camero_only()), None);
    }

    #[test]
    fn ipv4_mapped_client_address_matches_ipv4_network() {
        let headers = headers(&[("cf-connecting-ip", "::ffff:107.193.224.69")]);
        assert_eq!(
            trusted_source(loopback(), &headers, &camero_only()),
            Some(CAMERO.parse().unwrap())
        );
    }

    #[test]
    fn empty_configuration_trusts_nothing() {
        let headers = headers(&[("cf-connecting-ip", CAMERO)]);
        assert_eq!(trusted_source(loopback(), &headers, &[]), None);
    }
}
