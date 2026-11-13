//! The Craft Anything engine: one session, one registry of tools, one way in.
//!
//! The desktop UI, the CLI, the MCP server and the built-in AI agent all call [`Engine::call`]
//! with a tool id and JSON params. The engine checks availability and consent, runs the tool,
//! records the evidence it returns, and turns any escaped panic into an error.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod clean_room;

/// Tools an agent app may be told to approve on its own side (Codex's `approval_mode`): each one
/// asks the person itself (`Effects::needs_consent`), so approving it there skips nothing.
/// Everything else keeps the app's own policy; a test in the AI crate checks each is registered
/// and asks for consent.
pub const AGENT_APP_PREAPPROVED_TOOLS: &[&str] = &["rebuild_start"];
pub mod feed;
pub mod inbox;
pub mod journal;
pub mod mechanics;
pub mod outputs;
pub mod pins;
pub mod plugins;
pub mod project;
pub mod questions;
pub mod registry;
pub mod self_test;
pub mod session;
pub mod settings;
pub mod target;
#[doc(hidden)]
pub mod testing;
pub mod tool;
pub mod tool_list;
pub mod toolchains;
pub mod tools;
pub mod versioned;
pub mod wasm_plugins;
pub mod workspace;

pub use craft_anything_evidence as evidence;
pub use registry::Registry;
pub use session::Session;
pub use settings::{Mode, Settings};
pub use target::{Target, TargetKind};
pub use tool::{AGENT_ENV, Caller, CancelToken, Ctx, Effects, Progress, Requires, ToolOutput, ToolSpec, Toolset};

use craft_anything_evidence::{ErrorCode, ToolError};
use craft_anything_platform::sync::{Mutex, MutexGuard};
use serde_json::Value;
use std::sync::PoisonError;

/// Whether the caller has the user's consent for a tool that runs code or changes a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Consent {
    /// Not asked: the engine refuses consent-gated tools unless the project or settings allow them.
    #[default]
    NotAsked,
    /// The user approved this one call.
    Granted,
}

/// Options for one call.
pub struct CallOptions<'a> {
    pub caller: Caller,
    pub consent: Consent,
    pub cancel: CancelToken,
    pub progress: &'a tool::ProgressFn,
}

fn no_progress(_: Progress) {}

impl CallOptions<'static> {
    pub fn new(caller: Caller) -> Self {
        CallOptions { caller, consent: Consent::NotAsked, cancel: CancelToken::default(), progress: &no_progress }
    }
}

impl<'a> CallOptions<'a> {
    pub fn with_consent(mut self, consent: Consent) -> Self {
        self.consent = consent;
        self
    }

    pub fn with_cancel(mut self, cancel: CancelToken) -> Self {
        self.cancel = cancel;
        self
    }

    pub fn with_progress<'b>(self, progress: &'b tool::ProgressFn) -> CallOptions<'b> {
        CallOptions { caller: self.caller, consent: self.consent, cancel: self.cancel, progress }
    }
}

/// Why a tool can't run right now, in words a beginner understands, plus how to fix it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Availability {
    pub available: bool,
    pub reason: Option<String>,
    pub remediation: Option<String>,
}

impl Availability {
    fn ok() -> Self {
        Availability { available: true, reason: None, remediation: None }
    }

    pub(crate) fn no(reason: impl Into<String>, remediation: impl Into<String>) -> Self {
        Availability { available: false, reason: Some(reason.into()), remediation: Some(remediation.into()) }
    }
}

/// The engine. Cheap to share behind an `Arc`.
pub struct Engine {
    registry: Registry,
    session: Mutex<Session>,
    /// Sign-in evidence that couldn't reach the ledger because Stop came while another tool held
    /// the session, with the project root it belongs to. Taken in whenever the session is next
    /// locked (see [`Engine::settle`]). Locked only briefly, and never while waiting for the session.
    pending: Mutex<Vec<(Option<std::path::PathBuf>, craft_anything_evidence::Evidence)>>,
}

/// A handle to the engine a tool runs in, for tools that start work which calls back into it (a
/// rebuild run). Only set on engines made with [`Engine::shared`]; it never keeps the engine alive.
pub struct EngineRef(pub std::sync::Weak<Engine>);

impl Engine {
    pub fn new(session: Session) -> Self {
        Engine { registry: Registry::with_all_tools(), session: Mutex::new(session), pending: Mutex::new(Vec::new()) }
    }

