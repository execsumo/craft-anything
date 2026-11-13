use super::*;
use crate::{CallOptions, Caller, Consent, Engine};

fn granted() -> CallOptions<'static> {
    CallOptions::new(Caller::Cli).with_consent(Consent::Granted)
}

#[test]
fn a_site_key_is_the_normalized_host_and_effective_port() {
    let k = |u: &str| site_key(u).unwrap();
    assert_eq!(k("https://Example.COM/path?q=1#x"), "site-example.com_443");
    assert_eq!(k("https://example.com:443/"), "site-example.com_443");
    assert_eq!(k("http://example.com:80"), "site-example.com_80");
    assert_eq!(k("http://example.com"), k("https://EXAMPLE.com.:080"));
    assert_eq!(k("https://example.com"), k("http://example.com:0443"));
    assert_ne!(k("http://example.com"), k("https://example.com"));
    assert_eq!(k("http://example.com:443"), "site-example.com_443");
    assert_eq!(k("https://app.example.com:8443/x"), "site-app.example.com_8443");
    assert_eq!(k("http://127.0.0.1:5173/"), "site-127.0.0.1_5173");
    assert_eq!(k("http://[::1]:3000/"), "site-ip6_--1_3000");
    assert_eq!(k("https://user:pw@example.com/"), "site-example.com_443", "credentials are not part of the key");
    assert_eq!(k("https://example.com./"), "site-example.com_443");
    assert_eq!(k("https://xn--bcher-kva.example/"), "site-xn--bcher-kva.example_443", "punycode is a name like any other");
    assert_ne!(k("https://a.example.com/"), k("https://example.com/"), "exact host only");
    // Names Windows reserves for devices never become a folder name on their own.
    assert_eq!(k("https://con/"), "site-con_443");
    assert_eq!(k("https://nul.example.com/"), "site-nul.example.com_443");
    assert!(site_key("https://example.com../").is_err(), "only one trailing dot is the same host");
    assert!(site_key(&format!("https://{}.example/", "a".repeat(120))).is_err(), "keys stay short enough for any path");
}

/// Replacing a record changes its revision even when the display timestamp is unchanged.
#[test]
fn saved_profile_revisions_distinguish_same_second_replacements() {
    let s = crate::testing::session();
    let key = site_key("https://example.com").unwrap();
    save_fixture(&s, &key);
    let record = sign_ins_dir(&s).join(&key).join(RECORD_FILE);
    let first = unique("saved");
    let second = unique("saved");
    assert_ne!(first, second);
    for revision in [first, second] {
        std::fs::write(&record, json!({"version": RECORD_VERSION, "site": key, "saved_at": 1, "revision": revision}).to_string()).unwrap();
        let loaded = saved(&s, &key).unwrap();
        assert_eq!(loaded.saved_at, 1);
        assert_eq!(loaded.revision, revision);
    }
}

#[test]
fn hostile_hosts_never_make_a_key() {
    for url in [
        "https://bücher.example/",
        "https://../",
        "https://./",
        "https://.example.com/",
        "https://a..b/",
        "https://exa%2fmple.com/",
        "https://exa_mple.com/",
        "https://ip6_--1/",
        "https://host:0/",
        "https://host:99999/",
        "https://host:12a/",
        "https://[::1/",
        "https://[zz]/",
        "https://@/",
        "https://..\\..\\x/",
        "file:///etc/passwd",
        "https:///nohost",
        "not a url",
    ] {
        assert!(site_key(url).is_err(), "{url} made a key: {:?}", site_key(url));
    }
    let long = format!("https://{}.example/", "a".repeat(300));
    assert!(site_key(&long).is_err(), "an overlong host is refused");
    // Whatever is accepted is one safe path component.
    for url in ["https://a-b.c", "http://x:8080", "http://[fe80::1]"] {
        let key = site_key(url).unwrap();
        assert!(key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.' | '_')), "{key}");
        assert!(!key.is_empty() && key != "." && key != ".." && !key.contains(".."), "{key}");
    }
}

/// A saved sign-in for `key` in `session`'s folder: a cookie file to keep, and caches, locks and
/// a port file that must never be copied.
fn save_fixture(session: &Session, key: &str) -> PathBuf {
    let dir = sign_ins_dir(session).join(key);
    let profile = dir.join(PROFILE_DIR);
    std::fs::create_dir_all(profile.join("Default/Cache")).unwrap();
    std::fs::create_dir_all(profile.join("Default/Service Worker/CacheStorage")).unwrap();
    std::fs::write(profile.join("Default/Cookies"), b"fake-cookie-db").unwrap();
    std::fs::write(profile.join("Default/Cache/data_0"), b"cached").unwrap();
    std::fs::write(profile.join("Default/Service Worker/CacheStorage/x"), b"cached").unwrap();
    std::fs::write(profile.join("Default/Service Worker/Database"), b"kept").unwrap();
    std::fs::write(profile.join("SingletonCookie"), b"lock").unwrap();
    std::fs::write(profile.join("DevToolsActivePort"), b"9\n/devtools/browser/x").unwrap();
    std::fs::write(profile.join("Local State"), b"{}").unwrap();
    std::fs::write(dir.join(RECORD_FILE), json!({"version": 1, "site": key, "saved_at": 1}).to_string()).unwrap();
    profile
}

#[test]
fn a_profile_copy_skips_caches_locks_and_links() {
    let s = crate::testing::session();
    let from = save_fixture(&s, "site-example.com_443");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"not ours").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), from.join("Default/linked-dir")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), from.join("Default/linked-file")).unwrap();
        std::os::unix::fs::symlink("host-123", from.join("SingletonLock")).unwrap();
    }
    let to = tempfile::tempdir().unwrap();
    copy_profile(&from, to.path(), &CopyCaps::default()).unwrap();
    let t = to.path();
    assert_eq!(std::fs::read(t.join("Default/Cookies")).unwrap(), b"fake-cookie-db");
    assert_eq!(std::fs::read(t.join("Default/Service Worker/Database")).unwrap(), b"kept");
    assert!(t.join("Local State").is_file());
    for gone in [
        "Default/Cache",
        "Default/Service Worker/CacheStorage",
        "SingletonCookie",
        "SingletonLock",
        "DevToolsActivePort",
        "Default/linked-dir",
        "Default/linked-file",
    ] {
        assert!(std::fs::symlink_metadata(t.join(gone)).is_err(), "{gone} was copied");
    }
    assert!(from.join("Default/Cache/data_0").is_file(), "the saved sign-in is untouched");
}

