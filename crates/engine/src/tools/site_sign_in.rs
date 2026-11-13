//! Saved sign-ins for websites (#1178). `web_site_sign_in` opens a visible browser on a site with
//! a profile folder Craft Anything keeps for that site; the person signs in themselves and closes
//! the window. Later, `web_site_open` on the same site (exact host and port) starts its private
//! browser with a copy of that profile, so the study sees the pages as the person does when signed
//! in. `web_site_sign_out` deletes the saved sign-in.
//!
//! We never see, read or store a password, and never read cookie or storage values: the browser
//! keeps them in its own profile files, which are copied whole except for the browser's own
//! password and card stores: saving those is turned off in these profiles, and any that exist are
//! never copied (see [`is_credential_store`]).

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
/// Longest site key: with the store's own path it stays well under Windows' 260-character limit
/// for a path (most real host names are far shorter).
const MAX_KEY_LEN: usize = 120;
/// Every key starts with this, so no key is a name Windows reserves (`con`, `nul`, `com1`…).
const KEY_PREFIX: &str = "site-";

/// Register the tools for saving and forgetting website sign-ins.
pub fn specs() -> Vec<ToolSpec> {
    vec![
        spec(
            SIGN_IN_TOOL,
            "Sign in to a website",
            "Open a website in a visible browser window with a profile Craft Anything keeps for that site, and wait until you close the window (or timeout_secs passes, default 600, at most 1800). Sign in there yourself: Craft Anything never sees your password and never reads cookie or storage values. Until you forget it with web_site_sign_out, later web_site_open calls for that exact host and effective port (studies and captures included) use a copy of that sign-in by default; signed_in: false starts fresh. An expired or rejected session may still open signed out. The sign-in is kept in Craft Anything's folder on this computer: anyone who can open your files here could use it. When a sign-in is already saved for the site, the window opens with it, and what you leave when you close the window replaces it: to use another account, sign out on the site in the window first, or forget the saved one with web_site_sign_out. Needs a desktop where a window can open.",
            "Sign in to a site once yourself, so its pages can be studied as you see them. The sign-in is kept on this computer, and used for that site until you forget it.",
            Toolset::Browser,
            Effects::EVIDENCE
                .union(Effects::EXECUTES_FOREIGN_CODE)
                .union(Effects::ACCESSES_NETWORK)
                .union(Effects::LAUNCHES_PROCESS)
                .union(Effects::WRITES_FILESYSTEM)
                .union(Effects::MAY_DISCARD_DATA),
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

/// The folder name a site's saved sign-in lives under: `site-` and the URL's host in lower case,
/// plus its effective `_port` (omitted ports resolve to 80 for HTTP and 443 for HTTPS):
/// `site-example.com_443`, `site-app.example.com_8443`, `site-ip6_--1_80` for `http://[::1]`. Only `a-z`, `0-9`, `-`, `.` and `_` are allowed, so a key is always one
/// safe path component; a host with anything else (a Unicode name: give its `xn--` form) is refused,
/// never rewritten, so two sites can't share a key.
pub fn site_key(url: &str) -> Result<String, ToolError> {
    let u = check_url(url)?;
    let (rest, default_port) = match u.strip_prefix("https://") {
        Some(r) => (r, 443u16),
        None => (u.strip_prefix("http://").unwrap_or(u), 80u16),
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
    // One trailing dot is the same host written fully; more is no host.
    let host = host.strip_suffix('.').unwrap_or(&host);
    // `_` only ever comes from the IPv6 prefix above, so names and addresses can't collide.
    let name_ok = |h: &str| h.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
    let host_ok = !host.is_empty()
        && (name_ok(host) || host.strip_prefix("ip6_").is_some_and(|a| a.chars().all(|c| c.is_ascii_hexdigit() || c == '-')))
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains("..");
    if !host_ok {
        return Err(bad());
    }
    let port = match port {
        None | Some("") => default_port,
        Some(p) => p.parse::<u16>().ok().filter(|n| *n != 0 && p.chars().all(|c| c.is_ascii_digit())).ok_or_else(bad)?,
    };
    let key = format!("{KEY_PREFIX}{host}_{port}");
    if key.len() > MAX_KEY_LEN {
        return Err(bad());
    }
    Ok(key)
}

/// Where saved sign-ins live: a folder in this session's own config folder.
pub fn sign_ins_dir(session: &Session) -> PathBuf {
    session.config_dir().join("sign-ins")
}

/// Reserve one site's saved profile without waiting while another tool holds the session.
pub(crate) fn lock_site(session: &Session, key: &str) -> Result<std::fs::File, ToolError> {
    // Keep lock files outside deletable profiles; unlinking them could split the reservation.
    let directory = sign_ins_dir(session).join(".locks");
    private_dirs(&directory)?;
    let path = directory.join(format!("{key}.lock"));
    let file =
        std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path).map_err(|error| ToolError::io(path.display(), error))?;
    owner_only(&path, 0o600)?;
    // A program another thread is starting holds a copy of every open file until its exec closes
    // them, so a reservation just released can still look held for a moment: try again briefly
    // before calling the site busy. A real holder (a sign-in window) outlasts this by far.
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < LOCK_GRACE => std::thread::sleep(Duration::from_millis(5)),
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(ToolError::new(ErrorCode::TimedOut, format!("Another browser operation is using the saved sign-in for {key}."))
                    .with_remediation("Close that site's sign-in window or wait for its browser operation to finish, then try again."));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(ToolError::io(path.display(), error)),
        }
    }
}

/// How long [`lock_site`] keeps trying before it reports a site busy.
const LOCK_GRACE: Duration = Duration::from_millis(250);

/// A sign-in saved for one site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSignIn {
    pub key: String,
    /// The browser profile folder to copy.
    pub profile: PathBuf,
    /// When it was saved, in seconds since 1970.
    pub saved_at: u64,
    /// Opaque identity of this saved profile, changed on every save.
    pub revision: String,
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
    meta.is_dir().then(|| {
        let saved_at = record.get("saved_at").and_then(Value::as_u64).unwrap_or(0);
        let revision = record.get("revision").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| saved_at.to_string());
        SavedSignIn { key: key.to_string(), profile, saved_at, revision }
    })
}