    /// The engine as most apps use it: shared, with tools from higher layers (`extra`, e.g. the
    /// AI layer's rebuild tools) registered next to the engine's own, and an [`EngineRef`] in the
    /// session so those tools can reach it. A duplicate id keeps the engine's own tool.
    pub fn shared(session: Session, extra: Vec<ToolSpec>) -> std::sync::Arc<Engine> {
        Engine::shared_with(session, extra, Vec::new())
    }

    /// [`Engine::shared`] plus tools defined at run time (a plugin's, an external MCP server's):
    /// owned names and a JSON schema, served by a function that can tell them apart by
    /// [`Ctx::tool`]. Invalid or duplicate ones are skipped and logged.
    pub fn shared_with(session: Session, extra: Vec<ToolSpec>, runtime: Vec<registry::RuntimeTool>) -> std::sync::Arc<Engine> {
        let mut tools = tools::all();
        tools.extend(extra);
        std::sync::Arc::new_cyclic(|me| {
            let mut session = session;
            session.ext.insert(std::sync::Arc::new(EngineRef(me.clone())));
            Engine { registry: Registry::new(tools).with_runtime(runtime), session: Mutex::new(session), pending: Mutex::new(Vec::new()) }
        })
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Lock the session (recovering from a poisoned lock rather than cascading a panic).
    #[track_caller]
    pub fn session(&self) -> MutexGuard<'_, Session> {
        let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        self.settle(&mut session);
        session
    }

