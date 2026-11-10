//! Saved sign-ins for websites (#1178). `web_site_sign_in` opens a visible browser on a site with
//! a profile folder Craft Anything keeps for that site; the person signs in themselves and closes
//! the window. Later, `web_site_open` on the same site (exact host and port) starts its private
//! browser with a copy of that profile, so the study sees the pages as the person does when signed
//! in. `web_site_sign_out` deletes the saved sign-in.
//!
//! We never see, read or store a password, and never read cookie or storage values: the browser
//! keeps them in its own profile files, which are only copied whole.

use super::browser_tools::browser_env;
use super::native_common::spec;
use super::web_common::{self, Browser, LaunchOptions, ProfileChoice};
use super::website_tools::check_url;
use crate::session::Session;
use crate::tool::{CancelToken, Ctx, Effects, Requires, ToolOutput, ToolSpec, Toolset, parse_params, schema_of};
use craft_anything_engines::cdp::{self, Deadline};
use craft_anything_evidence::{Authority, Confidence, Coverage, ErrorCode, EvidenceBuilder, Limitation, Location, Subject, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The tool that saves a sign-in; errors and evidence name it.
pub const SIGN_IN_TOOL: &str = "web_site_sign_in";
/// The tool that deletes one.
pub const SIGN_OUT_TOOL: &str = "web_site_sign_out";

/// Version of the `sign-in.json` record kept beside each saved profile.
const RECORD_VERSION: u64 = 1;
const RECORD_FILE: &str = "sign-in.json";
const PROFILE_DIR: &str = "profile";
/// Longest site key (a host name is at most 253 characters, plus a port).
const MAX_KEY_LEN: usize = 260;

pub fn specs() -> Vec<ToolSpec> {
    vec![
        spec(
            SIGN_IN_TOOL,
            "Sign in to a website",
            "Open a website in a visible browser window with a profile Craft Anything keeps for that site, and wait until you close the window (or timeout_secs passes, default 600, at most 1800). Sign in there yourself: Craft Anything never sees your password and never reads cookie or storage values. Later web_site_open calls for the same site (exact host and port) start with a copy of that sign-in, so studies and captures see the pages you see when signed in. web_site_sign_out forgets it. Needs a desktop where a window can open.",
            "Sign in to a site once yourself, so its pages can be studied as you see them.",
            Toolset::Browser,
            Effects::EVIDENCE
                .union(Effects::EXECUTES_FOREIGN_CODE)
                .union(Effects::ACCESSES_NETWORK)
                .union(Effects::LAUNCHES_PROCESS)
                .union(Effects::WRITES_FILESYSTEM),
            Requires::Engine("chromium"),
            schema_of::<SignInParams>,
            web_site_sign_in,
        ),
        spec(
            SIGN_OUT_TOOL,
            "Forget a website sign-in",
            "Delete the sign-in saved by web_site_sign_in for a website (exact host and port). Later web_site_open calls for that site use a fresh private browser again. Nothing saved is not an error.",
            "Forget the sign-in saved for a site.",
            Toolset::Browser,
            Effects::EVIDENCE.union(Effects::WRITES_FILESYSTEM).union(Effects::MAY_DISCARD_DATA),
            Requires::Nothing,
            schema_of::<SignOutParams>,
            web_site_sign_out,
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// Site keys and where saved sign-ins live

/// The folder name a site's saved sign-in lives under: the URL's host in lower case, plus `_port`
/// when the port isn't the scheme's default (`example.com`, `app.example.com_8443`, `127.0.0.1_5173`,
/// `ip6_--1` for `[::1]`). Only `a-z`, `0-9`, `-`, `.` and `_` are allowed, so a key is always one
/// safe path component; a host with anything else (a Unicode name: give its `xn--` form) is refused,
/// never rewritten, so two sites can't share a key.
pub fn site_key(url: &str) -> Result<String, ToolError> {
    let u = check_url(url)?;
    let (rest, default_port) = match u.strip_prefix("https://") {
        Some(r) => (r, "443"),
        None => (u.strip_prefix("http://").unwrap_or(u), "80"),
    };
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or_default();
    // Credentials before the host are never part of the key.
    let hostport = authority.rsplit('@').next().unwrap_or_default();
    let bad = || {
        ToolError::invalid(format!("{url:?} has no host name a sign-in can be saved for"))
            .with_remediation("Give the site's address as https://host (a name with letters outside a-z goes in its xn-- form).")
    };
    let (host, port) = if let Some(v6) = hostport.strip_prefix('[') {
        let (addr, after) = v6.split_once(']').ok_or_else(bad)?;
        if addr.is_empty() || !addr.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
            return Err(bad());
        }
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':').ok_or_else(bad)?),
        };
        (format!("ip6_{}", addr.to_ascii_lowercase().replace(':', "-")), port)
    } else if hostport.contains('_') {
        return Err(bad());
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) => (h.to_ascii_lowercase(), Some(p)),
            None => (hostport.to_ascii_lowercase(), None),
        }
    };
    let host = host.trim_end_matches('.');
    // `_` only ever comes from the IPv6 prefix above, so names and addresses can't collide.
    let name_ok = |h: &str| h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
    let host_ok = !host.is_empty()
        && (name_ok(host) || host.strip_prefix("ip6_").is_some_and(|a| a.chars().all(|c| c.is_ascii_hexdigit() || c == '-')))
        && !host.starts_with('.')
        && !host.contains("..");
    if !host_ok {
        return Err(bad());
    }
    let port = match port {
        None | Some("") => None,
        Some(p) => {
            let n: u16 = p.parse().ok().filter(|n| *n != 0 && p.chars().all(|c| c.is_ascii_digit())).ok_or_else(bad)?;
            (n.to_string() != default_port).then_some(n)
        }
    };
    let key = match port {
        Some(n) => format!("{host}_{n}"),
        None => host.to_string(),
    };
    if key.len() > MAX_KEY_LEN {
        return Err(bad());
    }
    Ok(key)
}