/// Largest `Preferences` file read back before changing it.
const MAX_PREFERENCES_BYTES: u64 = 8 << 20;

/// The browser profile (inside the user-data folder) every sign-in and seeded browser starts in:
/// the one [`prepare_sign_in_profile`] sets up, chosen with `--profile-directory`.
pub(crate) const SIGN_IN_PROFILE: &str = "Default";

/// Prepare the browser profile at `profile` for a sign-in: keep cookies that last only until the
/// browser closes, the way a browser does when it's set to continue where it left off
/// (`session.restore_on_startup` = 1 in `Default/Preferences`), and turn off saving passwords,
/// addresses and cards, so the browser never offers to keep them in a profile we copy. The rest of
/// an existing Preferences file is kept; one that is unreadable, too big or not a JSON object is
/// replaced.
pub fn prepare_sign_in_profile(profile: &Path) -> Result<(), ToolError> {
    let dir = profile.join(SIGN_IN_PROFILE);
    std::fs::create_dir_all(&dir).map_err(|e| ToolError::io(dir.display(), e))?;
    let path = dir.join("Preferences");
    let existing = std::fs::symlink_metadata(&path)
        .ok()
        .filter(|m| m.is_file() && m.len() <= MAX_PREFERENCES_BYTES)
        .and_then(|_| std::fs::read(&path).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let mut prefs = match existing {
        Some(Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    let session = prefs.entry("session").or_insert_with(|| json!({}));
    if !session.is_object() {
        *session = json!({});
    }
    if let Value::Object(s) = session {
        s.insert("restore_on_startup".into(), json!(1));
    }
    prefs.insert("credentials_enable_service".into(), json!(false));
    prefs.insert("credentials_enable_autosignin".into(), json!(false));
    for (section, key) in [("profile", "password_manager_enabled"), ("autofill", "profile_enabled"), ("autofill", "credit_card_enabled")] {
        let s = prefs.entry(section).or_insert_with(|| json!({}));
        if !s.is_object() {
            *s = json!({});
        }
        if let Value::Object(s) = s {
            s.insert(key.into(), json!(false));
        }
    }
    let bytes = serde_json::to_vec(&Value::Object(prefs)).map_err(|e| ToolError::internal(e.to_string()))?;
    crate::project::write_atomic(&path, &bytes)
}

/// Seconds since the Unix epoch for the saved sign-in's display timestamp.
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
    /// Bound profile copies to 512 MiB, 20,000 entries and 16 nested folders.
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
    // The tabs that were open: a copy must not reopen them (its session cookies are kept anyway).
    "Sessions",
];

/// Files that belong to one running browser: its locks and the port it listened on.
const SKIPPED_FILES: &[&str] = &[
    "SingletonLock",
    "SingletonSocket",
    "SingletonCookie",
    "lockfile",
    "DevToolsActivePort",
    "RunningChromeVersion",
    "Current Session",
    "Current Tabs",
    "Last Session",
    "Last Tabs",
];

/// The browser's own stores of passwords, addresses and cards (SQLite files and their `-journal`,
/// `-wal` and `-shm` companions). Saving to them is off in our profiles, but a profile saved before
/// that, or a browser that ignores the setting, may still hold them: they are never copied.
const CREDENTIAL_STORES: &[&str] = &["Login Data", "Web Data", "Account Web Data"];

/// Whether a profile file is one of [`CREDENTIAL_STORES`] or a companion of one.
pub(crate) fn is_credential_store(name: &str) -> bool {
    CREDENTIAL_STORES.iter().any(|store| name.strip_prefix(store).is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '-'])))
}