    /// Try to lock without blocking (the UI uses this so a long tool never freezes a frame).
    #[track_caller]
    pub fn try_session(&self) -> Option<MutexGuard<'_, Session>> {
        let mut session = match self.session.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return None,
        };
        self.settle(&mut session);
        Some(session)
    }

    /// Take pending sign-in evidence into `session`'s ledger when it belongs to the project open
    /// now (or to no project, with none open). Evidence for another project is already in that
    /// project's store, so it's dropped from memory here, not recorded in the wrong ledger.
    fn settle(&self, session: &mut Session) {
        let pending = std::mem::take(&mut *self.pending.lock().unwrap_or_else(PoisonError::into_inner));
        let open = session.project.as_ref().map(|p| p.root.clone());
        for (root, ev) in pending {
            if root == open
                && let Err(e) = session.record(ev)
            {
                log::warn!("couldn't record a stopped sign-in's evidence: {}", e.message);
            }
        }
    }

    pub fn availability(&self, spec: &ToolSpec, session: &Session) -> Availability {
        if let Some(no) = clean_room::availability(spec.id, session.clean_room()) {
            return no;
        }
        match spec.requires {
            Requires::Nothing => Availability::ok(),
            Requires::Target => {
                if session.active_id().is_some() {
                    Availability::ok()
                } else {
                    Availability::no("Nothing is open yet.", "Open a file, app or folder first.")
                }
            }
            Requires::Ghidra => {
                if !session.clean_room() {
                    Availability::ok()
                } else {
                    Availability::no("This needs Deep mode (with Ghidra), and you're in Clean room mode.", "Turn on Deep mode in the title bar or Settings.")
                }
            }
            Requires::Engine(id) => match session.settings.engine_path(id) {
                Some(_) => Availability::ok(),
                None => {
                    let (reason, remediation) = session.settings.engine_not_found(id);
                    Availability::no(reason, remediation)
                }
            },
            Requires::Os(hosts) => {
                if hosts.contains(&std::env::consts::OS) {
                    Availability::ok()
                } else {
                    let names = hosts.iter().map(|h| match *h {
                        "macos" => "macOS",
                        "windows" => "Windows",
                        "linux" => "Linux",
                        other => other,
                    });
                    let names = names.collect::<Vec<_>>().join(" or ");
                    Availability::no(
                        format!("This needs {names} and is not available on this host ({}).", craft_anything_evidence::host_triple()),
                        format!("Run it on {names}."),
                    )
                }
            }
        }
    }

    /// Run a tool by id. When a project is open, the call is appended to its journal.
    pub fn call(&self, id: &str, params: Value, opts: CallOptions<'_>) -> Result<ToolOutput, ToolError> {
        let started = std::time::Instant::now();
        let logged_params = journal::bounded(&params);
        let caller = opts.caller;
        let mut journal_at = JournalAt::Live;
        let result = self.call_inner(id, params, opts, &mut journal_at);
        let outcome = match &result {
            Ok(ToolOutput::Evidence(ev)) => serde_json::json!({"ok": true, "evidence_id": ev.id}),
            Ok(ToolOutput::Value(_)) => serde_json::json!({"ok": true}),
            Err(e) => serde_json::json!({"ok": false, "error_code": e.code, "error": e.message}),
        };
        let mut entry = serde_json::json!({"tool": id, "caller": caller, "params": logged_params, "duration_ms": started.elapsed().as_millis() as u64});
        if let (Some(m), serde_json::Value::Object(o)) = (entry.as_object_mut(), outcome) {
            m.extend(o);
        }
        match journal_at {
            JournalAt::Live => self.journal("tool_call", entry),
            // Written without the session: the call may have been stopped while another tool holds it.
            JournalAt::Project(Some(root)) => journal::append(&root, "tool_call", entry),
            JournalAt::Project(None) => {}
        }
        result
    }

    /// Append an entry to the open project's journal (no-op without a project). Never fails.
    pub fn journal(&self, kind: &str, entry: Value) {
        let root = self.session().project.as_ref().map(|p| p.root.clone());
        if let Some(root) = root {
            journal::append(&root, kind, entry);
        }
    }

    fn call_inner(&self, id: &str, params: Value, opts: CallOptions<'_>, journal_at: &mut JournalAt) -> Result<ToolOutput, ToolError> {
        let spec = *self.registry.get(id).ok_or_else(|| {
            ToolError::invalid(format!("No tool named {id}")).with_remediation("List tools with `craft-anything-cli commands` or the command palette.")
        })?;
        // Stop works while waiting for the session, and a holder that looks stuck is an error here,
        // not a wait forever. Tools are long jobs: the label says so (they have their own deadlines).
        let cancel = opts.cancel.clone();
        let mut session = match self.session.lock_unless(&|| cancel.is_cancelled()) {
            Ok(g) => {
                let mut g = g.unwrap_or_else(PoisonError::into_inner);
                self.settle(&mut g);
                g
            }
            Err(craft_anything_platform::sync::GaveUp::Stopped) => return Err(ToolError::cancelled()),
            Err(craft_anything_platform::sync::GaveUp::Stuck(why)) => {
                return Err(ToolError::new(ErrorCode::TimedOut, format!("{} couldn't start because another part of the app is stuck.", spec.title))
                    .with_remediation("Restart the app; your work is kept.")
                    .with_details(serde_json::json!({"lock": why})));
            }
        };
        session.label(spec.id);
        if session.clean_room() && !clean_room::allows(spec.id) {
            return Err(clean_room::refusal(spec.id));
        }
        let avail = self.availability(&spec, &session);
        if !avail.available {
            let code = match spec.requires {
                Requires::Target => ErrorCode::NotFound,
                // A missing external engine (adb, frida, …) is a provider problem, not a capability
                // the build lacks; agents branch on this to offer an install.
                Requires::Engine(_) => ErrorCode::EngineMissing,
                _ => ErrorCode::FeatureMissing,
            };
            let mut err = ToolError::new(code, avail.reason.unwrap_or_default());
            err.remediation = avail.remediation;
            if matches!(spec.requires, Requires::Os(_)) {
                err = err.with_details(serde_json::json!({"reason": "wrong_os", "tool": spec.id}));
            }
            return Err(err);
        }
        if spec.effects.needs_consent() && opts.consent != Consent::Granted {
            let allowed = !session.settings.ask_before_running || session.project.as_ref().is_some_and(|p| p.is_allowed(spec.id));
            if !allowed {
                return Err(ToolError::new(ErrorCode::ConsentRequired, format!("{} runs code or changes a device, so it needs your OK first.", spec.title))
                    .with_remediation("Approve it when asked, or allow it for this project in Settings.")
                    .with_details(serde_json::json!({"tool": spec.id})));
            }
        }
        let run = spec.run;
        let invoke = |session: &mut Session| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut ctx = Ctx { tool: spec.id, session, cancel: opts.cancel.clone(), progress: opts.progress, caller: opts.caller };
                run(&mut ctx, params)
            }))
        };
        if spec.id == tools::site_sign_in::SIGN_IN_TOOL {
            // Sign-in needs launch settings, not exclusive access to the person's live session.
            let mut isolated = Session::isolated(&session.config_dir());
            isolated.settings = session.settings.clone();
            isolated.deep_for_run = session.deep_for_run;
            // The window may stay open while the person opens another project: what it saved
            // belongs to the project it was started from.
            let started_in = session.project.clone();
            *journal_at = JournalAt::Project(started_in.as_ref().map(|p| p.root.clone()));
            drop(session);
            let output = Self::finished(spec.id, spec.title, invoke(&mut isolated))?;
            self.record_sign_in(spec.id, &output, started_in.as_ref(), &cancel)?;
            return Ok(output);
        }
        let output = Self::finished(spec.id, spec.title, invoke(&mut session))?;
        if let ToolOutput::Evidence(ev) = &output {
            session.record((**ev).clone())?;
        }
        Ok(output)
    }

    /// A tool's own result, or its panic as an error (logged) instead of a crash.
    fn finished(id: &str, title: &str, result: std::thread::Result<Result<ToolOutput, ToolError>>) -> Result<ToolOutput, ToolError> {
        match result {
            Ok(r) => r,
            Err(panic) => {
                let what = panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_default();
                log::error!("tool {id} panicked: {what}");
                Err(ToolError::internal(format!("{title} hit a bug and stopped safely. Your work is kept. ({what})")))
            }
        }
    }

    /// Record a finished sign-in's evidence in the project it started in. The live session is
    /// waited for only until Stop; then, or when another project is open by now, the evidence goes
    /// straight to the starting project's store, so a saved profile is never left unrecorded.
    fn record_sign_in(&self, tool: &'static str, output: &ToolOutput, started_in: Option<&project::Project>, cancel: &CancelToken) -> Result<(), ToolError> {
        let ToolOutput::Evidence(ev) = output else { return Ok(()) };
        let root = started_in.map(|p| &p.root);
        if let Ok(guard) = self.session.lock_unless(&|| cancel.is_cancelled()) {
            let mut session = guard.unwrap_or_else(PoisonError::into_inner);
            self.settle(&mut session);
            session.label(tool);
            if session.project.as_ref().map(|p| &p.root) == root {
                return session.record((**ev).clone());
            }
            // Another project is open now: its ledger isn't where this belongs.
            return started_in.map_or(Ok(()), |project| project.store_evidence(ev));
        }
        // Stopped while another tool holds the session: keep it on disk now and in the ledger later.
        if let Some(project) = started_in {
            project.store_evidence(ev)?;
        }
        self.pending.lock().unwrap_or_else(PoisonError::into_inner).push((root.cloned(), (**ev).clone()));
        Ok(())
    }
}