/// Where saved sign-ins live: a folder in this session's own config folder.
pub fn sign_ins_dir(session: &Session) -> PathBuf {
    session.config_dir().join("sign-ins")
}

/// A sign-in saved for one site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSignIn {
    pub key: String,
    /// The browser profile folder to copy.
    pub profile: PathBuf,
    /// When it was saved, in seconds since 1970.
    pub saved_at: u64,
}

/// The sign-in saved for `key`, when there is a complete one (its record and a real profile folder).
pub fn saved(session: &Session, key: &str) -> Option<SavedSignIn> {
    let dir = sign_ins_dir(session).join(key);
    let record: Value = serde_json::from_slice(&std::fs::read(dir.join(RECORD_FILE)).ok()?).ok()?;
    if record.get("version").and_then(Value::as_u64) != Some(RECORD_VERSION) || record.get("site").and_then(Value::as_str) != Some(key) {
        return None;
    }
    let profile = dir.join(PROFILE_DIR);
    let meta = std::fs::symlink_metadata(&profile).ok()?;
    meta.is_dir().then(|| SavedSignIn { key: key.to_string(), profile, saved_at: record.get("saved_at").and_then(Value::as_u64).unwrap_or(0) })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Copying a saved profile

/// Limits on a profile copy. Caches are skipped, so a real sign-in profile is far below these.
#[derive(Debug, Clone, Copy)]
pub struct CopyCaps {
    pub max_bytes: u64,
    pub max_files: u64,
    pub max_depth: usize,
}

impl Default for CopyCaps {
    fn default() -> Self {
        CopyCaps { max_bytes: 512 << 20, max_files: 20_000, max_depth: 16 }
    }
}

/// Folders a browser rebuilds by itself: never copied, wherever they are in the profile.
const SKIPPED_DIRS: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "ShaderCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "CacheStorage",
    "ScriptCache",
    "Crashpad",
    "Crash Reports",
    "component_crx_cache",
    "extensions_crx_cache",
    "optimization_guide_model_store",
    "Safe Browsing",
];

/// Files that belong to one running browser: its locks and the port it listened on.
const SKIPPED_FILES: &[&str] = &["SingletonLock", "SingletonSocket", "SingletonCookie", "lockfile", "DevToolsActivePort", "RunningChromeVersion"];