/// Delete the credential stores a saved profile kept before they were left out of copies. Only the
/// profile's top level and its browser profiles (`Default`, `Profile 1`, …) hold them; links are
/// never followed. Best effort: a seeded copy skips them anyway.
pub(crate) fn remove_credential_stores(profile: &Path) {
    let mut dirs = vec![profile.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(profile) {
        dirs.extend(entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).take(1_000));
    }
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten().take(20_000) {
            if e.file_type().is_ok_and(|t| t.is_file()) && is_credential_store(&e.file_name().to_string_lossy()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Make a folder only its owner can open. On Windows the folder keeps the access rules of the
/// person's own config folder, which already keeps other users out.
fn private_dir(path: &Path) -> Result<(), ToolError> {
    std::fs::create_dir(path).map_err(|e| ToolError::io(path.display(), e))?;
    owner_only(path, 0o700)
}

/// Owner-only permissions on Unix (`mode`); on Windows the person's config folder's access rules apply.
fn owner_only(path: &Path, mode: u32) -> Result<(), ToolError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| ToolError::io(path.display(), e))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// `dir` and its parent (the store) made if missing, both owner-only.
fn private_dirs(dir: &Path) -> Result<(), ToolError> {
    std::fs::create_dir_all(dir).map_err(|e| ToolError::io(dir.display(), e))?;
    if let Some(store) = dir.parent() {
        owner_only(store, 0o700)?;
    }
    owner_only(dir, 0o700)
}

/// A name no other call in this process (or another) is using right now.
fn unique(prefix: &str) -> String {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!("{prefix}-{}-{}-{nanos}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// Copy the saved profile `from` into the empty folder `to`, skipping caches, lock files and the
/// browser's password and card stores, and never following a link (links are left out). Going over a cap is an error, not a partial copy.
pub fn copy_profile(from: &Path, to: &Path, caps: &CopyCaps) -> Result<(), ToolError> {
    let over = |what: String| {
        ToolError::new(ErrorCode::OverLimit, format!("The sign-in profile is too big to copy: {what}.")).with_remediation(format!(
            "Forget the saved sign-in with {SIGN_OUT_TOOL} and sign in again with {SIGN_IN_TOOL}, visiting only the site you want to study."
        ))
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
                files = files.saturating_add(1);
                if files > caps.max_files {
                    return Err(over(format!("more than {} files and folders", caps.max_files)));
                }
                private_dir(&target)?;
                stack.push((path, target, depth.saturating_add(1)));
            } else if meta.is_file() {
                if SKIPPED_FILES.contains(&name_str.as_ref()) || is_credential_store(&name_str) {
                    continue;
                }
                files = files.saturating_add(1);
                bytes = bytes.saturating_add(meta.len());
                if files > caps.max_files {
                    return Err(over(format!("more than {} files and folders", caps.max_files)));
                }
                if bytes > caps.max_bytes {
                    return Err(over(format!("more than {} MiB", caps.max_bytes >> 20)));
                }
                std::fs::copy(&path, &target).map_err(|e| ToolError::io(path.display(), e))?;
                owner_only(&target, 0o600)?;
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

/// Default time allowed for the person to finish signing in.
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
    /// Stable result identifier for how the sign-in window ended.
    fn id(self) -> &'static str {
        match self {
            Ended::Closed => "closed",
            Ended::TimedOut => "timed_out",
            Ended::Cancelled => "cancelled",
        }
    }
}

/// On Linux and the other Unix systems (not macOS) a window needs a desktop session; without one (a server, a remote shell) say so
/// before anything starts.
fn no_display_error(display: Option<String>, wayland: Option<String>) -> Option<ToolError> {
    let has = |v: Option<String>| v.is_some_and(|s| !s.trim().is_empty());
    (cfg!(all(unix, not(target_os = "macos"))) && !has(display) && !has(wayland)).then(|| {
        ToolError::new(ErrorCode::NotSupported, "There's no desktop here to open a sign-in window on.")
            .with_remediation(format!("Run {SIGN_IN_TOOL} from Craft Anything on a computer with a screen (the app, or the CLI in a desktop session). Websites still open without signing in."))
    })
}

/// Save the profile left by a visible sign-in window and check it on the site.
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
    let _reservation = lock_site(ctx.session, &key)?;
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
    let mut browser = browser;
    let env = browser_env(ctx.session.clean_room(), &browser);
    let started = Instant::now();
    let Some(profile) = browser.profile_dir().map(Path::to_path_buf) else {
        return Err(ToolError::internal("the sign-in browser had no profile"));
    };
    let ended = wait_for_close(&browser.endpoint, &profile, Duration::from_secs(p.timeout_secs), &ctx.cancel);
    // The profile is copied only once the browser has gone: it writes its cookies as it exits.
    let closed = close_and_wait(&mut browser, &profile);
    let killed = closed == BrowserClosed::Stopped;
    let kept = keep_closed_profile(closed, &profile, &dir);
    drop(browser);
    kept?;
    // Open the site once more the way web_site_open will, to see whether the sign-in was kept.
    ctx.report(None, format!("Checking that {key} opens signed in…"));
    let checked = check_saved(ctx, &url, &dir);
    let (check, check_note) = check_result(checked);
    let saved_at = now_secs();
    let record = json!({"version": RECORD_VERSION, "site": key, "saved_at": saved_at, "revision": unique("saved")});
    let bytes = serde_json::to_vec_pretty(&record).map_err(|e| ToolError::internal(e.to_string()))?;
    crate::project::write_atomic(&dir.join(RECORD_FILE), &bytes)?;
    let waited = started.elapsed().as_secs();
    let summary = match ended {
        Ended::Closed => format!("Saved your sign-in for {key}. Websites opened on {key} now use it by default, if the site still accepts it."),
        Ended::TimedOut => format!("The sign-in window for {key} was still open after {} s, so it was closed; what it had saved is kept.", p.timeout_secs),
        Ended::Cancelled => format!("Stopped waiting for the sign-in window for {key}; what it had saved is kept."),
    };
    let mut ev = EvidenceBuilder::new(SIGN_IN_TOOL, Subject { kind: "website".into(), sha256: None, name: Some(key.clone()) }, env)
        .params(json!({"site": key, "timeout_secs": p.timeout_secs}))
        .result(json!({"site": key, "saved": true, "saved_at": saved_at, "ended": ended.id(), "waited_secs": waited, "replaced_earlier": had, "check": check}))
        .summary(summary)
        .confidence(Confidence::Observed)
        .authority(Authority::OurRun)
        .limitation(Limitation::new(
            "not_checked",
            "Craft Anything never sees your password and never reads cookie or storage values: whether you finished signing in is judged only by opening the site once more (`check`).",
        ))
        .limitation(Limitation::new(
            "session_cookies",
            "Sign-ins that last only while the browser is open (no \"remember me\") are kept too, but a site may still end them sooner than a remembered one.",
        ))
        .limitation(Limitation::new(
            "kept_here",
            format!("The sign-in is kept in Craft Anything's folder on this computer: anyone who can open your files here could use it. Later web_site_open calls for {key} use it by default until {SIGN_OUT_TOOL} forgets it; signed_in: false starts fresh, and the site may expire or reject the session."),
        ))
        .limitation(Limitation::new(
            "exact_site",
            format!("It's used only when that exact host and port is opened, but it may also hold the sign-ins of other sites you passed through while signing in (a single sign-on provider). It lasts until the site ends it or {SIGN_OUT_TOOL} forgets it."),
        ))
        .coverage(Coverage::complete(1))
        .location(Location::new("site", key.clone()));
    ev = ev.limitations(check_note);
    if killed {
        ev = ev.limitation(Limitation::new(
            "browser_stopped",
            "The browser didn't finish closing by itself and was stopped, so the latest part of the sign-in may not have been saved. If pages open signed out, sign in again.",
        ));
    }
    if ended != Ended::Closed {
        ev = ev.limitation(Limitation::new(
            "window_stopped",
            "The window was closed by Craft Anything, not by you: a sign-in still in progress may not be complete.",
        ));
    }
    Ok(ToolOutput::evidence(ev.build()?))
}

/// Where the check's picture of the page goes: the site's own sign-in folder, owner-only, never
/// a place that's committed or sent on (it shows the person's own account).
const CHECK_PICTURE: &str = "check.png";

/// What opening the site with the just-saved sign-in showed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Checked {
    /// The page still asks for a password (the same test as a capture's `sign_in_page`).
    pub sign_in_page: bool,
    /// Where it ended up, without query or fragment.
    pub url: String,
    /// The picture: its path, sha256, width and height.
    pub picture: Value,
}

/// Does a form on the page ask for a password? (A capture's `sign_in_page` reads the same
/// field types from its form outline.)
const PASSWORD_JS: &str =
    "Array.from(document.forms).some(f => Array.from(f.elements).some(e => e.type === 'password')) || !!document.querySelector('input[type=password]')";

/// Open `url` headless with a copy of the sign-in saved in `dir`, as web_site_open does, and look.
fn check_saved(ctx: &mut Ctx<'_>, url: &str, dir: &Path) -> Result<Checked, ToolError> {
    let dl = Deadline::new(Duration::from_secs(45), ctx.cancel.clone());
    let opts = LaunchOptions { profile: ProfileChoice::SeededFrom(dir.join(PROFILE_DIR)), ..LaunchOptions::default() };
    let browser = Browser::launch(ctx, &opts, &dl)?;
    let targets = cdp::json_list(&browser.endpoint, &dl)?;
    let target = web_common::pick_target(&targets, None, Some("page"))?;
    let mut client = web_common::connect_target(&browser, target, craft_anything_engines::cdp::PAGE_MAX_MESSAGE, &dl)?;
    let r = (|| -> Result<(String, bool, Vec<u8>), ToolError> {
        super::website_tools::navigate(&mut client, url, 1280, 900, 1500, &dl)?;
        let here = super::website_tools::eval(&mut client, "location.href", &dl)?.as_str().unwrap_or_default().to_string();
        let password = super::website_tools::eval(&mut client, PASSWORD_JS, &dl)? == Value::Bool(true);
        let shot = client.call("Page.captureScreenshot", json!({"format": "png"}), None, &dl)?;
        let data = shot.get("data").and_then(Value::as_str).ok_or_else(|| ToolError::new(ErrorCode::GarbledOutput, "the browser returned no screenshot"))?;
        Ok((here, password, web_common::b64_decode(data)?))
    })();
    super::browser_tools::cleanup(&mut client, &dl);
    drop(browser);
    let (here, sign_in_page, png) = r?;
    let (width, height) = web_common::png_dimensions(&png)?;
    let path = dir.join(CHECK_PICTURE);
    crate::project::write_atomic(&path, &png)?;
    owner_only(&path, 0o600)?;
    let picture =
        json!({"path": path.display().to_string(), "sha256": craft_anything_evidence::sha256_hex(&png), "width": width, "height": height, "bytes": png.len()});
    Ok(Checked { sign_in_page, url: super::website_tools::shown_url(&here), picture })
}

/// The check as a typed result (`done`, `sign_in_page`, `url`, `picture`, `error`) and what it
/// means for the person. A check that couldn't run never fails the sign-in or discards it.
pub(crate) fn check_result(r: Result<Checked, ToolError>) -> (Value, Vec<Limitation>) {
    match r {
        Ok(c) => {
            let mut notes = vec![Limitation::new(
                "check_picture",
                format!(
                    "{} is a picture of the site opened with your sign-in, so it shows your own account. It stays in Craft Anything's folder on this computer and is deleted by {SIGN_OUT_TOOL}.",
                    c.picture["path"].as_str().unwrap_or(CHECK_PICTURE)
                ),
            )];
            if c.sign_in_page {
                notes.push(Limitation::new(
                    "sign_in_not_kept",
                    "Opened with the saved sign-in, the site may still be asking you to sign in (it shows a password field), so the sign-in may not have been kept: some sites tie a sign-in to the browser it was made in. Sign in again and tick \"remember me\" or \"stay signed in\" if the site offers it.",
                ));
            }
            (json!({"done": true, "sign_in_page": c.sign_in_page, "url": c.url, "picture": c.picture, "error": null}), notes)
        }
        Err(e) => (
            json!({"done": false, "sign_in_page": null, "url": null, "picture": null, "error": e.message}),
            vec![Limitation::new(
                "not_checked_after",
                format!(
                    "The sign-in was saved, but opening the site again to check it didn't work ({}), so whether it opens signed in isn't known yet.",
                    e.message
                ),
            )],
        ),
    }
}

/// Copy the closed sign-in browser's profile into `dir/profile`. The copy is made under a name of
/// its own and swapped in only when complete (the old one is moved aside first, then deleted), so
/// a failure never leaves half a sign-in. The caller holds the site's reservation, so folders
/// left by an earlier run that stopped part-way can be deleted first.
pub(crate) fn keep_profile(from: &Path, dir: &Path) -> Result<(), ToolError> {
    private_dirs(dir)?;
    remove_leftovers(dir);
    let staging = dir.join(unique("profile.new"));
    private_dir(&staging)?;
    if let Err(e) = copy_profile(from, &staging, &CopyCaps::default()) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e.with_remediation(format!(
            "Sign in again with {SIGN_IN_TOOL}, visiting only the site you want to study, and close the window when you're signed in."
        )));
    }
    let profile = dir.join(PROFILE_DIR);
    let old = dir.join(unique("profile.old"));
    let moved_aside = std::fs::rename(&profile, &old).is_ok();
    if let Err(e) = std::fs::rename(&staging, &profile) {
        let _ = std::fs::remove_dir_all(&staging);
        if moved_aside {
            let _ = std::fs::rename(&old, &profile);
        }
        return Err(ToolError::io(profile.display(), e));
    }
    if moved_aside {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
}

/// Delete `profile.new-*` and `profile.old-*` folders a run that stopped part-way left in `dir`.
/// The caller holds the site's reservation, so no other save can still be using these folders.
fn remove_leftovers(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let leftover = name.starts_with("profile.new-") || name.starts_with("profile.old-");
        if leftover && std::fs::symlink_metadata(e.path()).is_ok_and(|m| m.is_dir()) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// The browser holding `profile` has exited: it removes its profile lock as the last step of
/// closing, after writing cookies (`SingletonLock` on Unix; on Windows `lockfile` stays locked
/// until then).
fn profile_released(profile: &Path) -> bool {
    if cfg!(windows) {
        let lock = profile.join("lockfile");
        std::fs::symlink_metadata(&lock).is_err() || std::fs::remove_file(&lock).is_ok()
    } else {
        std::fs::symlink_metadata(profile.join("SingletonLock")).is_err()
    }
}

/// Whether the browser has created its platform-specific profile lock.
fn profile_locked(profile: &Path) -> bool {
    if cfg!(windows) { profile.join("lockfile").exists() } else { std::fs::symlink_metadata(profile.join("SingletonLock")).is_ok() }
}

/// What one look at the sign-in browser saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    /// It has exited (its profile lock is gone).
    Exited,
    /// It didn't answer.
    Silent,
    /// It answered with no page open.
    NoPages,
    Open,
}

/// Looks in a row that mean the window was closed: the browser exited (once is enough, when its
/// lock was seen while it ran), or it went silent or showed no pages in every look for 2 s (one
/// slow answer, or a moment between pages while signing in, isn't a close).
#[derive(Debug, Default)]
struct CloseWatch {
    since: Option<Instant>,
    looks: u32,
}

impl CloseWatch {
    const LOOKS: u32 = 3;
    const SPAN: Duration = Duration::from_secs(2);

    /// Confirm an exit immediately, or require repeated missing-page observations over time.
    fn closed(&mut self, look: Look, now: Instant) -> bool {
        match look {
            Look::Exited => true,
            Look::Open => {
                *self = CloseWatch::default();
                false
            }
            Look::Silent | Look::NoPages => {
                let since = *self.since.get_or_insert(now);
                self.looks = self.looks.saturating_add(1);
                self.looks >= Self::LOOKS && now.saturating_duration_since(since) >= Self::SPAN
            }
        }
    }
}

/// Wait until the person closes the browser, the time is up or the call is stopped.
fn wait_for_close(endpoint: &cdp::Endpoint, profile: &Path, wait: Duration, cancel: &CancelToken) -> Ended {
    let started = Instant::now();
    let mut watch = CloseWatch::default();
    let mut lock_seen = false;
    loop {
        if cancel.is_cancelled() {
            return Ended::Cancelled;
        }
        if started.elapsed() >= wait {
            return Ended::TimedOut;
        }
        lock_seen |= profile_locked(profile);
        let probe = Deadline::new(Duration::from_secs(3), CancelToken::default());
        let look = if lock_seen && profile_released(profile) {
            Look::Exited
        } else {
            match cdp::json_list(endpoint, &probe) {
                Err(_) => Look::Silent,
                Ok(targets) if !targets.iter().any(|t| t.get("type").and_then(Value::as_str) == Some("page")) => Look::NoPages,
                Ok(_) => Look::Open,
            }
        };
        if watch.closed(look, Instant::now()) {
            return Ended::Closed;
        }
        std::thread::sleep(Duration::from_millis(700));
    }
}

/// How the sign-in browser ended, as far as we could confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserClosed {
    /// It exited by itself and released its profile.
    Itself,
    /// It didn't finish closing, so it was stopped, and the stop was confirmed.
    Stopped,
    /// It was still running after being stopped: its profile may still be changing.
    Unconfirmed,
}

