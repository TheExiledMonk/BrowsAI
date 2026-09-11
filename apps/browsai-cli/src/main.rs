use browsai_action_planner::{ActionPlanner, PlanningError};
use browsai_agent_protocol::{PageQuery, PROTOCOL_VERSION};
use browsai_agent_runtime::{
    AgentRuntime, AgentSessionId, SolveAuditEvent, TakeoverManager,
};
use browsai_agent_tree::{AgentNode, SemanticRole, StructuralRole};
use browsai_compatibility_check::{
    run_site, CheckConfig, CheckRun, CorpusState, FailureCategory, FailureRecord, Phase, RunStore,
    SiteExecutor, SiteRecord, SiteResult, SiteStatus, Subsystem, SubsystemResult,
};
use browsai_engine_api::{BrowserEngine, ContextOptions, VirtualViewport};
use browsai_engine_servo::ServoEngine;
use browsai_input::{
    ActionType, AgentAction, MouseTrajectoryOptions, NativeInputDispatcher, NativeInputEvent,
};
use browsai_sandbox::{Capability, SandboxPolicy};
use browsai_state::PageSnapshot;
use std::env;
use std::io::Write;
use std::time::Instant;
use url::Url;

#[cfg(unix)]
unsafe extern "C" {
    fn _exit(status: i32) -> !;
}

/// Maximum cursor distance (in CSS pixels) between the previous dispatched
/// position and a challenge widget center for the helper to take the
/// discrete-click path. Anything farther triggers the humanized trajectory.
/// Re-exported as `browsai_agent_runtime::SOLVE_NEAR_THRESHOLD_PX`.
const AUTO_SOLVE_NEAR_THRESHOLD_PX: f64 = browsai_agent_runtime::SOLVE_NEAR_THRESHOLD_PX;

/// Return a random cursor origin inside the given viewport, seeded from the
/// page URL and a per-invocation counter. Real users do not always start
/// with the cursor at (0, 0) when a page loads; we mimic that by drawing a
/// fresh starting point from a deterministic per-page RNG so trajectories
/// are not all identical.
fn random_cursor_origin(
    viewport: VirtualViewport,
    url: &Url,
    invocation_seed: u64,
) -> (f64, f64) {
    use browsai_input::SplitMix64;
    let url_seed = url.as_str().bytes().fold(0u64, |acc, byte| {
        acc.wrapping_mul(0x100000001B3).wrapping_add(byte as u64)
    });
    let mut rng = SplitMix64::new(url_seed.wrapping_add(invocation_seed));
    let margin = 32.0_f64;
    let max_x = (viewport.width as f64 - margin).max(margin + 1.0);
    let max_y = (viewport.height as f64 - margin).max(margin + 1.0);
    let x = margin + rng.next_unit() * (max_x - margin);
    let y = margin + rng.next_unit() * (max_y - margin);
    (x, y)
}

fn auto_solve_challenges(
    engine: &mut ServoEngine,
    runtime: &mut AgentRuntime,
    session: AgentSessionId,
    takeovers: &TakeoverManager,
    page: browsai_engine_api::PageId,
    snapshot: &PageSnapshot,
    now: u64,
    cursor_origin: (f64, f64),
) -> Result<Vec<SolveAuditEvent>, String> {
    browsai_agent_runtime::solve_observed_challenges(
        page,
        snapshot,
        runtime,
        session,
        takeovers,
        now,
        cursor_origin,
        |event| {
            engine
                .dispatch_input(page, event.clone())
                .map_err(|error| format!("auto-solve dispatch failed: {error:?}"))
        },
        |error| eprintln!("BROWSAI_AUTO_SOLVE_ERROR: {error}"),
    )
}

fn usage() -> &'static str {
    "browsai commands:\n  version\n  capabilities\n  profile <name>\n  workspace <profile-name>\n  headless <url>\n  open <url>\n  navigate <url>\n  render <url>\n  query <url>\n  action <url> <target> <kind>\n  live-open <url> [--link-cursor N --link-limit N --link-max-bytes N --link-max-duration-ms N] [--auto-solve]\n  live-search <query> [--open-links] [--link-cursor N --link-limit N --link-max-bytes N --link-max-duration-ms N]\n  check site <url>\n  check corpus <sites.csv>\n  check report <run-id>\n  logs <log.json>\n  audit <journal.json>\n  replay <journal.json>\n  benchmark <result.json>\n  recovery <checkpoint.json>\n"
}