/// Review of #1459: a password the person let the browser save must not travel with the sign-in.
#[test]
fn password_and_card_stores_are_never_copied_and_leave_an_earlier_profile() {
    let s = crate::testing::session();
    let from = save_fixture(&s, "site-example.com_443");
    let stores = [
        "Default/Login Data",
        "Default/Login Data-journal",
        "Default/Login Data For Account",
        "Default/Login Data For Account-wal",
        "Default/Web Data",
        "Default/Web Data-shm",
        "Default/Account Web Data",
        "Profile 1/Login Data",
    ];
    std::fs::create_dir_all(from.join("Profile 1")).unwrap();
    for f in stores {
        std::fs::write(from.join(f), b"fake-password-store").unwrap();
    }
    // Look-alikes that are not credential stores still travel.
    std::fs::write(from.join("Default/Login Database Notes"), b"kept").unwrap();
    let to = tempfile::tempdir().unwrap();
    copy_profile(&from, to.path(), &CopyCaps::default()).unwrap();
    assert_eq!(std::fs::read(to.path().join("Default/Cookies")).unwrap(), b"fake-cookie-db");
    assert!(to.path().join("Default/Login Database Notes").is_file());
    for f in stores {
        assert!(std::fs::symlink_metadata(to.path().join(f)).is_err(), "{f} was copied");
    }
    // A profile saved before the stores were left out loses them when it's next used.
    remove_credential_stores(&from);
    for f in stores {
        assert!(std::fs::symlink_metadata(from.join(f)).is_err(), "{f} was left in the saved profile");
    }
    assert_eq!(std::fs::read(from.join("Default/Cookies")).unwrap(), b"fake-cookie-db");
}

#[test]
fn sign_in_profiles_turn_off_password_and_card_saving() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Default")).unwrap();
    std::fs::write(d.path().join("Default/Preferences"), br#"{"profile": {"password_manager_enabled": true, "name": "x"}, "autofill": 3}"#).unwrap();
    prepare_sign_in_profile(d.path()).unwrap();
    let prefs: Value = serde_json::from_slice(&std::fs::read(d.path().join("Default/Preferences")).unwrap()).unwrap();
    assert_eq!(prefs["credentials_enable_service"], false);
    assert_eq!(prefs["credentials_enable_autosignin"], false);
    assert_eq!(prefs["profile"]["password_manager_enabled"], false);
    assert_eq!(prefs["profile"]["name"], "x");
    assert_eq!(prefs["autofill"]["profile_enabled"], false);
    assert_eq!(prefs["autofill"]["credit_card_enabled"], false);
}