/// Copy the sign-in browser's profile into `dir` only when the browser is confirmed gone. A
/// browser that may still be writing would leave a half-written copy that replaces the earlier
/// sign-in, so then nothing is copied, the earlier one stays as it was, and the save fails.
fn keep_closed_profile(closed: BrowserClosed, from: &Path, dir: &Path) -> Result<(), ToolError> {
    if closed == BrowserClosed::Unconfirmed {
        return Err(ToolError::unavailable("The sign-in window wouldn't close, so its sign-in wasn't saved.")
            .with_remediation(format!("Close the window yourself, then run {SIGN_IN_TOOL} again. Any sign-in saved before is unchanged.")));
    }
    keep_profile(from, dir)
}

/// Ask the browser to close if it's still running, so it writes its sign-in to disk, and wait
/// (at most 20 s) until it has exited. One that doesn't is stopped, and the result says whether
/// that stop was confirmed.
fn close_and_wait(browser: &mut Browser, profile: &Path) -> BrowserClosed {
    let dl = Deadline::new(Duration::from_secs(5), CancelToken::default());
    if let Ok(version) = cdp::json_version(&browser.endpoint, &dl)
        && let Ok(mut client) = web_common::connect_target(browser, &version, 1 << 20, &dl)
    {
        let _ = client.call("Browser.close", json!({}), None, &dl);
    }
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        let probe = Deadline::new(Duration::from_secs(1), CancelToken::default());
        if profile_released(profile) && cdp::json_version(&browser.endpoint, &probe).is_err() {
            return BrowserClosed::Itself;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if browser.stop_and_wait(Duration::from_secs(10)) { BrowserClosed::Stopped } else { BrowserClosed::Unconfirmed }
}

// ---------------------------------------------------------------------------------------------
// web_site_sign_out

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignOutParams {
    /// An address on the site (http or https); its host and port pick the saved sign-in.
    pub url: String,
}

/// Forget only the requested site's saved profile, without following a store entry's symlink.
fn web_site_sign_out(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: SignOutParams = parse_params(params)?;
    let key = site_key(&p.url)?;
    let _reservation = lock_site(ctx.session, &key)?;
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