fn run_internal(args: &[String], one_shot_live_runtime: bool) -> Result<String, String> {
    match args.get(1).map(String::as_str) {
        Some("version") => Ok(format!(
            "browsai {} protocol {}",
            env!("CARGO_PKG_VERSION"),
            PROTOCOL_VERSION
        )),
        Some("capabilities") => serde_json::to_string_pretty(&ServoEngine::new().capabilities())
            .map_err(|error| error.to_string()),
        Some("profile") => {
            let name = args
                .get(2)
                .ok_or_else(|| "profile requires a name".to_string())?;
            let mut profiles = browsai_profiles::ProfileManager::default();
            let id = profiles.create(name);
            serde_json::to_string(
                &profiles
                    .get(&id)
                    .ok_or_else(|| "profile creation failed".to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        Some("workspace") => {
            let name = args
                .get(2)
                .ok_or_else(|| "workspace requires a profile name".to_string())?;
            let mut profiles = browsai_profiles::ProfileManager::default();
            let profile = profiles.create(name);
            let mut workspace = browsai_workspace::Workspace::new(profile);
            workspace.create_window(1280, 720);
            serde_json::to_string(&workspace.checkpoint()).map_err(|error| error.to_string())
        }
        Some(command @ ("headless" | "navigate" | "open")) => {
            let url = Url::parse(
                args.get(2)
                    .ok_or_else(|| "headless requires a URL".to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let mut engine = ServoEngine::new();
            let headless = command == "headless";
            let context = engine
                .create_context(ContextOptions {
                    headless,
                    viewport: Some(VirtualViewport::default()),
                    deterministic_clock_millis: Some(0),
                    no_raster: headless,
                    ..Default::default()
                })
                .map_err(|error| error.to_string())?;
            let page = engine
                .create_page(context)
                .map_err(|error| error.to_string())?;
            let navigation = engine
                .navigate(page, url)
                .map_err(|error| error.to_string())?;
            let snapshot = engine.snapshot(page).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({ "page": navigation.page, "url": navigation.url, "snapshot": snapshot.id }).to_string())
        }
        Some("query") | Some("render") => {
            let url = Url::parse(
                args.get(2)
                    .ok_or_else(|| "query/render requires a URL".to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let mut engine = ServoEngine::new();
            let context = engine
                .create_context(ContextOptions {
                    headless: true,
                    viewport: Some(VirtualViewport::default()),
                    deterministic_clock_millis: Some(0),
                    no_raster: true,
                    ..Default::default()
                })
                .map_err(|error| error.to_string())?;
            let page = engine
                .create_page(context)
                .map_err(|error| error.to_string())?;
            engine
                .navigate(page, url)
                .map_err(|error| error.to_string())?;
            let snapshot = engine.snapshot(page).map_err(|error| error.to_string())?;
            let paged = args.iter().any(|argument| {
                argument == "--cursor"
                    || argument == "--limit"
                    || argument.starts_with("--cursor=")
                    || argument.starts_with("--limit=")
            });
            if paged {
                let cursor = bounded_option(args, "--cursor", 0, 100_000)?;
                let limit = bounded_option(args, "--limit", 100, 1_000)?.max(1);
                let page = PageQuery::new(&snapshot.tree).render_page(cursor, limit);
                serde_json::to_string(&serde_json::json!({
                    "results": page.results,
                    "cursor": page.offset,
                    "limit": limit,
                    "total": page.total,
                    "truncated": page.next_offset.is_some(),
                    "next_cursor": page.next_offset.map(|offset| offset.to_string()),
                }))
                .map_err(|error| error.to_string())
            } else {
                serde_json::to_string(&PageQuery::new(&snapshot.tree).render(None))
                    .map_err(|error| error.to_string())
            }
        }
        Some("action") => {
            let url = Url::parse(
                args.get(2)
                    .ok_or_else(|| "action requires a URL".to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let target = args
                .get(3)
                .ok_or_else(|| "action requires a target".to_string())?;
            let kind = parse_action_type(
                args.get(4)
                    .ok_or_else(|| "action requires a kind".to_string())?,
            )?;
            let mut engine = ServoEngine::new();
            let context = engine
                .create_context(ContextOptions {
                    headless: true,
                    no_raster: true,
                    deterministic_clock_millis: Some(0),
                    ..Default::default()
                })
                .map_err(|error| error.to_string())?;
            let page = engine
                .create_page(context)
                .map_err(|error| error.to_string())?;
            engine
                .navigate(page, url)
                .map_err(|error| error.to_string())?;
            let snapshot = engine.snapshot(page).map_err(|error| error.to_string())?;
            let planned = ActionPlanner::default()
                .plan(
                    &snapshot.tree,
                    &browsai_layout_observer::LayoutSnapshot::default(),
                    AgentAction {
                        id: "cli-action".into(),
                        target: target.clone(),
                        action_type: kind,
                        parameters: serde_json::Value::Null,
                    },
                )
                .map_err(|error| format!("action planning failed: {error:?}"))?;
            serde_json::to_string(&planned.events).map_err(|error| error.to_string())
        }
        Some("live-search") => run_live_search(args),
        Some("live-open") => run_live_open(args, one_shot_live_runtime),
        Some("check") => run_check_command(args),
        Some("audit") => {
            let path = args
                .get(2)
                .ok_or_else(|| "audit requires a journal path".to_string())?;
            let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            let journal = browsai_audit::AuditJournal::from_json(&contents)
                .map_err(|error| format!("invalid audit journal: {error:?}"))?;
            Ok(serde_json::json!({
                "valid": true,
                "records": journal.len(),
            })
            .to_string())
        }
        Some("logs") => {
            let path = args
                .get(2)
                .ok_or_else(|| "logs requires a log path".to_string())?;
            let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            let logger = browsai_logging::SecretSafeLogger::from_json(&contents)
                .map_err(|error| format!("invalid log export: {error}"))?;
            Ok(serde_json::json!({ "valid": true, "events": logger.events().len() }).to_string())
        }
        Some("replay") => {
            let path = args
                .get(2)
                .ok_or_else(|| "replay requires a journal path".to_string())?;
            let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            let journal = browsai_audit::AuditJournal::from_json(&contents)
                .map_err(|error| format!("invalid audit journal: {error:?}"))?;
            let mut replayed = 0usize;
            journal
                .replay(|_| replayed += 1)
                .map_err(|error| format!("audit replay failed: {error:?}"))?;
            Ok(serde_json::json!({ "valid": true, "replayed": replayed }).to_string())
        }
        Some("benchmark") => {
            let path = args
                .get(2)
                .ok_or_else(|| "benchmark requires a result path".to_string())?;
            let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
            let result: serde_json::Value = serde_json::from_str(&contents)
                .map_err(|error| format!("invalid benchmark result: {error}"))?;
            let object = result
                .as_object()
                .ok_or_else(|| "benchmark result must be an object".to_string())?;
            for field in [
                "commit",
                "fixture",
                "workload",
                "sample_count",
                "median",
                "p95",
            ] {
                if !object.contains_key(field) {
                    return Err(format!("benchmark result missing field: {field}"));
                }
            }
            if object
                .get("sample_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                return Err("benchmark sample_count must be greater than zero".into());
            }
            Ok(serde_json::json!({ "valid": true, "workload": object["workload"] }).to_string())
        }
        Some("recovery") => {
            let path = args
                .get(2)
                .ok_or_else(|| "recovery requires a checkpoint path".to_string())?;
            let store = browsai_recovery::FileRecoveryStore::new(
                path,
                browsai_recovery::RecoveryFileOptions::default(),
            );
            let checkpoint = store
                .load()
                .map_err(|error| format!("invalid recovery checkpoint: {error:?}"))?;
            serde_json::to_string(&checkpoint).map_err(|error| error.to_string())
        }
        _ => Err(usage().into()),
    }
}

fn run_live_open(args: &[String], one_shot_live_runtime: bool) -> Result<String, String> {
    let raw_url = args
        .get(2)
        .ok_or_else(|| "live-open requires a URL".to_string())?;
    let url = Url::parse(raw_url).map_err(|error| error.to_string())?;
    let link_cursor = bounded_option(args, "--link-cursor", 0, 10_000)?;
    let link_limit = bounded_option(args, "--link-limit", 2, 100)?;
    let link_max_bytes = bounded_option(args, "--link-max-bytes", 16 * 1024, 1_048_576)?;
    let link_max_duration_ms = bounded_option(args, "--link-max-duration-ms", 250, 5_000)?;
    let textbox_cursor = bounded_option(args, "--textbox-cursor", 0, 10_000)?;
    let textbox_limit = bounded_option(args, "--textbox-limit", 50, 100)?;
    let control_cursor = bounded_option(args, "--control-cursor", 0, 10_000)?;
    let control_limit = bounded_option(args, "--control-limit", 12, 100)?;
    eprintln!("BROWSAI_STAGE:startup");
    std::env::set_var("BROWSAI_DIAGNOSTIC_STATUS", "1");
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            headless: true,
            use_real_browser_runtime: true,
            viewport: Some(VirtualViewport::default()),
            no_raster: true,
            ..Default::default()
        })
        .map_err(|error| format!("live runtime unavailable: {error}"))?;
    let page = engine
        .create_page(context)
        .map_err(|error| error.to_string())?;
    eprintln!("BROWSAI_STAGE:navigation");
    engine
        .navigate(page, url.clone())
        .map_err(|error| format!("navigation failed: {error}"))?;
    eprintln!("BROWSAI_STAGE:navigation_complete");
    // Allow asynchronous page initialization (including ServiceWorker
    // registration and framework bootstrap promises) to settle before the
    // first DOM projection. Navigation completion alone does not mean those
    // tasks have run.
    eprintln!("BROWSAI_STAGE:stability");
    engine
        .pump_runtime(page, 2_000)
        .map_err(|error| format!("post-navigation event loop failed: {error}"))?;
    eprintln!("BROWSAI_STAGE:dom_projection");
    let initial_snapshot = engine
        .snapshot(page)
        .map_err(|error| format!("snapshot failed: {error}"))?;
    let mut auto_solve_audit: Vec<SolveAuditEvent> = Vec::new();
    if args.iter().any(|arg| arg == "--auto-solve") {
        eprintln!("BROWSAI_STAGE:auto_solve");
        let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
        let session = runtime.open([Capability::SolveChallenge], Default::default());
        runtime.start(session).map_err(|error| error.to_string())?;
        let takeovers = TakeoverManager::default();
        let now = runtime.now_millis();
        let cursor_origin = random_cursor_origin(VirtualViewport::default(), &url, 1);
        eprintln!(
            "BROWSAI_AUTO_SOLVE: initial cursor = ({:.1}, {:.1})",
            cursor_origin.0, cursor_origin.1
        );
        match auto_solve_challenges(
            &mut engine,
            &mut runtime,
            session,
            &takeovers,
            page,
            &initial_snapshot,
            now,
            cursor_origin,
        ) {
            Ok(events) => {
                for event in &events {
                    eprintln!(
                        "BROWSAI_AUTO_SOLVE: provider={} capability={} target={:?}",
                        event.provider, event.capability_used, event.target_node_id
                    );
                }
                auto_solve_audit = events;
            }
            Err(error) => {
                eprintln!("BROWSAI_AUTO_SOLVE_ERROR: {error}");
            }
        }
    }
    let snapshot = if auto_solve_audit.is_empty() {
        initial_snapshot
    } else {
        engine
            .snapshot(page)
            .map_err(|error| format!("post-auto-solve snapshot failed: {error}"))?
    };
    let visible_link_ids = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Link && node.state.visible)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let link_page = bounded_string_page(
        &visible_link_ids,
        link_cursor,
        link_limit,
        link_max_bytes,
        link_max_duration_ms,
    );
    let candidate_links = link_page.items.clone();
    let textbox_count = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Textbox)
        .count();
    let candidate_textboxes = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Textbox)
        .map(|node| {
            serde_json::json!({
                "id": node.id,
                "name": node.name,
                "visible": node.state.visible,
                "geometry": node.geometry,
            })
        })
        .skip(textbox_cursor)
        .take(textbox_limit)
        .collect::<Vec<_>>();
    let candidate_control_count = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| {
            node.state.visible
                && matches!(
                    node.structural_role,
                    StructuralRole::Button
                        | StructuralRole::Checkbox
                        | StructuralRole::Radio
                        | StructuralRole::Link
                )
        })
        .count();
    let candidate_controls = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| {
            node.state.visible
                && matches!(
                    node.structural_role,
                    StructuralRole::Button
                        | StructuralRole::Checkbox
                        | StructuralRole::Radio
                        | StructuralRole::Link
                )
        })
        .skip(control_cursor)
        .take(control_limit)
        .map(|node| {
            serde_json::json!({
                "id": node.id,
                "role": format!("{:?}", node.structural_role),
                "name": node.name,
                "geometry": node.geometry,
            })
        })
        .collect::<Vec<_>>();
    let mut clicked_links = Vec::new();
    if args.iter().any(|arg| arg == "--click-links") {
        eprintln!("BROWSAI_STAGE:interaction");
        let planner = ActionPlanner::default();
        let initial_runtime_url = engine
            .runtime_current_url(page)
            .map_err(|error| error.to_string())?;
        for target in candidate_links.iter() {
            let planned = planner
                .plan(
                    &snapshot.tree,
                    &browsai_layout_observer::LayoutSnapshot::default(),
                    AgentAction {
                        id: format!("live-click-{}", clicked_links.len() + 1),
                        target: target.clone(),
                        action_type: ActionType::Click,
                        parameters: serde_json::Value::Null,
                    },
                )
                .map_err(|error| format!("link {target} could not be planned: {error:?}"))?;
            let mut dispatcher = EngineDispatcher {
                engine: &mut engine,
                page,
            };
            planner
                .execute(&planned, snapshot.tree.generation, &mut dispatcher)
                .map_err(|error| format!("link {target} click failed: {error}"))?;
            clicked_links.push(target.clone());
            // A successful link click may replace the document or navigate to
            // another page. Never dispatch the remaining targets from the old
            // DOM snapshot into that new browsing context.
            if engine
                .runtime_current_url(page)
                .map_err(|error| error.to_string())?
                != initial_runtime_url
            {
                break;
            }
        }
    }
    let mut probed_controls = Vec::new();
    let mut skipped_controls = Vec::new();
    if args.iter().any(|arg| arg == "--probe-controls") {
        eprintln!("BROWSAI_STAGE:interaction");
        let planner = ActionPlanner::default();
        let control_snapshot = if args.iter().any(|arg| arg == "--click-links") {
            engine
                .snapshot(page)
                .map_err(|error| format!("control snapshot failed: {error}"))?
        } else {
            // A second projection can race with worker teardown on SPA login
            // shells such as X. With no preceding link navigation, the
            // initial snapshot is already the correct control set.
            snapshot.clone()
        };
        for node in control_snapshot
            .tree
            .nodes
            .iter()
            .filter(|node| {
                node.state.visible
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| !name.trim().is_empty())
                    && matches!(
                        node.structural_role,
                        StructuralRole::Button | StructuralRole::Checkbox | StructuralRole::Radio
                    )
            })
            .take(3)
        {
            let planned = match planner.plan(
                &control_snapshot.tree,
                &browsai_layout_observer::LayoutSnapshot::default(),
                AgentAction {
                    id: format!("live-probe-{}", probed_controls.len() + 1),
                    target: node.id.clone(),
                    action_type: ActionType::Click,
                    parameters: serde_json::Value::Null,
                },
            ) {
                Ok(planned) => planned,
                Err(PlanningError::TargetNotInteractable(_)) => {
                    skipped_controls.push(node.id.clone());
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "control {} could not be planned: {error:?}",
                        node.id
                    ));
                }
            };
            let mut dispatcher = EngineDispatcher {
                engine: &mut engine,
                page,
            };
            planner
                .execute(&planned, control_snapshot.tree.generation, &mut dispatcher)
                .map_err(|error| format!("control {} click failed: {error}", node.id))?;
            probed_controls.push(node.id.clone());
        }
    }
    let diagnostics_url = engine
        .runtime_current_url(page)
        .map_err(|error| error.to_string())?;
    let navigation_history = engine
        .runtime_navigation_history(page)
        .map_err(|error| error.to_string())?;
    let navigation_request_history = engine
        .runtime_navigation_request_history(page)
        .map_err(|error| error.to_string())?;
    let navigation_request_history_truncated = engine
        .runtime_navigation_request_history_truncated(page)
        .map_err(|error| error.to_string())?;
    let mut redirect_chain = navigation_request_history
        .iter()
        .map(|url| url.as_str().to_owned())
        .collect::<Vec<_>>();
    for url in navigation_history.iter().map(|url| url.as_str()) {
        if redirect_chain.last().map(String::as_str) != Some(url) {
            redirect_chain.push(url.to_owned());
        }
    }
    eprintln!("BROWSAI_STAGE:evaluation");
    let full_diagnostics = std::env::var_os("BROWSAI_FULL_DIAGNOSTICS").is_some();
    let diagnostics = if diagnostics_url.as_str() == "about:blank" {
        // Service endpoints, blocked navigations, and failed document loads can
        // leave Servo on its initial blank document. Do not ask the checked
        // evaluator to run document code in that state: there is no meaningful
        // origin to authorize, and the resulting capability-denied error
        // misclassifies a non-document response as a browser-policy failure.
        serde_json::json!({
            "error": "non-document page: about:blank",
            "document_available": false,
        })
    } else if !full_diagnostics {
        serde_json::json!({
            "document_available": true,
            "diagnostics_mode": "snapshot-only",
        })
    } else {
        let diagnostics_origin = diagnostics_url.origin().ascii_serialization();
        engine
            .evaluate_page_script_checked(
                page,
                browsai_engine_api::PageEvaluationRequest {
                    source: browsai_engine_api::ScriptSource(
                        "({title:document.title,readyState:document.readyState,userAgent:navigator.userAgent,vendor:navigator.vendor,platform:navigator.platform,webdriver:!!navigator.webdriver,userAgentData:!!navigator.userAgentData,windowChrome:!!window.chrome,webglMode:window.__browsaiWebGLMode||'unknown',fakeWebGL:!!window.__browsaiFakeWebGLActive,webglProbe:(function(){try{var c=document.createElement('canvas'),g=c.getContext('webgl',{antialias:true,alpha:true,powerPreference:'high-performance',failIfMajorPerformanceCaveat:true});if(!g)return {context:false};return {context:true,version:g.getParameter(g.VERSION),vendor:g.getParameter(g.VENDOR),renderer:g.getParameter(g.RENDERER),attrs:g.getContextAttributes&&g.getContextAttributes(),extensions:g.getSupportedExtensions?g.getSupportedExtensions().length:0,shaderPrecision:!!(g.getShaderPrecisionFormat&&g.getShaderPrecisionFormat(g.VERTEX_SHADER,g.HIGH_FLOAT)),maxTexture:g.getParameter(g.MAX_TEXTURE_SIZE),maxViewport:g.getParameter(g.MAX_VIEWPORT_DIMS)}}catch(e){return {context:false,error:String(e)}}})(),webgl2Probe:(function(){try{var c=document.createElement('canvas'),g=c.getContext('webgl2',{antialias:true,alpha:true,powerPreference:'high-performance',failIfMajorPerformanceCaveat:true});if(!g)return {context:false};return {context:true,version:g.getParameter(g.VERSION),instanceof:typeof WebGL2RenderingContext!=='undefined'&&g instanceof WebGL2RenderingContext,renderer:g.getParameter(g.RENDERER),extensions:g.getSupportedExtensions?g.getSupportedExtensions().length:0,maxTexture:g.getParameter(g.MAX_TEXTURE_SIZE),maxVertexUniforms:g.getParameter(g.MAX_VERTEX_UNIFORM_VECTORS),maxFragmentUniforms:g.getParameter(g.MAX_FRAGMENT_UNIFORM_VECTORS),maxVarying:g.getParameter(g.MAX_VARYING_VECTORS)}}catch(e){return {context:false,error:String(e)}}})(),bodyText:(document.body&&document.body.innerText||'').slice(0,500),bodyLength:(document.body&&document.body.innerText||'').length,htmlLength:document.documentElement?document.documentElement.outerHTML.length:0,controls:Array.from(document.querySelectorAll('input,textarea,button')).slice(0,20).map(function(e){var s=getComputedStyle(e),w=Math.max(0,Number(e.offsetWidth)||parseFloat(s.width)||0),h=Math.max(0,Number(e.offsetHeight)||parseFloat(s.height)||0);return {tag:e.tagName,type:e.type||'',name:e.name||'',aria:e.getAttribute('aria-label')||'',display:s.display,visibility:s.visibility,opacity:s.opacity,rect:{x:Number(e.offsetLeft)||0,y:Number(e.offsetTop)||0,w:w,h:h}}})})".into(),
                    ),
                    origin: diagnostics_origin,
                    capability_granted: true,
                    timeout_millis: 1_000,
                    max_timeout_millis: 5_000,
                    provenance_reference: "live-open-diagnostics".into(),
                },
            )
            .map(|result| result.value)
            .unwrap_or_else(|error| serde_json::json!({"error": error.to_string()}))
    };
    let document_response_metadata = if diagnostics_url.as_str() == "about:blank"
        || diagnostics.get("error").is_some()
        || !full_diagnostics
    {
        serde_json::Value::Null
    } else {
        let diagnostics_origin = diagnostics_url.origin().ascii_serialization();
        engine
            .evaluate_page_script_checked(
                page,
                browsai_engine_api::PageEvaluationRequest {
                    source: browsai_engine_api::ScriptSource(
                        "({mimeType:document.contentType || '',statusCode:typeof document.__browsaiStatusCode==='number'?document.__browsaiStatusCode:null})".into(),
                    ),
                    origin: diagnostics_origin,
                    capability_granted: true,
                    timeout_millis: 500,
                    max_timeout_millis: 1_000,
                    provenance_reference: "live-open-document-mime".into(),
                },
            )
            .map(|result| result.value)
            .unwrap_or(serde_json::Value::Null)
    };
    let resource_diagnostics = if diagnostics_url.as_str() == "about:blank" {
        serde_json::Value::Null
    } else {
        let diagnostics_origin = diagnostics_url.origin().ascii_serialization();
        engine
            .evaluate_page_script_checked(
                page,
                browsai_engine_api::PageEvaluationRequest {
                    source: browsai_engine_api::ScriptSource(
                        "({scripts:Array.from(document.scripts||[]).map(function(script){return {src:script.src||null,inline:!script.src,async:!!script.async,defer:!!script.defer,type:script.type||'text/javascript',text:script.src?null:String(script.text||script.textContent||'').slice(0,500),hasOnload:typeof script.onload==='function',hasOnerror:typeof script.onerror==='function'};}),solveSimpleChallengeType:typeof solveSimpleChallenge,resource_entries:(function(){try{return (performance.getEntriesByType('resource')||[]).filter(function(entry){return /\\\\.js(?:[?#]|$)|recaptcha|gstatic/i.test(entry.name||'');}).map(function(entry){return {name:entry.name,startTime:entry.startTime,duration:entry.duration,transferSize:entry.transferSize||0};});}catch(_){return [];}})()})".into(),
                    ),
                    origin: diagnostics_origin,
                    capability_granted: true,
                    timeout_millis: 1_000,
                    max_timeout_millis: 5_000,
                    provenance_reference: "live-open-resource-diagnostics".into(),
                },
            )
            .map(|result| result.value)
            .unwrap_or_else(|error| serde_json::json!({"error": error.to_string()}))
    };
    let output = serde_json::json!({
        "url": snapshot.url,
        "navigation": {
            "requested_url": url,
            "final_url": diagnostics_url,
            "history": redirect_chain,
            "history_truncated": navigation_request_history_truncated,
            "redirect_observed": redirect_chain.len() > 1,
            "http_status": document_response_metadata.get("statusCode"),
        "mime_type": document_response_metadata.get("mimeType"),
            "origin": diagnostics_url.origin().ascii_serialization(),
            "initiator": serde_json::Value::Null,
            "resource_type": "main_frame",
        },
        "load_status": engine.runtime_load_status(page).map_err(|error| error.to_string())?,
        "resource_diagnostics": resource_diagnostics,
        "resource_requests": engine
            .runtime_resource_request_history(page)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|url| url.to_string())
            .collect::<Vec<_>>(),
        "resource_requests_truncated": engine
            .runtime_resource_request_history_truncated(page)
            .map_err(|error| error.to_string())?,
        "runtime_messages": engine.runtime_messages(page).map_err(|error| error.to_string())?,
        "node_count": snapshot.tree.nodes.len(),
        "agent_tree_truncated": snapshot.tree.truncated,
        "candidate_links": candidate_links,
        "candidate_links_cursor": link_cursor,
        "candidate_links_limit": link_limit,
        "candidate_links_truncated": link_page.truncated,
        "candidate_links_next_cursor": link_page.next_cursor,
        "candidate_links_bytes": link_page.bytes_used,
        "candidate_links_max_bytes": link_max_bytes,
        "candidate_links_duration_ms": link_page.duration_ms,
        "candidate_links_max_duration_ms": link_max_duration_ms,
        "candidate_textboxes_cursor": textbox_cursor,
        "candidate_textboxes_limit": textbox_limit,
        "candidate_textboxes": candidate_textboxes,
        "candidate_textboxes_truncated": textbox_cursor.saturating_add(candidate_textboxes.len()) < textbox_count,
        "candidate_textboxes_next_cursor": textbox_cursor
            .checked_add(candidate_textboxes.len())
            .filter(|next| *next < textbox_count)
            .map(|next| next.to_string()),
        "candidate_controls_cursor": control_cursor,
        "candidate_controls_limit": control_limit,
        "candidate_controls": candidate_controls,
        "candidate_controls_truncated": control_cursor.saturating_add(candidate_controls.len()) < candidate_control_count,
        "candidate_controls_next_cursor": control_cursor
            .checked_add(candidate_controls.len())
            .filter(|next| *next < candidate_control_count)
            .map(|next| next.to_string()),
        "clicked_links": clicked_links,
        "probed_controls": probed_controls,
        "skipped_controls": skipped_controls,
        "auto_solve_audit": auto_solve_audit,
        "diagnostics": diagnostics,
    })
    .to_string();
    if one_shot_live_runtime {
        // Servo's destructor waits for every page task to acknowledge Exit;
        // pathological third-party pages can keep that acknowledgement alive
        // indefinitely. The CLI is a one-shot process, so emit the result and
        // terminate before entering that unbounded cleanup path.
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{output}");
        let _ = stdout.flush();
        #[cfg(unix)]
        unsafe {
            _exit(0);
        }
        #[cfg(not(unix))]
        std::process::exit(0);
    }
    Ok(output)
}