#[test]
fn session_cookies_are_kept_whatever_preferences_were_there() {
    let prefs_after = |before: Option<&[u8]>| {
        let d = tempfile::tempdir().unwrap();
        if let Some(b) = before {
            std::fs::create_dir_all(d.path().join("Default")).unwrap();
            std::fs::write(d.path().join("Default/Preferences"), b).unwrap();
        }
        prepare_sign_in_profile(d.path()).unwrap();
        serde_json::from_slice::<Value>(&std::fs::read(d.path().join("Default/Preferences")).unwrap()).unwrap()
    };
    assert_eq!(prefs_after(None)["session"]["restore_on_startup"], 1);
    let merged = prefs_after(Some(br#"{"intl": {"accept_languages": "en"}, "session": {"restore_on_startup": 5, "x": 2}}"#));
    assert_eq!(
        (merged["session"]["restore_on_startup"].as_u64(), merged["session"]["x"].as_u64(), merged["intl"]["accept_languages"].as_str()),
        (Some(1), Some(2), Some("en"))
    );
    for hostile in [&b"not json"[..], b"[1,2]", br#"{"session": "nope"}"#, b"\xff\xfe", b""] {
        assert_eq!(prefs_after(Some(hostile))["session"]["restore_on_startup"], 1, "{:?}", String::from_utf8_lossy(hostile));
    }
}

#[cfg(unix)]
#[test]
fn saved_sign_ins_are_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;
    let s = crate::testing::session();
    let from = save_fixture(&s, "site-from.example");
    let dir = sign_ins_dir(&s).join("site-example.com_443");
    keep_profile(&from, &dir).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&sign_ins_dir(&s)), 0o700);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join(PROFILE_DIR)), 0o700);
    assert_eq!(mode(&dir.join(PROFILE_DIR).join("Default")), 0o700);
    assert_eq!(mode(&dir.join(PROFILE_DIR).join("Default/Cookies")), 0o600);
}

/// A new sign-in replaces the old one whole, and leftovers of a run that stopped part-way are
/// cleared; a folder that's only half there is never reported as saved.
#[test]
fn a_new_sign_in_replaces_the_old_one_whole() {
    let s = crate::testing::session();
    let first = save_fixture(&s, "site-first.example");
    let dir = sign_ins_dir(&s).join("site-example.com_443");
    keep_profile(&first, &dir).unwrap();
    std::fs::create_dir_all(dir.join("profile.new-stale")).unwrap();
    std::fs::create_dir_all(dir.join("profile.old-stale")).unwrap();
    let second = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(second.path().join("Default")).unwrap();
    std::fs::write(second.path().join("Default/Cookies"), b"second").unwrap();
    keep_profile(second.path(), &dir).unwrap();
    assert_eq!(std::fs::read(dir.join(PROFILE_DIR).join("Default/Cookies")).unwrap(), b"second");
    assert!(!dir.join(PROFILE_DIR).join("Local State").exists(), "nothing of the old one is mixed in");
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec![PROFILE_DIR.to_string()], "no staging or old folders are left");
    assert_eq!(saved(&s, "site-example.com_443"), None, "no record yet: the tool writes it last");
}

/// A browser that may still be running is never copied: the earlier sign-in stays whole and the
/// save fails instead of reporting success.
#[test]
fn an_unconfirmed_browser_stop_keeps_the_earlier_sign_in() {
    let s = crate::testing::session();
    let first = save_fixture(&s, "site-first.example");
    let dir = sign_ins_dir(&s).join("site-example.com_443");
    keep_profile(&first, &dir).unwrap();
    let before = snapshot(&dir);
    let second = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(second.path().join("Default")).unwrap();
    std::fs::write(second.path().join("Default/Cookies"), b"half-written").unwrap();
    let e = keep_closed_profile(BrowserClosed::Unconfirmed, second.path(), &dir).unwrap_err();
    assert!(e.message.contains("wasn't saved"), "{}", e.message);
    assert_eq!(snapshot(&dir), before, "the earlier sign-in is untouched");
    keep_closed_profile(BrowserClosed::Stopped, second.path(), &dir).unwrap();
    assert_eq!(std::fs::read(dir.join(PROFILE_DIR).join("Default/Cookies")).unwrap(), b"half-written");
}

#[test]
fn a_window_counts_as_closed_only_when_it_stays_closed() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut w = CloseWatch::default();
    assert!(!w.closed(Look::Silent, at(0)), "one slow answer isn't a close");
    assert!(!w.closed(Look::NoPages, at(700)));
    assert!(!w.closed(Look::Open, at(1400)), "a page came back: start again");
    assert!(!w.closed(Look::NoPages, at(2100)));
    assert!(!w.closed(Look::NoPages, at(2800)));
    assert!(!w.closed(Look::Silent, at(3500)), "three looks, but not yet 2 s");
    assert!(w.closed(Look::Silent, at(4200)));
    assert!(CloseWatch::default().closed(Look::Exited, at(0)), "the browser exiting is enough");
}