/// Copy the saved profile `from` into the empty folder `to`, skipping caches and lock files and
/// never following a link (links are left out). Going over a cap is an error, not a partial copy.
pub fn copy_profile(from: &Path, to: &Path, caps: &CopyCaps) -> Result<(), ToolError> {
    let over = |what: String| {
        ToolError::new(ErrorCode::OverLimit, format!("The saved sign-in is too big to use: {what}."))
            .with_remediation(format!("Forget it with {SIGN_OUT_TOOL} and sign in again with {SIGN_IN_TOOL}."))
    };
    let (mut bytes, mut files) = (0u64, 0u64);
    let mut stack: Vec<(PathBuf, PathBuf, usize)> = vec![(from.to_path_buf(), to.to_path_buf(), 0)];
    while let Some((src, dst, depth)) = stack.pop() {
        let entries = std::fs::read_dir(&src).map_err(|e| ToolError::io(src.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| ToolError::io(src.display(), e))?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let path = entry.path();
            // symlink_metadata: a link is seen as a link, never as what it points at.
            let meta = std::fs::symlink_metadata(&path).map_err(|e| ToolError::io(path.display(), e))?;
            let target = dst.join(&name);
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if SKIPPED_DIRS.contains(&name_str.as_ref()) {
                    continue;
                }
                if depth.saturating_add(1) > caps.max_depth {
                    return Err(over(format!("its folders go deeper than {} levels", caps.max_depth)));
                }
                std::fs::create_dir(&target).map_err(|e| ToolError::io(target.display(), e))?;
                stack.push((path, target, depth.saturating_add(1)));
            } else if meta.is_file() {
                if SKIPPED_FILES.contains(&name_str.as_ref()) {
                    continue;
                }
                files = files.saturating_add(1);
                bytes = bytes.saturating_add(meta.len());
                if files > caps.max_files {
                    return Err(over(format!("more than {} files", caps.max_files)));
                }
                if bytes > caps.max_bytes {
                    return Err(over(format!("more than {} MiB", caps.max_bytes >> 20)));
                }
                std::fs::copy(&path, &target).map_err(|e| ToolError::io(path.display(), e))?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// web_site_sign_in

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignInParams {
    /// The site's page to start on (http or https); the sign-in is saved for its host and port.
    pub url: String,
    /// Most seconds to wait for you to close the window (1..=1800, default 600).
    #[serde(default = "d_wait")]
    pub timeout_secs: u64,
}

fn d_wait() -> u64 {
    600
}

/// How the sign-in window ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    Closed,
    TimedOut,
    Cancelled,
}

impl Ended {
    fn id(self) -> &'static str {
        match self {
            Ended::Closed => "closed",
            Ended::TimedOut => "timed_out",
            Ended::Cancelled => "cancelled",
        }
    }
}

/// On Linux a window needs a desktop session; without one (a server, a remote shell) say so
/// before anything starts.
fn no_display_error(display: Option<String>, wayland: Option<String>) -> Option<ToolError> {
    let has = |v: Option<String>| v.is_some_and(|s| !s.trim().is_empty());
    (cfg!(target_os = "linux") && !has(display) && !has(wayland)).then(|| {
        ToolError::new(ErrorCode::NotSupported, "There's no desktop here to open a sign-in window on.")
            .with_remediation(format!("Run {SIGN_IN_TOOL} from Craft Anything on a computer with a screen (the app, or the CLI in a desktop session). Websites still open without signing in."))
    })
}

fn web_site_sign_in(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: SignInParams = parse_params(params)?;
    let url = check_url(&p.url)?.to_string();
    let key = site_key(&url)?;
    if !(1..=1800).contains(&p.timeout_secs) {
        return Err(ToolError::invalid("timeout_secs must be 1..=1800"));
    }
    if let Some(e) = no_display_error(std::env::var("DISPLAY").ok(), std::env::var("WAYLAND_DISPLAY").ok()) {
        return Err(e);
    }
    let dir = sign_ins_dir(ctx.session).join(&key);
    let earlier = saved(ctx.session, &key);
    let had = earlier.is_some();
    // The window runs on a temporary profile the browser can reach (a Snap's or Flatpak's own
    // folder), started from the earlier sign-in when there is one; it's copied into our folder
    // once the browser has closed, so our folder never has to be reachable by the browser.
    let opts = LaunchOptions {
        headless: false,
        profile: earlier.map_or(ProfileChoice::Fresh, |s| ProfileChoice::SeededFrom(s.profile)),
        start_url: Some(url.clone()),
        keeps_sign_in: true,
        ..LaunchOptions::default()
    };
    ctx.report(None, format!("Opening a window on {key}: sign in there, then close the window…"));
    let startup = Deadline::new(Duration::from_secs(60), ctx.cancel.clone());
    let browser = match Browser::launch(ctx, &opts, &startup) {
        Ok(b) => b,
        Err(e) => {
            let window = format!(
                "A sign-in needs a visible browser window: run {SIGN_IN_TOOL} on a computer with a screen, with a Chromium-based browser that isn't a Flatpak or Snap (set its path in Settings → Engines)."
            );
            let fix = match e.remediation.clone() {
                Some(r) => format!("{r} {window}"),
                None => window,
            };
            return Err(e.with_remediation(fix));
        }
    };
    let env = browser_env(ctx.session.clean_room(), &browser);
    let started = Instant::now();
    let ended = wait_for_close(&browser.endpoint, Duration::from_secs(p.timeout_secs), &ctx.cancel);
    close_politely(&browser);
    let kept = browser.profile_dir().map(|p| keep_profile(p, &dir)).unwrap_or_else(|| Err(ToolError::internal("the sign-in browser had no profile")));
    drop(browser);
    kept?;
    let saved_at = now_secs();
    let record = json!({"version": RECORD_VERSION, "site": key, "saved_at": saved_at});
    let bytes = serde_json::to_vec_pretty(&record).map_err(|e| ToolError::internal(e.to_string()))?;
    crate::project::write_atomic(&dir.join(RECORD_FILE), &bytes)?;
    let waited = started.elapsed().as_secs();
    let summary = match ended {
        Ended::Closed => format!("Saved your sign-in for {key}. Websites opened on {key} now start signed in."),
        Ended::TimedOut => format!("The sign-in window for {key} was still open after {} s, so it was closed; what it had saved is kept.", p.timeout_secs),
        Ended::Cancelled => format!("Stopped waiting for the sign-in window for {key}; what it had saved is kept."),
    };
    let mut ev = EvidenceBuilder::new(SIGN_IN_TOOL, Subject { kind: "website".into(), sha256: None, name: Some(key.clone()) }, env)
        .params(json!({"site": key, "timeout_secs": p.timeout_secs}))
        .result(json!({"site": key, "saved": true, "saved_at": saved_at, "ended": ended.id(), "waited_secs": waited, "replaced_earlier": had}))
        .summary(summary)
        .confidence(Confidence::Observed)
        .authority(Authority::OurRun)
        .limitation(Limitation::new(
            "not_checked",
            "Craft Anything never sees your password and never reads cookie or storage values, so it can't tell whether you finished signing in: the next opened page shows it.",
        ))
        .limitation(Limitation::new(
            "session_only",
            "A site that keeps you signed in only until the browser closes (no \"remember me\") isn't kept: like any browser, it forgets that sign-in when the window closes. Choose to stay signed in if the site offers it.",
        ))
        .limitation(Limitation::new(
            "exact_site",
            format!("Used only for pages on {key} (that exact host and port), until the site ends the sign-in or {SIGN_OUT_TOOL} forgets it."),
        ))
        .coverage(Coverage::complete(1))
        .location(Location::new("site", key.clone()));
    if ended != Ended::Closed {
        ev = ev.limitation(Limitation::new(
            "window_stopped",
            "The window was closed by Craft Anything, not by you: a sign-in still in progress may not be complete.",
        ));
    }
    Ok(ToolOutput::evidence(ev.build()?))
}

/// Copy the closed sign-in browser's profile into `dir/profile`, replacing an earlier one only
/// once the new copy is complete.
fn keep_profile(from: &Path, dir: &Path) -> Result<(), ToolError> {
    std::fs::create_dir_all(dir).map_err(|e| ToolError::io(dir.display(), e))?;
    let staging = dir.join("profile.new");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir(&staging).map_err(|e| ToolError::io(staging.display(), e))?;
    if let Err(e) = copy_profile(from, &staging, &CopyCaps::default()) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let profile = dir.join(PROFILE_DIR);
    if std::fs::symlink_metadata(&profile).is_ok() {
        std::fs::remove_dir_all(&profile).map_err(|e| ToolError::io(profile.display(), e))?;
    }
    std::fs::rename(&staging, &profile).map_err(|e| ToolError::io(profile.display(), e))
}

/// Wait until the person closes the browser: it stops answering, or its last page is gone.
fn wait_for_close(endpoint: &cdp::Endpoint, wait: Duration, cancel: &CancelToken) -> Ended {
    let started = Instant::now();
    loop {
        if cancel.is_cancelled() {
            return Ended::Cancelled;
        }
        if started.elapsed() >= wait {
            return Ended::TimedOut;
        }
        let probe = Deadline::new(Duration::from_secs(3), CancelToken::default());
        match cdp::json_list(endpoint, &probe) {
            Err(_) => return Ended::Closed,
            Ok(targets) if !targets.iter().any(|t| t.get("type").and_then(Value::as_str) == Some("page")) => return Ended::Closed,
            Ok(_) => {}
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Ask a browser that's still running to close, so it writes its sign-in to disk, and give it a
/// moment; dropping the `Browser` afterwards stops whatever is left.
fn close_politely(browser: &Browser) {
    let dl = Deadline::new(Duration::from_secs(5), CancelToken::default());
    let Ok(version) = cdp::json_version(&browser.endpoint, &dl) else { return };
    if let Ok(mut client) = web_common::connect_target(browser, &version, 1 << 20, &dl) {
        let _ = client.call("Browser.close", json!({}), None, &dl);
    }
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        let probe = Deadline::new(Duration::from_secs(1), CancelToken::default());
        if cdp::json_version(&browser.endpoint, &probe).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

// ---------------------------------------------------------------------------------------------
// web_site_sign_out

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignOutParams {
    /// An address on the site (http or https); its host and port pick the saved sign-in.
    pub url: String,
}

fn web_site_sign_out(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: SignOutParams = parse_params(params)?;
    let key = site_key(&p.url)?;
    let dir = sign_ins_dir(ctx.session).join(&key);
    let removed = match std::fs::symlink_metadata(&dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(ToolError::io(dir.display(), e)),
        // A link is removed itself; what it points at is left alone.
        Ok(m) if !m.is_dir() => {
            std::fs::remove_file(&dir).map_err(|e| ToolError::io(dir.display(), e))?;
            true
        }
        Ok(_) => {
            std::fs::remove_dir_all(&dir).map_err(|e| ToolError::io(dir.display(), e))?;
            true
        }
    };
    let summary = if removed {
        format!("Forgot the sign-in saved for {key}. Websites opened on {key} start signed out again.")
    } else {
        format!("No sign-in was saved for {key}, so there was nothing to forget.")
    };
    let env = web_common::cdp_env(ctx.session.clean_room(), None);
    let ev = EvidenceBuilder::new(SIGN_OUT_TOOL, Subject { kind: "website".into(), sha256: None, name: Some(key.clone()) }, env)
        .params(json!({"site": key}))
        .result(json!({"site": key, "removed": removed}))
        .summary(summary)
        .confidence(Confidence::Observed)
        .authority(Authority::OurRun)
        .limitation(Limitation::new(
            "this_computer",
            "Only Craft Anything's saved copy is deleted: the site may still count the sign-in as active until it expires.",
        ))
        .coverage(Coverage::complete(1))
        .location(Location::new("site", key))
        .build()?;
    Ok(ToolOutput::evidence(ev))
}

#[cfg(test)]
mod tests;