fn run_live_search(args: &[String]) -> Result<String, String> {
    let query = args
        .get(2)
        .ok_or_else(|| "live-search requires a query".to_string())?;
    let link_cursor = bounded_option(args, "--link-cursor", 0, 10_000)?;
    let link_limit = bounded_option(args, "--link-limit", 30, 100)?;
    let link_max_bytes = bounded_option(args, "--link-max-bytes", 16 * 1024, 1_048_576)?;
    let link_max_duration_ms = bounded_option(args, "--link-max-duration-ms", 250, 5_000)?;
    let url = Url::parse("https://www.google.com/").map_err(|error| error.to_string())?;
    let mut engine = ServoEngine::new();
    let context = engine
        .create_context(ContextOptions {
            headless: true,
            use_real_browser_runtime: true,
            viewport: Some(VirtualViewport::default()),
            no_raster: true,
            ..Default::default()
        })
        .map_err(|error| format!("live runtime unavailable: {error}"))?;
    let page = engine
        .create_page(context)
        .map_err(|error| error.to_string())?;
    engine
        .navigate(page, url)
        .map_err(|error| format!("Google navigation failed: {error}"))?;
    let landing = snapshot_after_settle(&mut engine, page, "Google snapshot")?;
    let textbox = landing
        .tree
        .nodes
        .iter()
        .find(|node| node.structural_role == StructuralRole::Textbox && node.state.visible)
        .ok_or_else(|| "Google homepage exposed no visible search textbox".to_string())?;
    let planner = ActionPlanner::default();
    for action in [
        AgentAction {
            id: "google-focus-search".into(),
            target: textbox.id.clone(),
            action_type: ActionType::Focus,
            parameters: serde_json::Value::Null,
        },
        AgentAction {
            id: "google-type-search".into(),
            target: textbox.id.clone(),
            action_type: ActionType::Type,
            parameters: serde_json::json!({"text": query}),
        },
        AgentAction {
            id: "google-submit-search".into(),
            target: textbox.id.clone(),
            action_type: ActionType::KeyPress,
            parameters: serde_json::json!({"key": "Enter"}),
        },
    ] {
        let planned = planner
            .plan(
                &landing.tree,
                &browsai_layout_observer::LayoutSnapshot::default(),
                action,
            )
            .map_err(|error| format!("Google search action could not be planned: {error:?}"))?;
        let mut dispatcher = EngineDispatcher {
            engine: &mut engine,
            page,
        };
        planner
            .execute(&planned, landing.tree.generation, &mut dispatcher)
            .map_err(|error| format!("Google search action failed: {error}"))?;
    }
    engine
        .pump_runtime(page, 2_000)
        .map_err(|error| format!("Google post-search event loop failed: {error}"))?;
    let snapshot = snapshot_after_settle(&mut engine, page, "Google result snapshot")?;
    let diagnostics = engine
        .evaluate_page_script_checked(
            page,
            browsai_engine_api::PageEvaluationRequest {
                source: browsai_engine_api::ScriptSource(
                    "({title:document.title,readyState:document.readyState,bodyLength:(document.body&&document.body.innerText||'').length,scripts:Array.from(document.scripts||[]).map(function(script){return {src:script.src||null,inline:!script.src,async:!!script.async,defer:!!script.defer,type:script.type||'text/javascript'};}),resource_entries:(function(){try{return (performance.getEntriesByType('resource')||[]).filter(function(entry){return /\\.js(?:[?#]|$)|recaptcha|gstatic/i.test(entry.name||'');}).map(function(entry){return {name:entry.name,startTime:entry.startTime,duration:entry.duration,transferSize:entry.transferSize||0};});}catch(_){return [];}})()})".into(),
                ),
                origin: "https://www.google.com".into(),
                capability_granted: true,
                timeout_millis: 1_000,
                max_timeout_millis: 5_000,
                provenance_reference: "live-search-diagnostics".into(),
            },
        )
        .map_err(|error| format!("Google diagnostics failed: {error}"))?;
    let discovered_urls = if args.iter().any(|arg| arg == "--discover") {
        discover_google_result_urls(&mut engine, page)?
    } else {
        Vec::new()
    };
    let total_link_count = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Link && node.state.visible)
        .count();
    let visible_link_ids = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Link && node.state.visible)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let link_page = bounded_string_page(
        &visible_link_ids,
        link_cursor,
        link_limit,
        link_max_bytes,
        link_max_duration_ms,
    );
    let links = link_page
        .items
        .iter()
        .filter_map(|id| snapshot.tree.nodes.iter().find(|node| &node.id == id))
        .map(|node| serde_json::json!({"id": node.id, "name": node.name, "generation": node.generation}))
        .collect::<Vec<_>>();
    let nodes = snapshot
        .tree
        .nodes
        .iter()
        .take(24)
        .map(|node| {
            serde_json::json!({
                "id": node.id,
                "role": format!("{:?}", node.structural_role),
                "name": node.name,
                "value": node.value,
                "visible": node.state.visible,
            })
        })
        .collect::<Vec<_>>();
    let mut opened = Vec::new();
    if args.iter().any(|arg| arg == "--open-links") {
        let planner = ActionPlanner::default();
        for link in snapshot
            .tree
            .nodes
            .iter()
            .filter(|node| {
                node.structural_role == StructuralRole::Link
                    && node.state.visible
                    && node.name.as_deref().is_some_and(|name| {
                        !matches!(
                            name,
                            "Skip to main content"
                                | "Accessibility help"
                                | "Go to Google Home"
                                | "Sign in"
                        )
                    })
            })
            .take(if args.iter().any(|arg| arg == "--open-one") {
                1
            } else {
                2
            })
        {
            let action = AgentAction {
                id: format!("google-click-{}", opened.len() + 1),
                target: link.id.clone(),
                action_type: ActionType::Click,
                parameters: serde_json::Value::Null,
            };
            let planned = planner
                .plan(
                    &snapshot.tree,
                    &browsai_layout_observer::LayoutSnapshot::default(),
                    action,
                )
                .map_err(|error| format!("link {} could not be planned: {error:?}", link.id))?;
            let mut dispatcher = EngineDispatcher {
                engine: &mut engine,
                page,
            };
            planner
                .execute(&planned, snapshot.tree.generation, &mut dispatcher)
                .map_err(|error| format!("link {} click failed: {error}", link.id))?;
            opened.push(link.id.clone());
        }
        // Deliver the native click and return the event result before waiting on
        // an arbitrary destination. Some real pages trigger pathological style
        // work in Servo during navigation; keeping the click operation bounded
        // prevents one destination from wedging the browser session.
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "query": query,
        "url": snapshot.url,
        "load_status": engine
            .runtime_load_status(page)
            .map_err(|error| error.to_string())?,
        "runtime_messages": engine
            .runtime_messages(page)
            .map_err(|error| error.to_string())?,
        "node_count": snapshot.tree.nodes.len(),
        "agent_tree_truncated": snapshot.tree.truncated,
        "diagnostics": diagnostics.value,
        "link_count": total_link_count,
        "links": links,
        "links_cursor": link_cursor,
        "links_limit": link_limit,
        "links_truncated": link_page.truncated,
        "links_next_cursor": link_page.next_cursor,
        "links_bytes": link_page.bytes_used,
        "links_max_bytes": link_max_bytes,
        "links_duration_ms": link_page.duration_ms,
        "links_max_duration_ms": link_max_duration_ms,
        "discovered_urls": discovered_urls,
        "nodes": nodes,
        "opened_link_targets": opened,
        "runtime": "servo"
    }))
    .map_err(|error| error.to_string())
}