#[test]
fn a_check_after_saving_is_typed_and_never_fails_the_sign_in() {
    let ok =
        Checked { sign_in_page: false, url: "https://ex.com/inbox".into(), picture: json!({"path": "check.png", "sha256": "ab", "width": 10, "height": 10}) };
    let (v, notes) = check_result(Ok(ok.clone()));
    assert_eq!((v["done"].as_bool(), v["sign_in_page"].as_bool(), v["url"].as_str()), (Some(true), Some(false), Some("https://ex.com/inbox")));
    assert!(notes.iter().any(|n| n.code == "check_picture" && n.message.contains("your own account")));
    assert!(!notes.iter().any(|n| n.code == "sign_in_not_kept"));
    let (v, notes) = check_result(Ok(Checked { sign_in_page: true, ..ok }));
    assert_eq!(v["sign_in_page"], true);
    assert!(notes.iter().any(|n| n.code == "sign_in_not_kept" && n.message.contains("remember me")));
    let (v, notes) = check_result(Err(ToolError::new(ErrorCode::TimedOut, "the page took too long")));
    assert_eq!((v["done"].as_bool(), v["sign_in_page"].is_null(), v["error"].as_str()), (Some(false), true, Some("the page took too long")));
    assert!(notes.iter().any(|n| n.code == "not_checked_after"));
}

#[test]
fn a_profile_copy_over_a_cap_is_refused() {
    let s = crate::testing::session();
    let from = save_fixture(&s, "site-example.com_443");
    let refused = |caps: CopyCaps| {
        let to = tempfile::tempdir().unwrap();
        copy_profile(&from, to.path(), &caps).unwrap_err()
    };
    let roomy = CopyCaps::default();
    let e = refused(CopyCaps { max_bytes: 4, ..roomy });
    assert_eq!(e.code, ErrorCode::OverLimit, "{}", e.message);
    assert!(e.remediation.unwrap_or_default().contains(SIGN_OUT_TOOL));
    assert_eq!(refused(CopyCaps { max_files: 1, ..roomy }).code, ErrorCode::OverLimit);
    let deep = refused(CopyCaps { max_depth: 1, ..roomy });
    assert!(deep.message.contains("deeper"), "{}", deep.message);
}

/// Public pages with no saved profile do not need a writable sign-in store.
#[cfg(unix)]
#[test]
fn opening_without_a_saved_profile_does_not_create_reservations() {
    let logs = tempfile::tempdir().unwrap();
    let mut session = crate::testing::session();
    session.settings.engines.tools.chromium = Some(recording_browser(logs.path()));
    std::fs::write(sign_ins_dir(&session), b"not a directory").unwrap();
    let e = Engine::new(session);
    let err = e.call("web_site_open", json!({"url": "https://example.com"}), granted()).unwrap_err();
    assert!(err.message.contains("closed after"), "a public page reaches browser startup: {}", err.message);
    assert!(!std::fs::read_to_string(logs.path().join("listing")).unwrap().contains("Cookies"));
}

/// Overlapping engines must not delete or seed from a profile while a sign-in is using it.
#[test]
fn same_site_operations_fail_fast_but_other_sites_and_fresh_opens_are_free() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::isolated(dir.path());
    s.settings.deep_mode = true;
    s.settings.engines.only_configured = true;
    s.settings.engines.tools.chromium = Some(std::env::current_exe().unwrap());
    save_fixture(&s, "site-example.com_443");
    let held = lock_site(&s, "site-example.com_443").unwrap();
    let e = Engine::new(s);
    assert_eq!(lock_site(&e.session(), "site-example.com_443").unwrap_err().code, ErrorCode::TimedOut);
    assert!(lock_site(&e.session(), "site-other.example_443").is_ok());
    for id in [SIGN_OUT_TOOL, "web_site_open"] {
        let err = e.call(id, json!({"url": "https://example.com"}), granted()).unwrap_err();
        assert_eq!(err.code, ErrorCode::TimedOut, "{id}: {}", err.message);
        assert!(err.remediation.unwrap_or_default().contains("window"));
    }
    #[cfg(unix)]
    {
        let logs = tempfile::tempdir().unwrap();
        e.session().settings.engines.tools.chromium = Some(recording_browser(logs.path()));
        let fresh = e.call("web_site_open", json!({"url": "https://example.com", "signed_in": false}), granted()).unwrap_err();
        assert!(fresh.message.contains("closed after"), "fresh requests reach browser startup: {}", fresh.message);
        assert!(!std::fs::read_to_string(logs.path().join("listing")).unwrap().contains("Cookies"));
    }
    assert!(e.call(SIGN_OUT_TOOL, json!({"url": "https://other.example"}), granted()).is_ok());
    let profile = dir.path().join("sign-ins/site-example.com_443/profile");
    assert!(profile.join("Default/Cookies").is_file(), "a refused sign-out keeps the saved profile");
    drop(held);
    assert!(e.call(SIGN_OUT_TOOL, json!({"url": "https://example.com"}), granted()).is_ok());
    assert!(!profile.exists());
}

