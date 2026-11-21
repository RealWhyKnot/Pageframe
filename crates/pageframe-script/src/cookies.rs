use url::Url;

#[derive(Clone, Debug)]
struct Entry {
    host: String,
    any_subdomain: bool,
    path: String,
    name: String,
    value: String,
}

impl Entry {
    fn parse(line: &str, url: &Url) -> Option<(Self, bool)> {
        let mut parts = line.split(';');
        let (name, value) = parts.next()?.split_once('=')?;
        let mut entry = Self {
            host: url.host_str()?.to_ascii_lowercase(),
            any_subdomain: false,
            path: default_path(url),
            name: name.trim().to_string(),
            value: value.trim().to_string(),
        };
        let mut expired = false;
        for attribute in parts {
            let (key, value) = attribute.split_once('=').unwrap_or((attribute, ""));
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "path" if value.starts_with('/') => entry.path = value.to_string(),
                "domain" if !value.is_empty() => {
                    let domain = value.trim_start_matches('.').to_ascii_lowercase();
                    let host_is_ip = !matches!(url.host(), Some(url::Host::Domain(_)));
                    if host_is_ip || !domain.contains('.') {
                        if domain != entry.host {
                            return None;
                        }
                        continue;
                    }
                    if !host_matches(&entry.host, &domain) {
                        return None;
                    }
                    entry.host = domain;
                    entry.any_subdomain = true;
                }
                "max-age" => expired |= value.parse::<i64>().is_ok_and(|age| age <= 0),
                _ => {}
            }
        }
        (!entry.name.is_empty()).then_some((entry, expired))
    }

    fn applies_to(&self, url: &Url) -> bool {
        let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
            return false;
        };
        let host_ok = if self.any_subdomain {
            host_matches(&host, &self.host)
        } else {
            host == self.host
        };
        let path = url.path();
        let path_ok = path == self.path
            || (path.starts_with(&self.path)
                && (self.path.ends_with('/') || path[self.path.len()..].starts_with('/')));
        host_ok && path_ok
    }
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn default_path(url: &Url) -> String {
    let path = url.path();
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => path[..index].to_string(),
    }
}

#[derive(Default)]
pub(crate) struct CookieJar {
    entries: Vec<Entry>,
}

impl CookieJar {
    pub(crate) fn header_for(&self, url: &Url) -> Option<String> {
        let pairs: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| entry.applies_to(url))
            .map(|entry| format!("{}={}", entry.name, entry.value))
            .collect();
        (!pairs.is_empty()).then(|| pairs.join("; "))
    }

    pub(crate) fn store(&mut self, url: &Url, headers: &[(String, String)]) {
        for (_, line) in headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case("set-cookie"))
        {
            let Some((entry, expired)) = Entry::parse(line, url) else {
                continue;
            };
            self.entries.retain(|old| {
                old.name != entry.name || old.host != entry.host || old.path != entry.path
            });
            if !expired {
                self.entries.push(entry);
            }
        }
    }
}