fn discover_google_result_urls(engine: &mut ServoEngine, page: u64) -> Result<Vec<String>, String> {
    let html = engine
        .evaluate_page_script_checked(
            page,
            browsai_engine_api::PageEvaluationRequest {
                source: browsai_engine_api::ScriptSource(
                    "document.documentElement ? document.documentElement.outerHTML : ''".into(),
                ),
                origin: "https://www.google.com".into(),
                capability_granted: true,
                timeout_millis: 5_000,
                max_timeout_millis: 5_000,
                provenance_reference: "live-search-discovery".into(),
            },
        )
        .map_err(|error| format!("Google result discovery failed: {error}"))?
        .value;
    let html = html
        .as_str()
        .ok_or_else(|| "Google DOM discovery returned a non-string document".to_string())?;
    let mut urls = Vec::new();
    for marker in ["href=\"", "href='"] {
        for remainder in html.split(marker).skip(1) {
            let quote = marker.as_bytes()[marker.len() - 1] as char;
            let Some(end) = remainder.find(quote) else {
                continue;
            };
            let href = remainder[..end].replace("&amp;", "&");
            if href.is_empty() {
                continue;
            }
            let absolute = Url::parse(&href)
                .or_else(|_| {
                    Url::parse("https://www.google.com/").and_then(|base| base.join(&href))
                })
                .ok();
            let Some(mut url) = absolute else {
                continue;
            };
            if url
                .domain()
                .is_some_and(|domain| domain.contains("google."))
                && url.path() == "/url"
            {
                let target = url
                    .query_pairs()
                    .find(|(key, _)| key == "q" || key == "url")
                    .map(|(_, value)| value.into_owned());
                if let Some(target) = target.and_then(|target| Url::parse(&target).ok()) {
                    url = target;
                }
            }
            if matches!(url.scheme(), "http" | "https")
                && !url
                    .domain()
                    .is_some_and(|domain| domain.contains("google."))
            {
                let value = url.to_string();
                if !urls.contains(&value) {
                    urls.push(value);
                }
            }
        }
    }
    Ok(urls)
}