#[test]
fn signing_out_forgets_only_that_site_and_nothing_saved_is_fine() {
    let s = crate::testing::session();
    let base = sign_ins_dir(&s);
    save_fixture(&s, "site-example.com_443");
    save_fixture(&s, "site-other.example");
    let e = Engine::new(s);
    let out = e.call(SIGN_OUT_TOOL, json!({"url": "https://EXAMPLE.com/account"}), CallOptions::new(Caller::Cli)).unwrap().to_json();
    assert_eq!(out["result"]["removed"], true, "{out}");
    assert!(!base.join("site-example.com_443").exists());
    assert!(base.join("site-other.example").join(RECORD_FILE).is_file(), "another site's sign-in stays");
    let again = e.call(SIGN_OUT_TOOL, json!({"url": "https://example.com/"}), CallOptions::new(Caller::Cli)).unwrap().to_json();
    assert_eq!(again["result"]["removed"], false);
    assert!(again["summary"].as_str().unwrap().contains("nothing to forget"));
    assert!(e.call(SIGN_OUT_TOOL, json!({"url": "https://../"}), CallOptions::new(Caller::Cli)).is_err());
}

#[cfg(unix)]
#[test]
fn signing_out_removes_a_link_not_what_it_points_at() {
    let s = crate::testing::session();
    let base = sign_ins_dir(&s);
    std::fs::create_dir_all(&base).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("keep"), b"x").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), base.join("site-example.com_443")).unwrap();
    let out = Engine::new(s).call(SIGN_OUT_TOOL, json!({"url": "https://example.com"}), CallOptions::new(Caller::Cli)).unwrap().to_json();
    assert_eq!(out["result"]["removed"], true);
    assert!(elsewhere.path().join("keep").is_file());
}

#[test]
fn a_saved_sign_in_needs_its_record_and_a_real_profile() {
    let s = crate::testing::session();
    assert_eq!(saved(&s, "site-example.com_443"), None);
    let profile = save_fixture(&s, "site-example.com_443");
    assert_eq!(saved(&s, "site-example.com_443").map(|x| x.profile), Some(profile.clone()));
    // A record for another site, or a newer version, isn't used.
    let record = sign_ins_dir(&s).join("site-example.com_443").join(RECORD_FILE);
    std::fs::write(&record, json!({"version": 2, "site": "site-example.com_443"}).to_string()).unwrap();
    assert_eq!(saved(&s, "site-example.com_443"), None);
    std::fs::write(&record, json!({"version": 1, "site": "site-other.example"}).to_string()).unwrap();
    assert_eq!(saved(&s, "site-example.com_443"), None);
}

#[test]
fn signed_in_true_with_nothing_saved_names_the_sign_in_tool() {
    let mut s = crate::testing::session();
    // A stand-in path gets past the engine check; the error comes before anything starts.
    s.settings.engines.tools.chromium = Some(std::env::current_exe().unwrap());
    let err = Engine::new(s).call("web_site_open", json!({"url": "https://example.com", "signed_in": true}), granted()).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound, "{}", err.message);
    assert!(err.message.contains("site-example.com_443"), "{}", err.message);
    assert!(err.remediation.unwrap_or_default().contains(SIGN_IN_TOOL));
}

#[test]
fn a_sign_in_needs_a_desktop_and_a_sane_wait() {
    if cfg!(target_os = "linux") {
        let e = no_display_error(None, Some(" ".into())).unwrap();
        assert_eq!(e.code, ErrorCode::NotSupported);
        assert!(e.remediation.unwrap_or_default().contains("screen"));
        assert!(no_display_error(Some(":0".into()), None).is_none());
        assert!(no_display_error(None, Some("wayland-0".into())).is_none());
    } else {
        assert!(no_display_error(None, None).is_none());
    }
    let mut s = crate::testing::session();
    s.settings.engines.tools.chromium = Some(std::env::current_exe().unwrap());
    let e = Engine::new(s);
    let err = e.call(SIGN_IN_TOOL, json!({"url": "https://example.com", "timeout_secs": 5000}), granted()).unwrap_err();
    assert!(err.message.contains("timeout_secs"), "{}", err.message);
    let err = e.call(SIGN_IN_TOOL, json!({"url": "https://bücher.example"}), granted()).unwrap_err();
    assert_eq!(err.code, ErrorCode::BadInput, "{}", err.message);
}

