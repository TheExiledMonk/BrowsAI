use browsai_profiles::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub expires: Option<i64>,
}

impl Cookie {
    pub fn parse(header: &str, request: &Url) -> Option<Self> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map_or(i64::MAX, |duration| duration.as_secs() as i64);
        Self::parse_at(header, request, now)
    }

    pub fn parse_at(header: &str, request: &Url, now_unix_seconds: i64) -> Option<Self> {
        let mut parts = header.split(';').map(str::trim);
        let (name, value) = parts.next()?.split_once('=')?;
        let mut cookie = Self {
            name: name.to_owned(),
            value: value.to_owned(),
            domain: request.host_str()?.to_owned(),
            host_only: true,
            path: "/".into(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
        };
        for part in parts {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            match key.to_ascii_lowercase().as_str() {
                "domain" => {
                    cookie.domain = value.trim_start_matches('.').to_ascii_lowercase();
                    cookie.host_only = false;
                }
                "path" if !value.is_empty() => cookie.path = value.to_owned(),
                "secure" => cookie.secure = true,
                "httponly" => cookie.http_only = true,
                "max-age" => {
                    let seconds = value.parse::<i64>().ok()?;
                    cookie.expires = Some(if seconds <= 0 {
                        now_unix_seconds
                    } else {
                        now_unix_seconds.saturating_add(seconds)
                    });
                }
                "samesite" if !value.is_empty() => {
                    cookie.same_site = Some(value.to_ascii_lowercase())
                }
                _ => {}
            }
        }
        Some(cookie)
    }
    pub fn matches(&self, url: &Url) -> bool {
        let host = match url.host_str() {
            Some(host) => host.to_ascii_lowercase(),
            None => return false,
        };
        let domain = self.domain.to_ascii_lowercase();
        let domain_match = if self.host_only {
            host == domain
        } else {
            host == domain || host.ends_with(&format!(".{domain}"))
        };
        let path_match = if self.path == "/" {
            true
        } else {
            url.path() == self.path
                || url
                    .path()
                    .starts_with(&format!("{}/", self.path.trim_end_matches('/')))
        };
        domain_match && path_match && (!self.secure || url.scheme() == "https")
    }

    pub fn matches_at(&self, url: &Url, now_unix_seconds: i64, same_site: bool) -> bool {
        self.matches(url)
            && self
                .expires
                .map_or(true, |expires| expires > now_unix_seconds)
            && self
                .same_site
                .as_deref()
                .map_or(true, |policy| match policy {
                    "strict" | "lax" => same_site,
                    "none" => self.secure,
                    _ => true,
                })
    }
}

#[derive(Clone, Debug, Default)]
pub struct CookieJar {
    values: BTreeMap<(ProfileId, String, String, String), Cookie>,
}

impl CookieJar {
    pub fn set(&mut self, profile: &ProfileId, top_level_site: &str, cookie: Cookie) {
        self.values.insert(
            (
                profile.clone(),
                top_level_site.to_owned(),
                cookie.domain.clone(),
                cookie.name.clone(),
            ),
            cookie,
        );
    }
    pub fn header_for(&self, profile: &ProfileId, top_level_site: &str, url: &Url) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(i64::MAX, |duration| duration.as_secs() as i64);
        self.header_for_at(profile, top_level_site, url, now)
    }

    pub fn header_for_at(
        &self,
        profile: &ProfileId,
        top_level_site: &str,
        url: &Url,
        now_unix_seconds: i64,
    ) -> String {
        let same_site = Url::parse(top_level_site)
            .ok()
            .is_some_and(|site| site.host_str() == url.host_str());
        self.values
            .iter()
            .filter(|((stored_profile, site, _, _), cookie)| {
                stored_profile == profile
                    && site == top_level_site
                    && cookie.matches_at(url, now_unix_seconds, same_site)
            })
            .map(|((_, _, _, name), cookie)| format!("{name}={}", cookie.value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn purge_expired(&mut self, now_unix_seconds: i64) -> usize {
        let before = self.values.len();
        self.values.retain(|_, cookie| {
            cookie
                .expires
                .map_or(true, |expires| expires > now_unix_seconds)
        });
        before - self.values.len()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let entries = self
            .values
            .iter()
            .map(|((profile, top_level_site, _, _), cookie)| CookieEntry {
                profile: profile.clone(),
                top_level_site: top_level_site.clone(),
                cookie: cookie.clone(),
            })
            .collect();
        serde_json::to_string(&CookieJarWire { entries })
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let wire: CookieJarWire = serde_json::from_str(value)?;
        let mut jar = Self::default();
        for entry in wire.entries {
            jar.set(&entry.profile, &entry.top_level_site, entry.cookie);
        }
        Ok(jar)
    }
}

#[derive(Serialize, Deserialize)]
struct CookieJarWire {
    entries: Vec<CookieEntry>,
}

#[derive(Serialize, Deserialize)]
struct CookieEntry {
    profile: ProfileId,
    top_level_site: String,
    cookie: Cookie,
}