/// Where [`Engine::call`] journals a call: the live session's project, or one fixed when the call
/// started (a sign-in that ran without holding the session).
enum JournalAt {
    Live,
    Project(Option<std::path::PathBuf>),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replace only the browser callback; authorization and dispatch remain the real path.
    fn sign_in_engine(run: tool::ToolFn) -> std::sync::Arc<Engine> {
        let mut session = crate::testing::session();
        session.settings.engines.tools.chromium = Some(std::env::current_exe().unwrap());
        let mut specs = tools::all();
        specs.iter_mut().find(|s| s.id == tools::site_sign_in::SIGN_IN_TOOL).unwrap().run = run;
        std::sync::Arc::new(Engine { registry: Registry::new(specs), session: Mutex::new(session), pending: Mutex::new(Vec::new()) })
    }

    #[test]
    fn sign_in_wait_leaves_session_available_and_records_even_after_stop() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        use std::time::{Duration, Instant};
        fn waiting(ctx: &mut Ctx<'_>, _: Value) -> Result<ToolOutput, ToolError> {
            assert!(ctx.session.settings.deep_mode);
            assert!(ctx.session.settings.engines.tools.chromium.is_some());
            assert!(ctx.session.project.is_none());
            assert!(ctx.session.ext.get::<EngineRef>().is_none());
            ctx.report(None, "waiting for the person");
            assert!(ctx.cancel.is_cancelled());
            ctx.session.settings.deep_mode = false;
            let forget = tools::site_sign_in::specs().into_iter().find(|s| s.id == tools::site_sign_in::SIGN_OUT_TOOL).unwrap();
            (forget.run)(ctx, serde_json::json!({"url": "https://example.com"}))
        }
        let e = sign_in_engine(waiting);
        let config = e.session().config_dir();
        let release = Arc::new(AtomicBool::new(false));
        let gate = release.clone();
        let (started, entered) = mpsc::channel();
        let cancel = CancelToken::default();
        let stop = cancel.clone();
        let worker = e.clone();
        let sign_in = std::thread::spawn(move || {
            let progress = move |_: Progress| {
                started.send(()).unwrap();
                let deadline = Instant::now() + Duration::from_secs(10);
                while !gate.load(Ordering::Acquire) && Instant::now() < deadline {
                    std::thread::park_timeout(Duration::from_millis(5));
                }
            };
            worker.call(
                tools::site_sign_in::SIGN_IN_TOOL,
                Value::Null,
                CallOptions::new(Caller::Cli).with_consent(Consent::Granted).with_cancel(cancel).with_progress(&progress),
            )
        });
        let waiting = entered.recv_timeout(Duration::from_secs(5));
        let worker = e.clone();
        let (sent, received) = mpsc::channel();
        let other = std::thread::spawn(move || {
            let result = worker.call("core_session_show", Value::Null, CallOptions::new(Caller::Cli));
            let _ = sent.send(result);
        });
        let concurrent = received.recv_timeout(Duration::from_secs(2));
        stop.cancel();
        release.store(true, Ordering::Release);
        let output = sign_in.join().unwrap().unwrap();
        other.join().unwrap();
        waiting.unwrap();
        concurrent.expect("another tool must complete before sign-in is released").unwrap();
        let ToolOutput::Evidence(ev) = output else { panic!("expected saved evidence") };
        let session = e.session();
        assert_eq!(session.config_dir(), config);
        assert!(session.settings.deep_mode, "the snapshot must not overwrite live settings");
        assert_eq!(session.ledger().filter(|record| record.id == ev.id).count(), 1);
    }

    /// A sign-in held at its window until `gate` opens; it then returns sign-out evidence.
    fn gated_sign_in(
        e: &std::sync::Arc<Engine>,
        cancel: CancelToken,
        gate: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> (std::thread::JoinHandle<Result<ToolOutput, ToolError>>, std::sync::mpsc::Receiver<()>) {
        let (started, entered) = std::sync::mpsc::channel();
        let worker = e.clone();
        let handle = std::thread::spawn(move || {
            let progress = move |_: Progress| {
                let _ = started.send(());
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                while !gate.load(std::sync::atomic::Ordering::Acquire) && std::time::Instant::now() < deadline {
                    std::thread::park_timeout(std::time::Duration::from_millis(5));
                }
            };
            worker.call(
                tools::site_sign_in::SIGN_IN_TOOL,
                Value::Null,
                CallOptions::new(Caller::Cli).with_consent(Consent::Granted).with_cancel(cancel).with_progress(&progress),
            )
        });
        (handle, entered)
    }

    fn signs_out(ctx: &mut Ctx<'_>, _: Value) -> Result<ToolOutput, ToolError> {
        ctx.report(None, "waiting for the person");
        let forget = tools::site_sign_in::specs().into_iter().find(|s| s.id == tools::site_sign_in::SIGN_OUT_TOOL).unwrap();
        (forget.run)(ctx, serde_json::json!({"url": "https://example.com"}))
    }

    fn journalled_sign_ins(root: &std::path::Path) -> usize {
        journal::days(root)
            .iter()
            .flat_map(|d| journal::read_day(root, d))
            .filter(|entry| entry.to_string().contains(tools::site_sign_in::SIGN_IN_TOOL))
            .count()
    }

    /// Review of #1459: another project opened while the window is open doesn't get the sign-in's record.
    #[test]
    fn sign_in_records_in_the_project_it_started_in() {
        use std::sync::{Arc, atomic::AtomicBool};
        let dirs = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let a = project::Project::create(&dirs.0.path().join("a"), "A", project::Purpose::default()).unwrap();
        let b = project::Project::create(&dirs.1.path().join("b"), "B", project::Purpose::default()).unwrap();
        let e = sign_in_engine(signs_out);
        e.session().project = Some(a.clone());
        let gate = Arc::new(AtomicBool::new(false));
        let (sign_in, entered) = gated_sign_in(&e, CancelToken::default(), gate.clone());
        entered.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        e.session().project = Some(b.clone());
        gate.store(true, std::sync::atomic::Ordering::Release);
        let ToolOutput::Evidence(ev) = sign_in.join().unwrap().unwrap() else { panic!("expected evidence") };
        assert!(a.load_evidence(&ev.id).is_ok(), "the starting project keeps the record");
        assert!(b.load_evidence(&ev.id).is_err(), "the project opened meanwhile doesn't get it");
        assert!(e.session().evidence(&ev.id).is_none(), "nor its ledger");
        assert_eq!((journalled_sign_ins(&a.root), journalled_sign_ins(&b.root)), (1, 0));
    }

    /// Review of #1459: Stop ends the wait for a session another tool holds; the record is still kept.
    #[test]
    fn a_stopped_sign_in_does_not_wait_for_a_busy_session_and_keeps_its_record() {
        use std::sync::{Arc, atomic::AtomicBool};
        let dir = tempfile::tempdir().unwrap();
        let a = project::Project::create(&dir.path().join("a"), "A", project::Purpose::default()).unwrap();
        for with_project in [true, false] {
            let e = sign_in_engine(signs_out);
            if with_project {
                e.session().project = Some(a.clone());
            }
            let gate = Arc::new(AtomicBool::new(false));
            let cancel = CancelToken::default();
            let (sign_in, entered) = gated_sign_in(&e, cancel.clone(), gate.clone());
            entered.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            let busy = e.session();
            cancel.cancel();
            gate.store(true, std::sync::atomic::Ordering::Release);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !sign_in.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let finished = sign_in.is_finished();
            drop(busy);
            assert!(finished, "a stopped sign-in must not wait for the session (project: {with_project})");
            let ToolOutput::Evidence(ev) = sign_in.join().unwrap().unwrap() else { panic!("expected evidence") };
            if with_project {
                assert!(a.load_evidence(&ev.id).is_ok(), "the saved profile's evidence is kept");
                assert_eq!(journalled_sign_ins(&a.root), 1);
            }
            // Once the other tool lets go, the ledger has it too, once.
            let session = e.session();
            assert_eq!(session.ledger().filter(|r| r.id == ev.id).count(), 1, "project: {with_project}");
        }
    }

    /// A stopped sign-in's evidence waits for the project it started in, never another one.
    #[test]
    fn pending_sign_in_evidence_reaches_only_its_own_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let a = project::Project::create(&dir.path().join("a"), "A", project::Purpose::default()).unwrap();
        let e = sign_in_engine(signs_out);
        let mut s = crate::testing::session();
        let ToolOutput::Evidence(ev) = signs_out(
            &mut Ctx { tool: tools::site_sign_in::SIGN_IN_TOOL, session: &mut s, cancel: CancelToken::default(), progress: &no_progress, caller: Caller::Cli },
            Value::Null,
        )
        .unwrap() else {
            panic!("expected evidence")
        };
        e.pending.lock().unwrap().push((Some(a.root.clone()), (*ev).clone()));
        assert!(e.session().evidence(&ev.id).is_none(), "no project is open: not this ledger");
        assert!(e.pending.lock().unwrap().is_empty(), "and it isn't kept waiting forever");
        e.session().project = Some(a.clone());
        e.pending.lock().unwrap().push((Some(a.root.clone()), (*ev).clone()));
        assert!(e.session().evidence(&ev.id).is_some(), "its own project is open: recorded");
    }

    #[test]
    fn isolated_sign_in_still_requires_consent_and_contains_panics() {
        fn broken(_: &mut Ctx<'_>, _: Value) -> Result<ToolOutput, ToolError> {
            panic!("stand-in browser bug");
        }
        let e = sign_in_engine(broken);
        let refused = e.call(tools::site_sign_in::SIGN_IN_TOOL, Value::Null, CallOptions::new(Caller::Cli)).unwrap_err();
        assert_eq!(refused.code, ErrorCode::ConsentRequired);
        let failed = e.call(tools::site_sign_in::SIGN_IN_TOOL, Value::Null, CallOptions::new(Caller::Cli).with_consent(Consent::Granted)).unwrap_err();
        assert_eq!(failed.code, ErrorCode::Internal);
        assert!(e.call("core_session_show", Value::Null, CallOptions::new(Caller::Cli)).is_ok());
    }

    #[test]
    fn unknown_tool_is_an_error_not_a_panic() {
        let e = Engine::new(crate::testing::session());
        let err = e.call("no_such_tool", Value::Null, CallOptions::new(Caller::Cli)).unwrap_err();
        assert_eq!(err.code, ErrorCode::BadInput);
    }

    /// #841: Try again repeated "install a browser" for a person who had set a path to theirs.
    #[test]
    fn a_tool_says_when_the_path_set_for_its_engine_is_not_there() {
        let mut session = crate::testing::session();
        session.settings.engines.tools.chromium = Some("/nowhere/brave".into());
        let e = Engine::new(session);
        let err = e.call("web_site_open", serde_json::json!({"url": "https://example.com"}), CallOptions::new(Caller::Cli)).unwrap_err();
        assert_eq!(err.code, ErrorCode::EngineMissing);
        assert!(err.message.contains("isn't there") && err.message.contains("brave"), "{}", err.message);
    }

    #[test]
    fn every_engine_a_tool_needs_is_in_the_engine_table() {
        let e = Engine::new(crate::testing::session());
        for spec in e.registry().all() {
            if let Requires::Engine(id) = spec.requires {
                assert!(craft_anything_engines::engine(id).is_some(), "{} needs unknown engine {id:?}", spec.id);
            }
        }
    }

    #[test]
    fn target_tools_explain_what_to_do() {
        let e = Engine::new(crate::testing::session());
        let err = e.call("native_cursor_show", Value::Null, CallOptions::new(Caller::Mcp)).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert!(err.remediation.is_some());
    }

    #[test]
    fn higher_layers_add_tools_and_reach_the_engine() {
        fn whoami(ctx: &mut Ctx<'_>, _: Value) -> Result<ToolOutput, ToolError> {
            let alive = ctx.session.ext.get::<EngineRef>().and_then(|r| r.0.upgrade()).is_some();
            Ok(ToolOutput::Value(serde_json::json!({"engine": alive})))
        }
        let extra = ToolSpec {
            id: "test_whoami",
            title: "Who am I",
            summary: "Test tool.",
            beginner: "Test tool.",
            toolset: Toolset::Core,
            effects: Effects::READ_ONLY,
            requires: Requires::Nothing,
            input_schema: tool::empty_schema,
            run: whoami,
        };
        let e = Engine::shared(crate::testing::session(), vec![extra]);
        assert!(e.registry().get("core_target_triage").is_some(), "the engine's own tools are still there");
        let out = e.call("test_whoami", Value::Null, CallOptions::new(Caller::Cli)).unwrap().to_json();
        assert_eq!(out["result"]["engine"], true);
    }

    #[test]
    fn tools_defined_at_run_time_get_their_own_schema_group_and_id() {
        use registry::{RuntimeTool, custom_toolset};
        fn echo(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::Value(serde_json::json!({"tool": ctx.tool, "params": params})))
        }
        let group = custom_toolset("demo_plugin").unwrap();
        let tool = |id: &str, schema: Value| RuntimeTool {
            id: id.into(),
            title: format!("Demo {id}"),
            summary: "A demo tool.".into(),
            beginner: "A demo tool.".into(),
            toolset: group,
            effects: Effects::READ_ONLY,
            requires: Requires::Nothing,
            schema,
            run: echo,
        };
        let schema = serde_json::json!({"type": "object", "properties": {"word": {"type": "string"}}});
        let e = Engine::shared_with(
            crate::testing::session(),
            Vec::new(),
            vec![
                tool("demo_one", schema.clone()),
                tool("demo_two", schema.clone()),
                tool("Bad Id", schema.clone()),
                tool("core_target_triage", schema.clone()),
                tool("demo_three", serde_json::json!("not an object")),
            ],
        );
        let r = e.registry();
        assert!(r.get("demo_one").is_some() && r.get("demo_two").is_some());
        assert!(r.get("demo_three").is_none(), "a schema must be an object");
        assert_ne!(r.get("core_target_triage").map(|s| s.title), Some("Demo core_target_triage"), "an engine tool keeps its id");
        assert!(r.get("bad_id").is_none());
        // One function serves both; it knows which was called.
        let out = e.call("demo_two", serde_json::json!({"word": "hi"}), CallOptions::new(Caller::Cli)).unwrap().to_json();
        assert_eq!(out["result"]["tool"], "demo_two");
        let spec = *r.get("demo_one").unwrap();
        assert_eq!(r.mcp_descriptor(&spec)["inputSchema"], schema);
        assert!(r.toolsets().contains(&group) && r.toolset("demo_plugin") == Some(group));
        assert_eq!(serde_json::to_value(group).unwrap(), "demo_plugin");
        assert!(custom_toolset("No Spaces").is_none());
        assert_eq!(custom_toolset("core"), Some(Toolset::Core), "a built-in id is the built-in group");
    }
}