/// A stand-in "browser" that writes down its arguments, what its profile folder holds and where
/// that folder is, then fails, so nothing real starts.
#[cfg(unix)]
fn recording_browser(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let log = dir.display();
    let script = format!(
        "#!/bin/sh\nfor a in \"$@\"; do case \"$a\" in --user-data-dir=*) d=\"${{a#--user-data-dir=}}\";; esac; done\n\
         printf '%s\\n' \"$@\" > '{log}/args'\nprintf '%s' \"$d\" > '{log}/profile'\n\
         (cd \"$d\" && find . | sort) > '{log}/listing'\nexit 1\n"
    );
    let exe = dir.join("fake-browser");
    std::fs::write(&exe, script).unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    exe
}

/// `web_site_open` on a site with a saved sign-in starts the browser with a copy of it (and the
/// shared cookie key flag on Linux); `signed_in: false` and another site start empty; the copy is
/// deleted afterwards and the saved sign-in is never changed.
#[cfg(unix)]
#[test]
fn opening_a_site_with_a_saved_sign_in_starts_from_a_copy() {
    let logs = tempfile::tempdir().unwrap();
    let mut s = crate::testing::session();
    s.settings.engines.tools.chromium = Some(recording_browser(logs.path()));
    let saved_profile = save_fixture(&s, "site-example.com_443");
    let e = Engine::new(s);
    let open = |p: Value| {
        for f in ["args", "profile", "listing"] {
            let _ = std::fs::remove_file(logs.path().join(f));
        }
        let err = e.call("web_site_open", p, granted()).unwrap_err();
        assert!(err.message.contains("closed after"), "{}", err.message);
        let read = |f: &str| std::fs::read_to_string(logs.path().join(f)).unwrap_or_default();
        (read("args"), read("listing"), PathBuf::from(read("profile")))
    };

    let (args, listing, copy) = open(json!({"url": "https://example.com/inbox", "timeout_secs": 30}));
    assert!(listing.contains("./Default/Cookies") && listing.contains("./Local State"), "{listing}");
    assert!(!listing.contains("Cache/") && !listing.contains("DevToolsActivePort") && !listing.contains("Singleton"), "{listing}");
    assert_eq!(args.contains("--password-store=basic"), cfg!(target_os = "linux"), "{args}");
    assert!(args.contains("--headless=new"), "captures stay headless: {args}");
    assert_ne!(copy, saved_profile, "the browser gets a copy, never the saved folder");
    assert!(!copy.exists(), "the copy is deleted with its browser");
    assert!(saved_profile.join("Default/Cache/data_0").is_file() && saved_profile.join("DevToolsActivePort").is_file(), "the saved sign-in is unchanged");

    for p in [json!({"url": "https://example.com/", "signed_in": false, "timeout_secs": 30}), json!({"url": "https://www.example.com/", "timeout_secs": 30})] {
        let (args, listing, _) = open(p.clone());
        assert!(!listing.contains("Cookies"), "{p}: {listing}");
        assert!(!args.contains("--password-store"), "the private path keeps its flags: {args}");
    }
}

// ---------------------------------------------------------------------------------------------
// Live tests: a real Chromium-based browser, a desktop for the window (`xvfb-run -a` works) and a
// local test site. Run one at a time: `cargo test -p craft-anything-engine --lib site_sign_in::
// -- --ignored --test-threads=1`. `CRAFT_ANYTHING_TEST_CHROMIUM` points at a browser the usual
// search doesn't find.

/// A live session whose config lives in `dir`.
fn live_in(dir: &Path) -> Session {
    let mut s = Session::isolated(dir);
    s.settings.deep_mode = true;
    crate::testing::live_browser(&mut s);
    s
}

/// A site with a sign-in: `/account` is "Signed in" with the `sid` cookie and a sign-in form
/// without it; `/login` sets `sid` (HttpOnly, with an expiry) and a plain `pref` cookie.
fn test_site() -> String {
    serve_site(false)
}

/// The test site; `forgets` makes `/account` ignore the cookie (a site whose sign-in isn't kept).
fn serve_site(forgets: bool) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            while !req.windows(4).any(|w| w == b"\r\n\r\n") && req.len() < 65536 {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => req.extend_from_slice(&buf[..n]),
                }
            }
            let req = String::from_utf8_lossy(&req).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let signed = !forgets && req.lines().any(|l| l.to_ascii_lowercase().starts_with("cookie:") && l.contains("sid=ok"));
            let session_only = req.lines().any(|l| l.to_ascii_lowercase().starts_with("cookie:") && l.contains("tmp=1"));
            let reply = if path.starts_with("/login") {
                "HTTP/1.1 302 Found\r\nSet-Cookie: sid=ok; Max-Age=86400; Path=/; HttpOnly\r\nSet-Cookie: pref=1; Max-Age=86400; Path=/\r\nSet-Cookie: tmp=1; Path=/\r\nLocation: /account\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                let page = if signed {
                    format!("<!doctype html><title>Signed in</title><h1>Signed in as Test Person</h1><h2>session-only cookie kept: {session_only}</h2>")
                } else {
                    "<!doctype html><title>Signed out</title><h1>Signed out</h1><form action=/login><input type=password name=p><button>Sign in</button></form>"
                        .to_string()
                };
                format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len())
            };
            let _ = s.write_all(reply.as_bytes());
        }
    });
    base
}