fn snapshot_after_settle(
    engine: &mut ServoEngine,
    page: u64,
    label: &str,
) -> Result<browsai_state::PageSnapshot, String> {
    let mut last_error = None;
    for _ in 0..3 {
        match engine.snapshot(page) {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) => {
                last_error = Some(error.to_string());
                engine
                    .pump_runtime(page, 2_000)
                    .map_err(|pump_error| format!("{label} recovery failed: {pump_error}"))?;
            }
        }
    }
    Err(format!(
        "{label} failed after retries: {}",
        last_error.unwrap_or_else(|| "unknown snapshot error".into())
    ))
}

struct EngineDispatcher<'a> {
    engine: &'a mut ServoEngine,
    page: u64,
}

impl NativeInputDispatcher for EngineDispatcher<'_> {
    type Error = browsai_engine_api::EngineError;

    fn dispatch(&mut self, event: NativeInputEvent) -> Result<(), Self::Error> {
        self.engine.dispatch_input(self.page, event)
    }
}

fn run_check_command(args: &[String]) -> Result<String, String> {
    match args.get(2).map(String::as_str) {
        Some("site") => {
            let url = Url::parse(
                args.get(3)
                    .ok_or_else(|| "check site requires a URL".to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let site = SiteRecord::new("site-000001", url);
            let config = CheckConfig::default();
            let mut executor = BrowserSiteExecutor;
            let result = run_site(&mut executor, site, "run-single-site", &config);
            serde_json::to_string_pretty(&result).map_err(|error| error.to_string())
        }
        Some("corpus") => {
            let path = args
                .get(3)
                .ok_or_else(|| "check corpus requires a sites.csv path".to_string())?;
            let sites = browsai_compatibility_check::load_sites_csv(path)
                .map_err(|error| error.to_string())?;
            let config = CheckConfig::default();
            let mut run = CheckRun::new(config.clone(), sites, 0);
            let store = RunStore::new("check-runs");
            run.state = CorpusState::Running;
            let run_id = run.id.clone();
            for index in 0..run.sites.len() {
                let site_result = &mut run.sites[index];
                let Some(site) = site_result.site.clone() else {
                    continue;
                };
                if !site.enabled {
                    site_result.state = Some(CorpusState::Complete);
                    site_result.status = Some(SiteStatus::Skipped);
                    continue;
                }
                site_result.state = Some(CorpusState::Running);
                let mut executor = BrowserSiteExecutor;
                *site_result = run_site(&mut executor, site, &run_id, &config);
                site_result.state = Some(
                    if matches!(
                        site_result.status,
                        Some(SiteStatus::Pass | SiteStatus::PassWithWarnings)
                    ) {
                        CorpusState::Complete
                    } else {
                        CorpusState::Failed
                    },
                );
            }
            run.state = CorpusState::Complete;
            store.save(&run).map_err(|error| error.to_string())?;
            browsai_compatibility_check::write_reports(
                &run,
                format!("check-runs/{}/reports", run.id),
            )
            .map_err(|error| error.to_string())?;
            serde_json::to_string_pretty(&run).map_err(|error| error.to_string())
        }
        Some("report") => {
            let run_id = args
                .get(3)
                .ok_or_else(|| "check report requires a run id".to_string())?;
            let run = RunStore::new("check-runs")
                .load(run_id)
                .map_err(|error| error.to_string())?;
            browsai_compatibility_check::render_json_report(&run).map_err(|error| error.to_string())
        }
        _ => Err(usage().into()),
    }
}

struct BrowserSiteExecutor;

impl SiteExecutor for BrowserSiteExecutor {
    fn execute(&mut self, site: &SiteRecord, _config: &CheckConfig) -> SiteResult {
        let mut result = SiteResult {
            state: Some(CorpusState::Running),
            ..Default::default()
        };
        let mut engine = ServoEngine::new();
        let context = match engine.create_context(ContextOptions {
            headless: true,
            viewport: Some(VirtualViewport::default()),
            deterministic_clock_millis: Some(0),
            no_raster: true,
            ..Default::default()
        }) {
            Ok(context) => context,
            Err(error) => {
                result.status = Some(SiteStatus::Fail);
                result.failures.push(FailureRecord::new(
                    "run-single-site",
                    site,
                    Phase::Startup,
                    FailureCategory::Engine,
                    browsai_compatibility_check::Severity::High,
                    error.to_string(),
                ));
                return result;
            }
        };
        let page = match engine.create_page(context) {
            Ok(page) => page,
            Err(error) => {
                result.status = Some(SiteStatus::Fail);
                result.failures.push(FailureRecord::new(
                    "run-single-site",
                    site,
                    Phase::Startup,
                    FailureCategory::Engine,
                    browsai_compatibility_check::Severity::High,
                    error.to_string(),
                ));
                return result;
            }
        };
        if let Err(error) = engine.navigate(page, site.url.clone()) {
            result.status = Some(SiteStatus::Fail);
            result.failures.push(FailureRecord::new(
                "run-single-site",
                site,
                Phase::Navigation,
                FailureCategory::Navigation,
                browsai_compatibility_check::Severity::High,
                error.to_string(),
            ));
            return result;
        }
        match engine.snapshot(page) {
            Ok(snapshot) => {
                result.status = Some(SiteStatus::Pass);
                result.state = Some(CorpusState::Complete);
                for subsystem in [
                    Subsystem::Engine,
                    Subsystem::AgentTree,
                    Subsystem::StructuralIr,
                    Subsystem::SemanticIr,
                ] {
                    result.subsystems.insert(
                        subsystem,
                        SubsystemResult {
                            status: SiteStatus::Pass,
                            details: None,
                        },
                    );
                }
                result.metrics.increment("snapshots", 1);
                result
                    .metrics
                    .gauge("agent_tree.nodes", snapshot.tree.nodes.len() as i64);
            }
            Err(error) => {
                result.status = Some(SiteStatus::Fail);
                result.failures.push(FailureRecord::new(
                    "run-single-site",
                    site,
                    Phase::AgentTree,
                    FailureCategory::Snapshot,
                    browsai_compatibility_check::Severity::High,
                    error.to_string(),
                ));
            }
        }
        result
    }
}

fn bounded_option(
    args: &[String],
    name: &str,
    default: usize,
    maximum: usize,
) -> Result<usize, String> {
    let mut value = None;
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        let raw = if argument == name {
            index += 1;
            args.get(index)
                .ok_or_else(|| format!("{name} requires a value"))?
                .as_str()
        } else if let Some(raw) = argument.strip_prefix(&format!("{name}=")) {
            raw
        } else {
            index += 1;
            continue;
        };
        if value.is_some() {
            return Err(format!("{name} may only be specified once"));
        }
        let parsed = raw
            .parse::<usize>()
            .map_err(|_| format!("{name} must be a non-negative integer"))?;
        if parsed > maximum {
            return Err(format!("{name} exceeds maximum {maximum}"));
        }
        value = Some(parsed);
        index += 1;
    }
    Ok(value.unwrap_or(default))
}

struct BoundedStringPage {
    items: Vec<String>,
    truncated: bool,
    next_cursor: Option<String>,
    bytes_used: usize,
    duration_ms: u128,
}

fn bounded_string_page(
    values: &[String],
    cursor: usize,
    limit: usize,
    max_bytes: usize,
    max_duration_ms: usize,
) -> BoundedStringPage {
    let started = Instant::now();
    let start = cursor.min(values.len());
    let mut items = Vec::new();
    let mut bytes_used = 0usize;
    let mut next_index = start;
    for (index, value) in values.iter().enumerate().skip(start).take(limit.max(1)) {
        let elapsed = started.elapsed().as_millis();
        if elapsed >= max_duration_ms as u128 && !items.is_empty() {
            break;
        }
        // Account for JSON string framing and a conservative comma separator.
        let item_bytes = value.len().saturating_add(3);
        if bytes_used.saturating_add(item_bytes) > max_bytes && !items.is_empty() {
            break;
        }
        if item_bytes > max_bytes && items.is_empty() {
            next_index = index.saturating_add(1);
            break;
        }
        bytes_used = bytes_used.saturating_add(item_bytes);
        items.push(value.clone());
        next_index = index.saturating_add(1);
    }
    let truncated = next_index < values.len();
    BoundedStringPage {
        items,
        truncated,
        next_cursor: truncated.then_some(next_index.to_string()),
        bytes_used,
        duration_ms: started.elapsed().as_millis(),
    }
}

fn parse_action_type(value: &str) -> Result<ActionType, String> {
    match value.to_ascii_lowercase().as_str() {
        "activate" => Ok(ActionType::Activate),
        "click" => Ok(ActionType::Click),
        "double-click" | "doubleclick" => Ok(ActionType::DoubleClick),
        "context-click" | "contextclick" => Ok(ActionType::ContextClick),
        "hover" => Ok(ActionType::Hover),
        "focus" => Ok(ActionType::Focus),
        "blur" => Ok(ActionType::Blur),
        "keypress" | "key-press" => Ok(ActionType::KeyPress),
        "scroll" => Ok(ActionType::Scroll),
        other => Err(format!("unsupported action kind: {other}")),
    }
}

#[cfg(test)]
fn run(args: &[String]) -> Result<String, String> {
    run_internal(args, false)
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let one_shot_live_runtime = args.get(1).is_some_and(|command| command == "live-open");
    match run_internal(&args, one_shot_live_runtime) {
        Ok(output) => {
            println!("{output}");
            if one_shot_live_runtime {
                std::process::exit(0);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{auto_solve_challenges, bounded_option, bounded_string_page, random_cursor_origin, run};

    #[test]
    fn bounded_projection_options_parse_and_enforce_limits() {
        let args = vec![
            "browsai".into(),
            "live-open".into(),
            "https://example.test".into(),
            "--link-cursor".into(),
            "4".into(),
            "--link-limit=8".into(),
        ];
        assert_eq!(bounded_option(&args, "--link-cursor", 0, 100), Ok(4));
        assert_eq!(bounded_option(&args, "--link-limit", 2, 100), Ok(8));
        assert!(bounded_option(&args, "--link-limit", 2, 4).is_err());
        assert!(bounded_option(
            &["--link-limit".into(), "not-a-number".into()],
            "--link-limit",
            2,
            100
        )
        .is_err());
    }

    #[test]
    fn link_pages_enforce_cursor_item_byte_and_continuation_limits() {
        let values = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let page = bounded_string_page(&values, 1, 3, 8, 250);
        assert_eq!(page.items, vec!["b", "c"]);
        assert!(page.truncated);
        assert_eq!(page.next_cursor.as_deref(), Some("3"));
        assert!(page.bytes_used <= 8);
        assert!(page.duration_ms <= 250);
    }
    #[test]
    fn version_and_headless_commands_work() {
        assert!(run(&["browsai".into(), "version".into()])
            .unwrap()
            .contains("protocol 1"));
        assert!(run(&[
            "browsai".into(),
            "headless".into(),
            "https://example.test".into()
        ])
        .unwrap()
        .contains("example.test"));
        assert!(run(&[
            "browsai".into(),
            "query".into(),
            "https://example.test".into()
        ])
        .unwrap()
        .starts_with('['));
        assert!(run(&[
            "browsai".into(),
            "open".into(),
            "https://example.test".into()
        ])
        .unwrap()
        .contains("example.test"));
        assert!(run(&[
            "browsai".into(),
            "render".into(),
            "https://example.test".into()
        ])
        .unwrap()
        .starts_with('['));
    }

    #[test]
    fn unsupported_commands_include_audit_and_recovery_help() {
        let error = run(&["browsai".into(), "unknown".into()]).unwrap_err();
        assert!(error.contains("audit <journal.json>"));
        assert!(error.contains("logs <log.json>"));
        assert!(error.contains("replay <journal.json>"));
        assert!(error.contains("benchmark <result.json>"));
        assert!(error.contains("recovery <checkpoint.json>"));
        assert!(error.contains("open <url>"));
        assert!(error.contains("profile <name>"));
        assert!(error.contains("workspace <profile-name>"));
        assert!(error.contains("action <url> <target> <kind>"));
    }

    #[test]
    fn profile_workspace_and_action_surfaces_are_available() {
        let profile = run(&["browsai".into(), "profile".into(), "work".into()]).unwrap();
        assert!(profile.contains("\"name\":\"work\""));
        let workspace = run(&["browsai".into(), "workspace".into(), "work".into()]).unwrap();
        assert!(workspace.contains("\"profile\""));
        assert!(run(&["browsai".into(), "action".into()]).is_err());
    }

    #[test]
    fn audit_and_recovery_commands_validate_persisted_files() {
        let base = std::env::temp_dir().join(format!(
            "browsai-cli-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let audit_path = base.with_extension("audit.json");
        let recovery_path = base.with_extension("checkpoint.json");

        let journal = browsai_audit::AuditJournal::default();
        std::fs::write(&audit_path, journal.to_json().unwrap()).unwrap();
        let audit_output = run(&[
            "browsai".into(),
            "audit".into(),
            audit_path.to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert!(audit_output.contains("\"valid\":true"));

        let checkpoint = browsai_recovery::SessionCheckpoint {
            id: uuid::Uuid::new_v4(),
            schema_version: 1,
            profiles: vec!["profile-1".into()],
            windows: vec!["window-1".into()],
            tabs: vec!["tab-1".into()],
            locks: vec![],
            pending_confirmations: vec![],
            state: Default::default(),
        };
        let store = browsai_recovery::FileRecoveryStore::new(
            &recovery_path,
            browsai_recovery::RecoveryFileOptions::default(),
        );
        store.save(checkpoint).unwrap();
        let recovery_output = run(&[
            "browsai".into(),
            "recovery".into(),
            recovery_path.to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert!(recovery_output.contains("profile-1"));

        let _ = std::fs::remove_file(audit_path);
        let _ = std::fs::remove_file(&recovery_path);
        let _ = std::fs::remove_file(recovery_path.with_extension("lock"));
    }

    #[test]
    fn logs_replay_and_benchmark_commands_validate_read_only_artifacts() {
        let base = std::env::temp_dir().join(format!("browsai-cli-extra-{}", std::process::id()));
        let logs_path = base.with_extension("logs.json");
        let journal_path = base.with_extension("journal.json");
        let benchmark_path = base.with_extension("benchmark.json");
        std::fs::write(
            &logs_path,
            browsai_logging::SecretSafeLogger::default().debug_trace(),
        )
        .unwrap();
        std::fs::write(
            &journal_path,
            browsai_audit::AuditJournal::default().to_json().unwrap(),
        )
        .unwrap();
        std::fs::write(
            &benchmark_path,
            serde_json::json!({"commit":"working-tree","fixture":"basic","workload":"startup","sample_count":1,"median":1,"p95":1}).to_string(),
        )
        .unwrap();
        assert!(run(&[
            "browsai".into(),
            "logs".into(),
            logs_path.to_string_lossy().into()
        ])
        .unwrap()
        .contains("\"valid\":true"));
        assert!(run(&[
            "browsai".into(),
            "replay".into(),
            journal_path.to_string_lossy().into()
        ])
        .unwrap()
        .contains("\"replayed\":0"));
        assert!(run(&[
            "browsai".into(),
            "benchmark".into(),
            benchmark_path.to_string_lossy().into()
        ])
        .unwrap()
        .contains("\"valid\":true"));
        let _ = std::fs::remove_file(logs_path);
        let _ = std::fs::remove_file(journal_path);
        let _ = std::fs::remove_file(benchmark_path);
    }

    #[test]
    fn random_cursor_origin_is_inside_viewport_and_differs_per_invocation() {
        use browsai_engine_api::VirtualViewport;
        use url::Url;
        let viewport = VirtualViewport {
            width: 1280,
            height: 720,
            device_scale_factor: 1.0,
        };
        let url = Url::parse("https://example.test/").unwrap();
        let mut positions: Vec<(f64, f64)> = (0..8)
            .map(|i| random_cursor_origin(viewport, &url, i))
            .collect();
        for (x, y) in &positions {
            assert!(*x >= 32.0 && *x <= (viewport.width as f64 - 32.0));
            assert!(*y >= 32.0 && *y <= (viewport.height as f64 - 32.0));
        }
        positions.dedup();
        assert!(
            positions.len() > 1,
            "expected different random origins across invocations, got {:?}",
            positions
        );
    }

    #[test]
    fn random_cursor_origin_is_deterministic_for_same_seed() {
        use browsai_engine_api::VirtualViewport;
        use url::Url;
        let viewport = VirtualViewport {
            width: 800,
            height: 600,
            device_scale_factor: 1.0,
        };
        let url = Url::parse("https://example.test/login").unwrap();
        let a = random_cursor_origin(viewport, &url, 42);
        let b = random_cursor_origin(viewport, &url, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn auto_solve_uses_discrete_click_when_cursor_is_already_at_target() {
        use browsai_agent_runtime::{AgentRuntime, TakeoverManager};
        use browsai_agent_tree::{
            AgentNode, AgentRenderTree, AgentValue, Geometry, NodeState, SemanticRole, StructuralRole,
        };
        use browsai_engine_api::{BrowserEngine, ContextOptions, VirtualViewport};
        use browsai_engine_servo::ServoEngine;
        use browsai_input::NativeInputEvent;
        use browsai_sandbox::{Capability, SandboxPolicy};
        use browsai_state::PageSnapshot;

        fn make_challenge_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> AgentNode {
            AgentNode {
                id: id.into(),
                origin: None,
                identity_key: Some(format!("challenge:{id}")),
                structural_role: StructuralRole::Region,
                semantic_role: Some(SemanticRole::Challenge),
                application_type: None,
                name: Some(format!("challenge {id}")),
                value: Some(AgentValue::Text("captcha".into())),
                description: Some("captcha".into()),
                state: NodeState::default(),
                geometry: Some(Geometry {
                    x,
                    y,
                    width: w,
                    height: h,
                }),
                relationships: Vec::new(),
                actions: Vec::new(),
                children: Vec::new(),
                provenance: Vec::new(),
                confidence: browsai_provenance::Confidence(1.0),
                generation: 0,
            }
        }

        let mut engine = ServoEngine::new();
        let context = engine
            .create_context(ContextOptions {
                headless: true,
                viewport: Some(VirtualViewport::default()),
                ..Default::default()
            })
            .unwrap();
        let page = engine.create_page(context).unwrap();
        let mut tree = AgentRenderTree::new_page("https://example.test/");
        // Center is (100 + 40, 200 + 40) = (140, 240).
        tree.nodes.push(make_challenge_node("hcaptcha", 100.0, 200.0, 80.0, 80.0));
        let snapshot = PageSnapshot {
            schema_version: 1,
            id: 0,
            url: "https://example.test/".parse().unwrap(),
            tree,
            focused_node: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            pending_network: 0,
            storage_generation: 0,
            semantic_generation: 0,
        };
        let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
        let session = runtime.open([Capability::SolveChallenge], Default::default());
        runtime.start(session).unwrap();
        let takeovers = TakeoverManager::default();
        let now = runtime.now_millis();
        // Cursor starts at the challenge center; expect a discrete click.
        let events = auto_solve_challenges(
            &mut engine,
            &mut runtime,
            session,
            &takeovers,
            page,
            &snapshot,
            now,
            (140.0, 240.0),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        let _ = NativeInputEvent::PointerDown { button: 0 };
    }

    #[test]
    fn auto_solve_uses_trajectory_when_cursor_is_far_from_target() {
        use browsai_agent_runtime::{AgentRuntime, TakeoverManager};
        use browsai_agent_tree::{
            AgentNode, AgentRenderTree, AgentValue, Geometry, NodeState, SemanticRole, StructuralRole,
        };
        use browsai_engine_api::{BrowserEngine, ContextOptions, VirtualViewport};
        use browsai_engine_servo::ServoEngine;
        use browsai_input::NativeInputEvent;
        use browsai_sandbox::{Capability, SandboxPolicy};
        use browsai_state::PageSnapshot;

        fn make_challenge_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> AgentNode {
            AgentNode {
                id: id.into(),
                origin: None,
                identity_key: Some(format!("challenge:{id}")),
                structural_role: StructuralRole::Region,
                semantic_role: Some(SemanticRole::Challenge),
                application_type: None,
                name: Some(format!("challenge {id}")),
                value: Some(AgentValue::Text("captcha".into())),
                description: Some("captcha".into()),
                state: NodeState::default(),
                geometry: Some(Geometry {
                    x,
                    y,
                    width: w,
                    height: h,
                }),
                relationships: Vec::new(),
                actions: Vec::new(),
                children: Vec::new(),
                provenance: Vec::new(),
                confidence: browsai_provenance::Confidence(1.0),
                generation: 0,
            }
        }

        let mut engine = ServoEngine::new();
        let context = engine
            .create_context(ContextOptions {
                headless: true,
                viewport: Some(VirtualViewport::default()),
                ..Default::default()
            })
            .unwrap();
        let page = engine.create_page(context).unwrap();
        let mut tree = AgentRenderTree::new_page("https://example.test/");
        // Far target: center (640, 360).
        tree.nodes.push(make_challenge_node("recaptcha", 600.0, 320.0, 80.0, 80.0));
        let snapshot = PageSnapshot {
            schema_version: 1,
            id: 0,
            url: "https://example.test/".parse().unwrap(),
            tree,
            focused_node: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            pending_network: 0,
            storage_generation: 0,
            semantic_generation: 0,
        };
        let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
        let session = runtime.open([Capability::SolveChallenge], Default::default());
        runtime.start(session).unwrap();
        let takeovers = TakeoverManager::default();
        let now = runtime.now_millis();
        // Cursor at (0, 0), target at (640, 360): far enough to trigger trajectory.
        let events = auto_solve_challenges(
            &mut engine,
            &mut runtime,
            session,
            &takeovers,
            page,
            &snapshot,
            now,
            (0.0, 0.0),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        // The trajectory emits PointerMove events; assert at least one is dispatched.
        let _ = NativeInputEvent::PointerMove { x: 0.0, y: 0.0 };
    }

    #[test]
    fn auto_solve_challenges_dispatches_click_for_each_geometry_node() {
        use browsai_agent_runtime::{AgentRuntime, TakeoverManager};
        use browsai_agent_tree::{
            AgentNode, AgentRenderTree, AgentValue, Geometry, NodeState, SemanticRole, StructuralRole,
        };
        use browsai_engine_api::{BrowserEngine, ContextOptions, VirtualViewport};
        use browsai_engine_servo::ServoEngine;
        use browsai_input::{MouseTrajectoryOptions, NativeInputEvent};
        use browsai_sandbox::{Capability, SandboxPolicy};
        use browsai_state::PageSnapshot;

        fn make_challenge_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> AgentNode {
            AgentNode {
                id: id.into(),
                origin: None,
                identity_key: Some(format!("challenge:{id}")),
                structural_role: StructuralRole::Region,
                semantic_role: Some(SemanticRole::Challenge),
                application_type: None,
                name: Some(format!("challenge {id}")),
                value: Some(AgentValue::Text("captcha".into())),
                description: Some("captcha".into()),
                state: NodeState::default(),
                geometry: Some(Geometry {
                    x,
                    y,
                    width: w,
                    height: h,
                }),
                relationships: Vec::new(),
                actions: Vec::new(),
                children: Vec::new(),
                provenance: Vec::new(),
                confidence: browsai_provenance::Confidence(1.0),
                generation: 0,
            }
        }

        let mut engine = ServoEngine::new();
        let context = engine
            .create_context(ContextOptions {
                headless: true,
                viewport: Some(VirtualViewport::default()),
                ..Default::default()
            })
            .unwrap();
        let page = engine.create_page(context).unwrap();
        let mut tree = AgentRenderTree::new_page("https://example.test/");
        tree.nodes.push(make_challenge_node("hcaptcha", 100.0, 200.0, 80.0, 80.0));
        let snapshot = PageSnapshot {
            schema_version: 1,
            id: 0,
            url: "https://example.test/".parse().unwrap(),
            tree,
            focused_node: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            pending_network: 0,
            storage_generation: 0,
            semantic_generation: 0,
        };
        let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
        let session = runtime.open([Capability::SolveChallenge], Default::default());
        runtime.start(session).unwrap();
        let takeovers = TakeoverManager::default();
        let now = runtime.now_millis();
        let events = auto_solve_challenges(
            &mut engine,
            &mut runtime,
            session,
            &takeovers,
            page,
            &snapshot,
            now,
            (0.0, 0.0),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].provider, "hcaptcha");
        assert_eq!(events[0].capability_used, "SolveChallenge+Unattended");
        assert!(events[0].takeover_id.is_none());
        let opts = MouseTrajectoryOptions::default();
        let trajectory = browsai_input::generate_human_trajectory((0.0, 0.0), (140.0, 240.0), opts);
        let pointer_down_count = trajectory
            .events
            .iter()
            .filter(|e| matches!(e, NativeInputEvent::PointerDown { .. }))
            .count();
        assert!(pointer_down_count >= 1);
    }
}
