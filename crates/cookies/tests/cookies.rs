use browsai_cookies::{Cookie, CookieJar};
use browsai_profiles::ProfileManager;
use url::Url;

#[test]
fn cookies_match_domain_path_and_secure_policy_with_partitioning() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let request = Url::parse("https://sub.example.test/app/page").unwrap();
    let cookie = Cookie::parse(
        "sid=abc; Domain=.example.test; Path=/app; Secure; HttpOnly",
        &request,
    )
    .unwrap();
    let mut jar = CookieJar::default();
    jar.set(&profile, "https://example.test", cookie);
    assert_eq!(
        jar.header_for(&profile, "https://example.test", &request),
        "sid=abc"
    );
    assert_eq!(jar.header_for(&profile, "https://other.test", &request), "");
}

#[test]
fn cookies_enforce_expiry_samesite_and_path_boundaries() {
    let request = Url::parse("https://example.test/app/page").unwrap();
    let expiring = Cookie {
        name: "sid".into(),
        value: "abc".into(),
        domain: "example.test".into(),
        host_only: false,
        path: "/app".into(),
        secure: true,
        http_only: false,
        same_site: Some("strict".into()),
        expires: Some(10),
    };
    let mut jar = CookieJar::default();
    let profile = ProfileManager::default().create("work");
    jar.set(&profile, "https://example.test", expiring);
    assert_eq!(
        jar.header_for_at(&profile, "https://example.test", &request, 9),
        "sid=abc"
    );
    assert_eq!(
        jar.header_for_at(&profile, "https://example.test", &request, 10),
        ""
    );
    let outside_path = Url::parse("https://example.test/application").unwrap();
    assert_eq!(
        jar.header_for_at(&profile, "https://example.test", &outside_path, 9),
        ""
    );
    assert_eq!(
        jar.header_for_at(&profile, "https://other.test", &request, 9),
        ""
    );
}

#[test]
fn host_only_cookies_do_not_cross_subdomains() {
    let request = Url::parse("https://example.test/login").unwrap();
    let cookie = Cookie::parse("sid=host-only", &request).unwrap();
    let mut jar = CookieJar::default();
    let profile = ProfileManager::default().create("work");
    jar.set(&profile, "https://example.test", cookie);
    assert_eq!(
        jar.header_for_at(&profile, "https://example.test", &request, 0),
        "sid=host-only"
    );
    let subdomain = Url::parse("https://sub.example.test/login").unwrap();
    assert_eq!(
        jar.header_for_at(&profile, "https://example.test", &subdomain, 0),
        ""
    );
}

#[test]
fn parsed_max_age_is_enforced_with_an_injected_clock() {
    let request = Url::parse("https://example.test/").unwrap();
    let cookie = Cookie::parse_at("session=abc; Max-Age=60", &request, 1_000).unwrap();
    assert!(cookie.matches_at(&request, 1_059, true));
    assert!(!cookie.matches_at(&request, 1_060, true));
    let expired = Cookie::parse_at("session=gone; Max-Age=0", &request, 1_000).unwrap();
    assert!(!expired.matches_at(&request, 1_000, true));
}

#[test]
fn cookie_jar_persists_partitions_and_purges_expired_entries() {
    let profile = ProfileManager::default().create("work");
    let request = Url::parse("https://example.test/").unwrap();
    let mut jar = CookieJar::default();
    jar.set(
        &profile,
        "https://example.test",
        Cookie::parse_at("session=abc; Max-Age=60", &request, 1_000).unwrap(),
    );
    let restored = CookieJar::from_json(&jar.to_json().unwrap()).unwrap();
    assert_eq!(
        restored.header_for_at(&profile, "https://example.test", &request, 1_010,),
        "session=abc"
    );
    let mut restored = restored;
    assert_eq!(restored.purge_expired(1_060), 1);
    assert!(restored
        .header_for_at(&profile, "https://example.test", &request, 1_060,)
        .is_empty());
}