/// Profile folders of launched browsers right now.
fn browser_profiles() -> Vec<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("craft-browser-"))).collect())
        .unwrap_or_default()
}

/// Processes whose command line names `needle`.
fn processes_naming(needle: &str) -> usize {
    std::fs::read_dir("/proc")
        .map(|d| d.flatten().filter(|e| std::fs::read(e.path().join("cmdline")).is_ok_and(|c| String::from_utf8_lossy(&c).contains(needle))).count())
        .unwrap_or(0)
}

/// Play the person at the sign-in window: wait for a new browser profile, then (when `sign_in`)
/// go to `/login` there, and close the browser as closing its window would.
fn person(base: String, before: Vec<PathBuf>, sign_in: bool) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let dl = Deadline::new(Duration::from_secs(60), CancelToken::default());
        let endpoint = loop {
            assert!(!dl.expired(), "no sign-in browser appeared");
            let port = browser_profiles().into_iter().filter(|p| !before.contains(p)).find_map(|p| {
                std::fs::read_to_string(p.join("DevToolsActivePort")).ok().and_then(|t| t.lines().next().and_then(|l| l.trim().parse::<u16>().ok()))
            });
            if let Some(port) = port {
                break cdp::parse_endpoint(&format!("http://127.0.0.1:{port}")).unwrap();
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let b = Browser::attached(&endpoint, "test", "");
        if sign_in {
            let targets = cdp::json_list(&endpoint, &dl).unwrap();
            let page = web_common::pick_target(&targets, None, Some("page")).unwrap();
            let mut c = web_common::connect_target(&b, page, 1 << 20, &dl).unwrap();
            c.call("Page.navigate", json!({"url": format!("{base}/login")}), None, &dl).unwrap();
            std::thread::sleep(Duration::from_secs(3));
        }
        let version = cdp::json_version(&endpoint, &dl).unwrap();
        let mut c = web_common::connect_target(&b, &version, 1 << 20, &dl).unwrap();
        let _ = c.call("Browser.close", json!({}), None, &dl);
    })
}

fn title_of_capture(e: &Engine, dir: &Path, name: &str) -> Value {
    let out = dir.join(name);
    e.call("web_site_capture", json!({"output_directory": out.display().to_string(), "widths": [800]}), granted()).unwrap();
    serde_json::from_str(&std::fs::read_to_string(out.join("capture.json")).unwrap()).unwrap()
}

/// Every file of a saved sign-in with its bytes, to tell whether anything changed it.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.clone(), std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    out.sort();
    out
}

/// The handoff: signed in once in a window, the headless capture sees the signed-in page; opened
/// with `signed_in: false`, or after signing out, it's signed out again.
#[test]
#[ignore]
fn a_sign_in_made_in_a_window_is_seen_by_a_headless_capture() {
    let base = test_site();
    let cfg = tempfile::tempdir().unwrap();
    let e = Engine::new(live_in(cfg.path()));
    let helper = person(base.clone(), browser_profiles(), true);
    let v = e.call(SIGN_IN_TOOL, json!({"url": format!("{base}/account"), "timeout_secs": 120}), granted()).unwrap().to_json();
    helper.join().unwrap();
    assert_eq!(v["result"]["saved"], true, "{v}");
    assert_eq!(v["result"]["ended"], "closed", "{v}");
    let check = &v["result"]["check"];
    assert_eq!((check["done"].as_bool(), check["sign_in_page"].as_bool()), (Some(true), Some(false)), "{check}");
    let picture = PathBuf::from(check["picture"]["path"].as_str().unwrap());
    assert!(std::fs::metadata(&picture).unwrap().len() > 1000, "a real picture of the page");
    assert!(picture.starts_with(cfg.path()), "the picture stays in Craft Anything's folder");

    let out = tempfile::tempdir().unwrap();
    let opened = e.call("web_site_open", json!({"url": format!("{base}/account")}), granted()).unwrap().to_json();
    assert_eq!(opened["result"]["signed_in"], true, "{opened}");
    assert!(opened["limitations"].to_string().contains("\"signed_in\""), "{}", opened["limitations"]);
    let cap = title_of_capture(&e, out.path(), "signed-in");
    assert_eq!(cap["title"], "Signed in", "the session cookie didn't survive: {}", cap["title"]);
    assert_eq!(cap["signed_in"], true);
    assert_eq!(cap["sign_in_page"], false);
    // A cookie with no expiry (a sign-in without "remember me") is kept too.
    assert_eq!(cap["headings"][1]["text"], "session-only cookie kept: true", "{}", cap["headings"]);

    e.call("web_site_open", json!({"url": format!("{base}/account"), "signed_in": false}), granted()).unwrap();
    let cap = title_of_capture(&e, out.path(), "forced-private");
    assert_eq!((cap["title"].as_str(), cap["signed_in"].as_bool(), cap["sign_in_page"].as_bool()), (Some("Signed out"), Some(false), Some(true)));

    e.call(SIGN_OUT_TOOL, json!({"url": base}), granted()).unwrap();
    let opened = e.call("web_site_open", json!({"url": format!("{base}/account")}), granted()).unwrap().to_json();
    assert_eq!(opened["result"]["signed_in"], false);
    assert!(opened["limitations"].to_string().contains("private_profile"), "{}", opened["limitations"]);
    assert_eq!(title_of_capture(&e, out.path(), "after-sign-out")["title"], "Signed out");
}

