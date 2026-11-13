use super::*;

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn run_with(opts: ServerOptions, lines: &[Value]) -> Vec<Value> {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    run_raw(opts, &input)
}

fn run_raw(opts: ServerOptions, input: &str) -> Vec<Value> {
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let buf = Buf::default();
    serve(engine, opts, std::io::Cursor::new(input.as_bytes().to_vec()), Box::new(buf.clone())).unwrap();
    let out = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
    out.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn run(lines: &[Value]) -> Vec<Value> {
    run_with(ServerOptions::default(), lines)
}

fn req(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn response(out: &[Value], id: u64) -> Value {
    out.iter().find(|m| m["id"] == json!(id)).cloned().unwrap_or_else(|| panic!("no response {id} in {out:?}"))
}

/// A coding agent's permission-prompt tool accepts exactly one text block holding the decision
/// JSON; anything else (structuredContent, extra notes, an error result) makes it refuse every
/// action. Failures must still come back as a readable decision.
#[test]
fn approval_tool_answers_in_the_permission_prompt_shape() {
    let out = run(&[
        req(1, "tools/call", json!({"name": "core_action_approve", "arguments": {"tool_name": "Read", "input": {"file_path": "x"}, "tool_use_id": "t"}})),
        req(2, "tools/call", json!({"name": "core_action_approve", "arguments": {"input": {}}})),
    ]);
    for id in [1, 2] {
        let r = response(&out, id)["result"].clone();
        assert_eq!(r["isError"], false, "{r}");
        assert!(r.get("structuredContent").is_none(), "{r}");
        let content = r["content"].as_array().unwrap();
        assert_eq!(content.len(), 1, "{r}");
        assert_eq!(content[0]["type"], "text");
        let decision: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
        assert!(decision["behavior"] == "allow" || decision["behavior"] == "deny", "{decision}");
    }
    let ok: Value = serde_json::from_str(response(&out, 1)["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(ok, json!({"behavior": "allow", "updatedInput": {"file_path": "x"}}));
    let bad: Value = serde_json::from_str(response(&out, 2)["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(bad["behavior"], "deny", "a malformed request is refused in the protocol's own words");
}

/// #253: an allowed action with a large input came back as an `over_limit` error with no decision.
#[test]
fn a_large_approval_keeps_its_decision() {
    let input = json!({"file_path": "x", "note": "a".repeat(4096)});
    let out = run_with(
        ServerOptions { max_response_bytes: 300, ..Default::default() },
        &[req(1, "tools/call", json!({"name": "core_action_approve", "arguments": {"tool_name": "Read", "input": input, "tool_use_id": "t"}}))],
    );
    let r = response(&out, 1)["result"].clone();
    assert_eq!(r["isError"], false, "{r}");
    let decision: Value = serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(decision, json!({"behavior": "allow", "updatedInput": input}));
}

/// #263: in Clean room, enabling a toolset counted the tools Clean room hides from the list.
#[test]
fn enabling_a_toolset_counts_the_tools_listed() {
    let input = format!(
        "{}
",
        req(1, "tools/call", json!({"name": "enable_toolset", "arguments": {"toolset": "all"}}))
    );
    let mut session = craft_anything_engine::testing::session();
    session.settings.deep_mode = false;
    let engine = Arc::new(Engine::new(session));
    let runs = engine.registry().all().iter().filter(|s| craft_anything_engine::clean_room::allows(s.id)).count();
    let buf = Buf::default();
    serve(engine, ServerOptions::default(), std::io::Cursor::new(input.into_bytes()), Box::new(buf.clone())).unwrap();
    let out: Vec<Value> = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let enabled: Value = serde_json::from_str(response(&out, 1)["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(enabled["result"]["visible_tools"], json!(runs), "{enabled}");
}

fn tool_names(v: &Value) -> Vec<String> {
    v["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn initialize_negotiates_version() {
    let out = run(&[
        req(1, "initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}})),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        req(2, "initialize", json!({"protocolVersion": "1999-01-01"})),
        req(3, "ping", json!({})),
    ]);
    let r = response(&out, 1);
    assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(r["result"]["serverInfo"]["name"], "craft-anything");
    assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], true);
    assert_eq!(response(&out, 2)["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    assert_eq!(response(&out, 3)["result"], json!({}));
    assert_eq!(out.len(), 3, "notifications get no response");
}

#[test]
fn progressive_toolsets() {
    let out = run(&[req(1, "tools/list", json!({})), req(2, "tools/call", json!({"name": "enable_toolset", "arguments": {"toolset": "native"}}))]);
    let names = tool_names(&response(&out, 1));
    assert!(names.contains(&"core_session_show".to_string()));
    assert!(names.contains(&"core_investigation_show".to_string()));
    assert!(names.contains(&"enable_toolset".to_string()));
    assert!(!names.contains(&"native_cursor_show".to_string()), "native tools hidden at first");
    assert!(out.iter().any(|m| m["method"] == "notifications/tools/list_changed"));
    // a second connection after enabling sees the tool
    let out = run(&[req(1, "tools/call", json!({"name": "enable_toolset", "arguments": {"toolset": "native"}})), req(2, "tools/list", json!({}))]);
    // tools/call runs on a worker; list may be answered first, so check the call result only
    let r = response(&out, 1);
    assert_eq!(r["result"]["isError"], false);
    let all = run_with(ServerOptions { all_tools: true, ..Default::default() }, &[req(1, "tools/list", json!({}))]);
    let names = tool_names(&response(&all, 1));
    assert!(names.contains(&"native_cursor_show".to_string()));
    assert!(!names.contains(&"enable_toolset".to_string()));
}

#[test]
fn tool_call_success_and_error_shapes() {
    let out = run(&[
        req(1, "tools/call", json!({"name": "core_session_show", "arguments": {}})),
        req(2, "tools/call", json!({"name": "core_target_close", "arguments": {}})),
        req(3, "tools/call", json!({"name": "no_such_tool", "arguments": {}})),
    ]);
    let ok = response(&out, 1);
    assert_eq!(ok["result"]["isError"], false);
    assert_eq!(ok["result"]["structuredContent"]["result"]["server"]["name"], "craft-anything");
    let text: Value = serde_json::from_str(ok["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, ok["result"]["structuredContent"]);
    let err = response(&out, 2);
    assert_eq!(err["result"]["isError"], true);
    let e: Value = serde_json::from_str(err["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(e["error"]["code"], "not_found");
    assert!(e["error"]["remediation"].is_string());
    assert_eq!(response(&out, 3)["result"]["isError"], true);
}

#[test]
fn hidden_tool_call_enables_its_toolset() {
    let out = run(&[req(1, "tools/call", json!({"name": "native_cursor_show", "arguments": {}}))]);
    assert!(out.iter().any(|m| m["method"] == "notifications/tools/list_changed"));
    assert_eq!(response(&out, 1)["result"]["isError"], true);
}

#[test]
fn protocol_errors() {
    let out = run_raw(ServerOptions::default(), "{not json\n[1,2]\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"bogus\"}\n\n");
    assert_eq!(out[0]["error"]["code"], -32700);
    assert_eq!(out[1]["error"]["code"], -32600);
    assert_eq!(out[2]["error"]["code"], -32601);
    assert_eq!(out[2]["id"], 7);
}

#[test]
fn prompts_and_completion() {
    let out = run(&[
        req(1, "prompts/list", json!({})),
        req(2, "prompts/get", json!({"name": "native_crash_explain", "arguments": {"symptom": "SIGSEGV in parse"}})),
        req(3, "prompts/get", json!({"name": "native_crash_explain", "arguments": {}})),
        req(4, "completion/complete", json!({"ref": {"type": "ref/prompt", "name": "core_unknowns_review"}, "argument": {"name": "evidence", "value": "ev_"}})),
        req(5, "completion/complete", json!({"ref": {"type": "ref/resource", "uri": "x"}, "argument": {"name": "a", "value": ""}})),
    ]);
    assert_eq!(response(&out, 1)["result"]["prompts"].as_array().unwrap().len(), 6);
    let text = response(&out, 2)["result"]["messages"][0]["content"]["text"].as_str().unwrap().to_string();
    assert!(text.contains("SIGSEGV in parse"));
    assert_eq!(response(&out, 3)["error"]["code"], -32602);
    assert_eq!(response(&out, 4)["result"]["completion"]["values"], json!([]));
    assert_eq!(response(&out, 5)["result"]["completion"]["total"], 0);
}

#[test]
fn resources() {
    let out = run(&[
        req(1, "resources/list", json!({})),
        req(2, "resources/read", json!({"uri": "investigation://brief"})),
        req(3, "resources/read", json!({"uri": "evidence://ev_missing"})),
        req(4, "resources/templates/list", json!({})),
        req(5, "resources/read", json!({"uri": "investigation://state"})),
    ]);
    let list = response(&out, 1);
    let uris: Vec<&str> = list["result"]["resources"].as_array().unwrap().iter().map(|r| r["uri"].as_str().unwrap()).collect();
    assert!(uris.contains(&"investigation://handoff"));
    assert!(response(&out, 2)["result"]["contents"][0]["text"].as_str().unwrap().starts_with("# Brief"));
    assert_eq!(response(&out, 3)["error"]["code"], -32002);
    assert_eq!(response(&out, 4)["result"]["resourceTemplates"][0]["uriTemplate"], "evidence://{evidence_id}");
    let state: Value = serde_json::from_str(response(&out, 5)["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(state["revision"], 0);
}

#[test]
fn oversize_results_become_resource_constraint() {
    let out = run_with(
        ServerOptions { max_response_bytes: 300, ..Default::default() },
        &[req(1, "tools/call", json!({"name": "core_session_show", "arguments": {}}))],
    );
    let r = response(&out, 1);
    assert_eq!(r["result"]["isError"], true);
    let e: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(e["error"]["code"], "over_limit");
    assert_eq!(e["error"]["details"]["resource"], "transport");
}

#[test]
fn progress_token_gets_notifications_and_cancel_of_unknown_is_ignored() {
    let out = run(&[
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 99}}),
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "core_session_show", "arguments": {}, "_meta": {"progressToken": "p1"}}}),
    ]);
    let progress: Vec<&Value> = out.iter().filter(|m| m["method"] == "notifications/progress").collect();
    assert!(!progress.is_empty());
    assert_eq!(progress[0]["params"]["progressToken"], "p1");
    assert_eq!(response(&out, 1)["result"]["isError"], false);
}

#[test]
fn memory_tools_work_over_mcp() {
    let out = run(&[req(1, "tools/call", json!({"name": "core_brief_write", "arguments": {"question": "How does login work?"}}))]);
    assert_eq!(response(&out, 1)["result"]["isError"], false);
}

fn test_server(engine: Arc<Engine>, opts: ServerOptions) -> Server {
    Server::new(engine, opts, Box::new(Buf::default()))
}

#[test]
fn provenance_meta_is_set_for_target_content() {
    let server = test_server(Arc::new(Engine::new(craft_anything_engine::testing::session())), ServerOptions::default());

    // By tool id: a known producer of target content, even when the record didn't set the flag.
    let mut body = json!({"content": [{"type": "text", "text": "x"}], "structuredContent": {"result": {}}, "isError": false});
    server.attach_provenance(&mut body, "native_function_pseudocode");
    assert_eq!(body["_meta"]["provenance"]["from_target"], true);
    assert!(body["content"].as_array().unwrap().iter().any(|c| c["text"].as_str().unwrap_or_default().contains("data, not instructions")));

    // By the evidence flag: an unlisted tool whose evidence set carries_target_content.
    let mut body2 = json!({"content": [], "structuredContent": {"carries_target_content": true}, "isError": false});
    server.attach_provenance(&mut body2, "some_unlisted_tool");
    assert_eq!(body2["_meta"]["provenance"]["from_target"], true);

    // A plain tool with no target content: no provenance marker.
    let mut body3 = json!({"content": [], "structuredContent": {"result": {"open": false}}, "isError": false});
    server.attach_provenance(&mut body3, "core_session_show");
    assert!(body3.get("_meta").is_none());
}

#[test]
fn oversize_evidence_collapses_to_a_summary() {
    use craft_anything_evidence::{Confidence, Environment, EvidenceBuilder, Subject};
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let ev = EvidenceBuilder::new(
        "extract_strings",
        Subject { kind: "native".into(), sha256: Some("ab".repeat(32)), name: Some("calc".into()) },
        Environment::builtin(true),
    )
    .result(json!({"strings": vec!["x"; 4000]}))
    .summary("Four thousand strings.")
    .confidence(Confidence::Observed)
    .target_content(true)
    .build()
    .unwrap();
    let ev_id = ev.id.clone();
    let full = serde_json::to_value(&ev).unwrap();
    engine.session().record(ev).unwrap();

    let server = test_server(engine, ServerOptions { max_response_bytes: 512, ..Default::default() });
    // Shape the success response the server would have produced for this evidence.
    let resp = ok_response(json!(1), json!({"structuredContent": full, "isError": false}));
    let summary = server.oversize_summary(&resp).expect("a saved summary");
    let sc = &summary["structuredContent"];
    assert_eq!(sc["detail"], "summary");
    assert_eq!(sc["saved_evidence"]["evidence_id"], json!(ev_id));
    assert_eq!(sc["continue_with"], "core_evidence_browse");
    assert_eq!(summary["isError"], false);
    // It carries the from-target marker too.
    assert_eq!(summary["_meta"]["provenance"]["from_target"], true);
    // And the summary is far smaller than the full record.
    assert!(serde_json::to_string(&summary).unwrap().len() < serde_json::to_string(&resp).unwrap().len());
}

/// Claude Code takes about 25k tokens of a tool's output, so its connections get a smaller budget
/// (bigger results come back summary-first); other clients and an explicit budget are left alone.
#[test]
fn claude_code_connections_get_a_budget_that_fits_its_limit() {
    let server = |opts: ServerOptions| Server::new(Arc::new(Engine::new(craft_anything_engine::testing::session())), opts, Box::new(Buf::default()));
    let hello = |name: &str| json!({"protocolVersion": PROTOCOL_VERSIONS[0], "clientInfo": {"name": name, "version": "1"}});
    let s = server(ServerOptions::default());
    s.handle_request("initialize", &hello("claude-code")).unwrap();
    assert_eq!(s.budget(), CLAUDE_CODE_RESPONSE_BYTES);
    let other = server(ServerOptions::default());
    other.handle_request("initialize", &hello("some-editor")).unwrap();
    assert_eq!(other.budget(), DEFAULT_MAX_RESPONSE_BYTES);
    let set = server(ServerOptions { max_response_bytes: 500_000, budget_for_client: false, ..Default::default() });
    set.handle_request("initialize", &hello("claude-code")).unwrap();
    assert_eq!(set.budget(), 500_000, "an explicit budget wins");
}

/// #563: "Always for this project" on a consent card was dropped, so the same tool was asked about
/// every time. It is remembered in the project, and a tool it was said for isn't asked again.
#[test]
fn always_for_this_project_on_a_consent_card_is_remembered() {
    use craft_anything_engine::inbox::{Answer, Inbox, InboxDir};
    use craft_anything_engine::workspace::{TargetRef, Workspace};
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::create(dir.path(), "t", "r", TargetRef::None).unwrap();
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    engine.session().ext.insert(Arc::new(InboxDir(ws.inbox_dir())));
    let server = Server::new(engine, ServerOptions::default(), Box::new(Buf::default()));
    let inbox = Inbox::new(ws.inbox_dir());

    // Allowed once: this call goes through, and nothing is remembered.
    let root = ws.inbox_dir();
    let answerer = |always: bool| {
        let inbox = Inbox::new(root.clone());
        std::thread::spawn(move || {
            for _ in 0..400 {
                if let Some(r) = inbox.pending().first() {
                    inbox.answer(&r.id, &Answer { allow: Some(true), always, ..Default::default() }).unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    };
    let once = answerer(false);
    assert_eq!(server.ask_consent("web_session_run", "why", &CancelToken::default()), Some(true));
    once.join().unwrap();
    assert!(Workspace::open(&ws.root).unwrap().manifest.allowed_actions.is_empty());

    // Always: remembered as `tool:<id>`, and the next call isn't asked (the token is already
    // cancelled, so asking would be refused, not waited on).
    let always = answerer(true);
    assert_eq!(server.ask_consent("web_session_run", "why", &CancelToken::default()), Some(true));
    always.join().unwrap();
    assert_eq!(Workspace::open(&ws.root).unwrap().manifest.allowed_actions, vec!["tool:web_session_run".to_string()]);
    let cancelled = CancelToken::default();
    cancelled.cancel();
    assert_eq!(server.ask_consent("web_session_run", "why", &cancelled), Some(true), "not asked again");
    assert!(inbox.pending().is_empty());
    // Another tool is still asked about.
    assert_eq!(server.ask_consent("browser_other", "why", &cancelled), Some(false));
}

/// #855, #856: In restricted mode, tools that act on the machine (build_command_run, plugins_add,
/// engines_install, adb_*) and web/capture tools (web_site_open, web_site_capture,
/// web_page_states_capture) are omitted from tools/list even when --all-tools is set.
#[test]
fn restricted_mode_omits_machine_and_web_tools() {
    let opts = ServerOptions { restricted: true, all_tools: true, ..Default::default() };
    let out = run_with(opts, &[req(1, "tools/list", json!({}))]);
    let names = tool_names(&response(&out, 1));

    // Machine-acting tools (#855) are absent
    assert!(!names.contains(&"build_command_run".to_string()));
    assert!(!names.contains(&"plugins_add".to_string()));
    assert!(!names.contains(&"engines_install".to_string()));
    assert!(!names.iter().any(|n| n.starts_with("adb_")));

    // Web and capture tools (#856) are absent by default
    assert!(!names.contains(&"web_site_open".to_string()));
    assert!(!names.contains(&"web_site_capture".to_string()));
    assert!(!names.contains(&"web_page_states_capture".to_string()));

    // Tools no list named: what each declares keeps them off too.
    for id in ["project_play", "web_browser_connect", "web_page_screenshot_take", "web_standin_start", "native_cutter_command_run"] {
        assert!(!names.contains(&id.to_string()), "{id}");
    }

    // Safe core tools remain available
    assert!(names.contains(&"core_session_show".to_string()));
    assert!(names.contains(&"core_investigation_show".to_string()));
}

/// #855, #856: Calling turned-off tools in restricted mode returns a refusal stating they are off.
#[test]
fn restricted_mode_refuses_calling_turned_off_tools() {
    let opts = ServerOptions { restricted: true, ..Default::default() };
    let out = run_with(
        opts,
        &[
            req(1, "tools/call", json!({"name": "web_site_open", "arguments": {"url": "https://example.com"}})),
            req(2, "tools/call", json!({"name": "web_site_capture", "arguments": {}})),
            req(3, "tools/call", json!({"name": "web_page_states_capture", "arguments": {}})),
            req(4, "tools/call", json!({"name": "build_command_run", "arguments": {"command": "cargo build"}})),
            req(5, "tools/call", json!({"name": "plugins_add", "arguments": {}})),
            req(6, "tools/call", json!({"name": "engines_install", "arguments": {}})),
            req(7, "tools/call", json!({"name": "adb_device_show", "arguments": {}})),
            req(8, "tools/call", json!({"name": "mcp__craft-anything__web_site_open", "arguments": {}})),
            req(9, "tools/call", json!({"name": "web-site-open", "arguments": {}})),
        ],
    );

    for (id, tool_name) in [
        (1, "web_site_open"),
        (2, "web_site_capture"),
        (3, "web_page_states_capture"),
        (4, "build_command_run"),
        (5, "plugins_add"),
        (6, "engines_install"),
        (7, "adb_device_show"),
        (8, "web_site_open"),
        (9, "web_site_open"),
    ] {
        let r = response(&out, id);
        assert_eq!(r["result"]["isError"], true, "id {id} ({tool_name}) should be error");
        let e: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(e["error"]["code"], "feature_missing");
        let msg = e["error"]["message"].as_str().unwrap();
        assert!(msg.contains("turned off in restricted mode"), "{msg}");
        assert_eq!(e["error"]["details"]["reason"], "restricted_mode");
        assert_eq!(e["error"]["details"]["tool"], tool_name);
    }
}

/// #856: When the operator allows web or capture tools in restricted mode, they are visible and
/// callable, while machine-acting tools remain off.
#[test]
fn restricted_mode_allows_web_and_capture_tools_when_operator_permits() {
    // With allow_web
    let opts = ServerOptions { restricted: true, all_tools: true, allow_web: true, ..Default::default() };
    let out = run_with(opts, &[req(1, "tools/list", json!({}))]);
    let names = tool_names(&response(&out, 1));
    assert!(names.contains(&"web_site_open".to_string()));
    assert!(!names.contains(&"web_site_capture".to_string()), "--allow-web is only for web tools");
    assert!(!names.contains(&"web_page_states_capture".to_string()));
    assert!(!names.contains(&"build_command_run".to_string()));
    assert!(!names.iter().any(|n| n.starts_with("adb_")));
    // Signing in opens a window on the host's desktop: never with --restricted.
    assert!(!names.contains(&"web_site_sign_in".to_string()) && !names.contains(&"web_site_sign_out".to_string()));

    // With allow_capture
    let opts_cap = ServerOptions { restricted: true, all_tools: true, allow_capture: true, ..Default::default() };
    let out_cap = run_with(opts_cap, &[req(1, "tools/list", json!({}))]);
    let names_cap = tool_names(&response(&out_cap, 1));
    assert!(!names_cap.contains(&"web_site_open".to_string()), "--allow-capture is only for capture tools");
    assert!(names_cap.contains(&"web_site_capture".to_string()));
    assert!(names_cap.contains(&"web_page_states_capture".to_string()));
    assert!(!names_cap.contains(&"build_command_run".to_string()));
    assert!(!names_cap.iter().any(|n| n.starts_with("adb_")));
}

/// Review of #1459: --allow-web (with --allow-run) must not let a restricted client open the
/// desktop person's saved account; sign-in is hidden there, so reusing one would bypass that.
#[test]
fn restricted_web_opens_never_use_a_saved_sign_in() {
    use craft_anything_engine::tools::site_sign_in;
    let session = craft_anything_engine::testing::session();
    // A saved sign-in for the site, as web_site_sign_in leaves it.
    let key = site_sign_in::site_key("https://example.com").unwrap();
    let dir = site_sign_in::sign_ins_dir(&session).join(&key);
    std::fs::create_dir_all(dir.join("profile/Default")).unwrap();
    std::fs::write(dir.join("profile/Default/Cookies"), b"fake-cookie-db").unwrap();
    std::fs::write(dir.join("sign-in.json"), json!({"version": 1, "site": key, "saved_at": 1}).to_string()).unwrap();
    assert!(site_sign_in::saved(&session, &key).is_some(), "the fixture must look like a saved sign-in");
    let engine = Arc::new(Engine::new(session));
    let opts = ServerOptions { restricted: true, allow_web: true, allow_run: true, ..Default::default() };

    let input: String = [
        req(1, "tools/call", json!({"name": "web_site_open", "arguments": {"url": "https://example.com", "signed_in": true}})),
        req(2, "tools/call", json!({"name": "mcp__craft-anything__web_site_open", "arguments": {"url": "https://example.com", "signed_in": "yes"}})),
    ]
    .iter()
    .map(|l| format!("{l}\n"))
    .collect();
    let buf = Buf::default();
    serve(engine.clone(), opts.clone(), std::io::Cursor::new(input.into_bytes()), Box::new(buf.clone())).unwrap();
    let out: Vec<Value> = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    for id in [1, 2] {
        let r = response(&out, id);
        assert_eq!(r["result"]["isError"], true, "id {id}");
        let e: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(e["error"]["details"]["reason"], "restricted_mode", "id {id}: {e}");
        assert_eq!(e["error"]["details"]["param"], "signed_in", "id {id}: {e}");
    }

    // Omitted (which would pick the saved sign-in) and explicit false both reach the tool as false.
    let server = Server::new(engine.clone(), opts, Box::new(Buf::default()));
    for args in [
        json!({"url": "https://example.com"}),
        json!({"url": "https://example.com", "signed_in": false}),
        json!({"url": "https://example.com", "signed_in": null}),
    ] {
        assert_eq!(server.restricted_args("web_site_open", args).unwrap()["signed_in"], false);
    }
    // Other tools and unrestricted servers are left alone.
    assert_eq!(server.restricted_args("core_session_show", json!({})).unwrap(), json!({}));
    let open = Server::new(engine, ServerOptions::default(), Box::new(Buf::default()));
    assert_eq!(open.restricted_args("web_site_open", json!({"url": "https://example.com"})).unwrap(), json!({"url": "https://example.com"}));
}

#[test]
fn restricted_mode_server_options_from_env() {
    let map = [("CRAFT_ANYTHING_MCP_RESTRICTED", "1"), ("CRAFT_ANYTHING_MCP_ALLOW_WEB", "true"), ("CRAFT_ANYTHING_MCP_ALLOW_CAPTURE", "1")]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();

    let opts = ServerOptions::from_env_vars(|k| map.get(k).map(|s| s.to_string()));
    assert!(opts.restricted);
    assert!(opts.allow_web);
    assert!(opts.allow_capture);
}
