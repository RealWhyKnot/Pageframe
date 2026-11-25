use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const DEVTOOLS_ORIGIN: &str = "devtools://devtools";

fn valid_port(port: &str) -> bool {
    !port.is_empty() && port.len() <= 5 && port.bytes().all(|byte| byte.is_ascii_digit())
}

fn split_host(host: &str) -> Option<(&str, Option<&str>)> {
    if let Some(rest) = host.strip_prefix('[') {
        let (address, tail) = rest.split_once(']')?;
        address.parse::<Ipv6Addr>().ok()?;
        let port = match tail {
            "" => None,
            tail => Some(tail.strip_prefix(':')?),
        };
        return Some((address, port));
    }
    match host.rsplit_once(':') {
        Some((name, port)) => Some((name, Some(port))),
        None => Some((host, None)),
    }
}

pub(crate) fn host_allowed(host: Option<&str>) -> bool {
    let Some(host) = host.map(str::trim).filter(|host| !host.is_empty()) else {
        return false;
    };
    let Some((name, port)) = split_host(host) else {
        return false;
    };
    if port.is_some_and(|port| !valid_port(port)) {
        return false;
    }
    name.eq_ignore_ascii_case("localhost")
        || name.parse::<IpAddr>().is_ok_and(|address| {
            address == IpAddr::V4(Ipv4Addr::LOCALHOST) || address == IpAddr::V6(Ipv6Addr::LOCALHOST)
        })
}

pub(crate) fn origin_allowed(origin: Option<&str>, host: Option<&str>) -> bool {
    let Some(origin) = origin.map(str::trim) else {
        return true;
    };
    if origin.eq_ignore_ascii_case(DEVTOOLS_ORIGIN) {
        return true;
    }
    let Some(host) = host.map(str::trim).filter(|host| host_allowed(Some(host))) else {
        return false;
    };
    ["http://", "https://"].iter().any(|scheme| {
        origin
            .get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
            && origin[scheme.len()..].eq_ignore_ascii_case(host)
    })
}

#[cfg(test)]
mod tests {
    use super::{host_allowed, origin_allowed};

    #[test]
    fn loopback_hosts_are_allowed() {
        for host in [
            "localhost",
            "LOCALHOST:9222",
            "127.0.0.1",
            "127.0.0.1:9222",
            "[::1]",
            "[::1]:9222",
            "[0:0:0:0:0:0:0:1]:9222",
        ] {
            assert!(host_allowed(Some(host)), "{host}");
        }
    }

    #[test]
    fn named_hosts_are_refused() {
        for host in [
            "",
            "evil.example",
            "evil.example:9222",
            "localhost.evil.example",
            "127.0.0.1.nip.io:9222",
            "localhost:",
            "localhost:92a2",
            "localhost:999999",
            "[::1",
            "[::1]x",
            "[evil]:9222",
            "::1",
        ] {
            assert!(!host_allowed(Some(host)), "{host}");
        }
        assert!(!host_allowed(None));
    }

    #[test]
    fn non_loopback_addresses_are_refused() {
        for host in [
            "192.168.1.5",
            "192.168.1.20:9222",
            "10.0.0.1",
            "10.0.0.1:9222",
            "0.0.0.0:9222",
            "127.0.0.2:9222",
            "[::]:9222",
            "[fe80::1]:9222",
            "[::ffff:127.0.0.1]:9222",
        ] {
            assert!(!host_allowed(Some(host)), "{host}");
        }
        assert!(!origin_allowed(
            Some("http://192.168.1.5:9222"),
            Some("192.168.1.5:9222")
        ));
    }

    #[test]
    fn clients_without_an_origin_are_allowed() {
        assert!(origin_allowed(None, Some("127.0.0.1:9222")));
    }

    #[test]
    fn the_frontend_origins_are_allowed() {
        let host = Some("127.0.0.1:9222");
        assert!(origin_allowed(Some("devtools://devtools"), host));
        assert!(origin_allowed(Some("http://127.0.0.1:9222"), host));
        assert!(origin_allowed(
            Some("http://localhost:9222"),
            Some("localhost:9222")
        ));
    }

    #[test]
    fn web_page_origins_are_refused() {
        let host = Some("127.0.0.1:9222");
        for origin in [
            "https://evil.example",
            "http://127.0.0.1:8080",
            "http://127.0.0.1:9222.evil.example",
            "null",
            "file://",
            "chrome-extension://abcdef",
        ] {
            assert!(!origin_allowed(Some(origin), host), "{origin}");
        }
        assert!(!origin_allowed(
            Some("http://evil.example:9222"),
            Some("evil.example:9222")
        ));
    }
}