/// Captures never change the saved sign-in, and two signed-in opens of one site at once both work.
#[test]
#[ignore]
fn captures_leave_the_saved_sign_in_alone_and_opens_can_overlap() {
    let base = test_site();
    let cfg = tempfile::tempdir().unwrap();
    let first = Engine::new(live_in(cfg.path()));
    let helper = person(base.clone(), browser_profiles(), true);
    first.call(SIGN_IN_TOOL, json!({"url": format!("{base}/account"), "timeout_secs": 120}), granted()).unwrap();
    helper.join().unwrap();
    let key = site_key(&base).unwrap();
    let saved_dir = sign_ins_dir(&live_in(cfg.path())).join(&key);
    let before = snapshot(&saved_dir);

    let second = Engine::new(live_in(cfg.path()));
    let out = tempfile::tempdir().unwrap();
    first.call("web_site_open", json!({"url": format!("{base}/account")}), granted()).unwrap();
    second.call("web_site_open", json!({"url": format!("{base}/account")}), granted()).unwrap();
    assert_eq!(title_of_capture(&first, out.path(), "one")["title"], "Signed in");
    assert_eq!(title_of_capture(&second, out.path(), "two")["title"], "Signed in");
    first.call("web_site_close", json!({}), granted()).unwrap();
    second.call("web_site_close", json!({}), granted()).unwrap();
    assert!(before == snapshot(&saved_dir), "a capture session changed the saved sign-in");
}

/// Nobody closes the window: the tool stops the browser at the deadline, keeps what it has and
/// leaves no browser running.
#[test]
#[ignore]
fn a_window_left_open_is_stopped_at_the_deadline() {
    let base = test_site();
    let cfg = tempfile::tempdir().unwrap();
    let e = Engine::new(live_in(cfg.path()));
    let started = Instant::now();
    let v = e.call(SIGN_IN_TOOL, json!({"url": format!("{base}/account"), "timeout_secs": 5}), granted()).unwrap().to_json();
    assert!(started.elapsed() < Duration::from_secs(45), "took {:?}", started.elapsed());
    assert_eq!(v["result"]["ended"], "timed_out", "{v}");
    assert!(v["limitations"].to_string().contains("window_stopped"));
    assert!(saved(&live_in(cfg.path()), &site_key(&base).unwrap()).is_some(), "what it had is kept");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(processes_naming("craft-browser-"), 0, "a browser was left running");
}

/// Without a desktop the sign-in says so at once. Run it without `xvfb-run`.
#[test]
#[ignore]
fn without_a_desktop_signing_in_is_refused_at_once() {
    if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() || !cfg!(target_os = "linux") {
        return;
    }
    let cfg = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let err = Engine::new(live_in(cfg.path())).call(SIGN_IN_TOOL, json!({"url": "http://127.0.0.1:9/"}), granted()).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(err.code, ErrorCode::NotSupported, "{}", err.message);
    println!("{} / {}", err.message, err.remediation.unwrap_or_default());
}

/// A site that doesn't keep the sign-in: the check after saving says it still asks for a password.
#[test]
#[ignore]
fn a_sign_in_the_site_forgets_is_caught_by_the_check() {
    let base = serve_site(true);
    let cfg = tempfile::tempdir().unwrap();
    let e = Engine::new(live_in(cfg.path()));
    let helper = person(base.clone(), browser_profiles(), true);
    let v = e.call(SIGN_IN_TOOL, json!({"url": format!("{base}/account"), "timeout_secs": 120}), granted()).unwrap().to_json();
    helper.join().unwrap();
    assert_eq!(v["result"]["check"]["sign_in_page"], true, "{}", v["result"]["check"]);
    assert!(v["limitations"].to_string().contains("sign_in_not_kept"), "{}", v["limitations"]);
}
