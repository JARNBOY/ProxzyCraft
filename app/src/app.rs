use crate::devices;
use crate::model::*;
use crate::proxy;
use crate::studio;
use crate::studio::{
    AndroidTransport, ApiTest, ExpectSource, ScreenPlay, ScreenStep, StepKind, StepOutcome,
    StepStatus, TargetPlatform,
};
use crate::theme::*;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Layout, Margin, Rect, RichText, ScrollArea,
    Sense, Stroke, TextStyle, Ui, Vec2,
};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Traffic,
    Live,
    Composer,
    MapLocal,
    Breakpoints,
    Logs,
    Agent,
    Tests,
    Screen,
    Acp,
    Guide,
    Tutorial,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MapViewMode {
    Table,
    Pipeline,
}

/// One row in the all-features palette. Keeping the action separate from the
/// view lets the same list offer screens and small jobs without a second menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PaletteAction {
    Go(View),
    StartTour,
    RunPractice,
    ClearTraffic,
    CopySummary,
}

#[derive(Clone, Copy)]
struct PaletteEntry {
    group: &'static str,
    label: &'static str,
    hint: &'static str,
    icon: NavIcon,
    action: PaletteAction,
}

/// One control-plane call, shown in the Agent Control feed so the person can
/// watch an external agent (or curl) drive the engine.
#[derive(Clone, Debug)]
struct ControlEvent {
    at: String,
    method: String,
    path: String,
    ok: bool,
}

/// The in-app screen-play runner. It really performs the engine steps and
/// pauses at a hit checkpoint when no harness traffic has arrived yet.
struct ScreenRun {
    play: usize,
    step: usize,
    step_entered: Instant,
    waiting: bool,
    /// Hits on the current rule when this step began, so a checkpoint waits for
    /// new traffic rather than pre-seeded sample hits.
    hit_base: u32,
    hit_armed: bool,
    outcomes: Vec<StepOutcome>,
    finished: bool,
}

/// A pending API-test request and the index it belongs to.
struct TestSend {
    index: usize,
    rx: Receiver<SendOutcome>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrafficFilter {
    All,
    Live,
    Pinned,
    Graphql,
    Rest,
    Errors,
    Mocked,
    Tunnels,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Overview,
    Request,
    Response,
    Headers,
    Curl,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResponseFormat {
    Json,
    Raw,
    Xml,
    Html,
}

impl ResponseFormat {
    fn label(self) -> &'static str {
        match self {
            ResponseFormat::Json => "JSON",
            ResponseFormat::Raw => "Raw",
            ResponseFormat::Xml => "XML",
            ResponseFormat::Html => "HTML",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            ResponseFormat::Json => "Pretty-print JSON (default)",
            ResponseFormat::Raw => "Show the exact captured body",
            ResponseFormat::Xml => "Indent XML markup",
            ResponseFormat::Html => "Indent HTML markup",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyView {
    Json,
    Raw,
    Markup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ComposerTab {
    Headers,
    Body,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    CopyUrl,
    CopyCurl,
    CopyBody,
    CopyResponse,
    Share,
    Repeat,
    Pin,
    MockThis,
    CatchThis,
}

struct Toast {
    text: String,
    until: f64,
}

struct BreakpointSession {
    rule_name: String,
    stage: BpStage,
    live_bp_id: Option<u64>,
    method: String,
    url: String,
    headers: String,
    body: String,
    base_status: u16,
    base_resp: String,
    base_latency: u64,
}

struct ComposerResponse {
    status: u16,
    latency_ms: u64,
    size: u64,
    headers: Vec<(String, String)>,
    body: String,
    mocked: Option<String>,
    error: Option<String>,
}

struct MapEditor {
    id: Option<u64>,
    name: String,
    folder: String,
    pattern: String,
    method: RuleMethod,
    status: String,
    content_type: String,
    delay: String,
    body: String,
    local_file: String,
    graphql_name: String,
    enabled: bool,
    active_scenario: MockScenario,
    success_status: String,
    success_body: String,
    error_status: String,
    error_body: String,
    empty_status: String,
    empty_body: String,
    curl_paste: String,
}

impl MapEditor {
    fn sync_current_to_slot(&mut self) {
        match self.active_scenario {
            MockScenario::Success => {
                self.success_status = self.status.clone();
                self.success_body = self.body.clone();
            }
            MockScenario::Error => {
                self.error_status = self.status.clone();
                self.error_body = self.body.clone();
            }
            MockScenario::Empty => {
                self.empty_status = self.status.clone();
                self.empty_body = self.body.clone();
            }
        }
    }

    fn switch_scenario(&mut self, target: MockScenario) {
        self.sync_current_to_slot();
        self.active_scenario = target;
        match target {
            MockScenario::Success => {
                self.status = self.success_status.clone();
                self.body = self.success_body.clone();
            }
            MockScenario::Error => {
                self.status = self.error_status.clone();
                self.body = self.error_body.clone();
            }
            MockScenario::Empty => {
                self.status = self.empty_status.clone();
                self.body = self.empty_body.clone();
            }
        }
    }

    fn load_local_file(&mut self) -> Result<(), String> {
        let path = self.local_file.trim().to_string();
        if path.is_empty() {
            return Err("Paste a file path first".into());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("Could not read {}: {}", path, e))?;
        self.body = text;
        if self.content_type.trim().is_empty() || self.content_type == "application/json" {
            self.content_type = content_type_for_path(&path).to_string();
        }
        self.sync_current_to_slot();
        Ok(())
    }

    fn apply_curl(&mut self) -> bool {
        let Some(parsed) = parse_curl(&self.curl_paste) else {
            return false;
        };
        self.method = rule_method_for(&parsed.method);
        let base = parsed.url.split('?').next().unwrap_or(&parsed.url);
        self.pattern = if parsed.url.contains('?') {
            format!("{}?*", base)
        } else {
            format!("{}*", base)
        };
        if self.name.trim().is_empty() || self.name == "Local fixture" {
            let leaf = base
                .trim_end_matches('/')
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or("endpoint");
            self.name = format!("Mock {}", leaf);
        }
        if let Some(gql) = graphql_info(&parsed.method, &parsed.body) {
            if gql.operation != "Anonymous" {
                self.graphql_name = gql.operation;
            }
        }
        true
    }
}

struct BpEditor {
    id: Option<u64>,
    name: String,
    pattern: String,
    graphql_name: String,
    stage: BpStage,
    enabled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectMode {
    /// Install the certificate, point the device at the proxy and restart it.
    Full,
    /// Install or copy the certificate only.
    CertOnly,
}

/// The confirmation shown after a device is detected and before anything is
/// installed. It keeps the exact row the person clicked, so the dialog can
/// name the device and explain what will change.
#[derive(Clone)]
struct DevicePrompt {
    device: devices::DetectedDevice,
    mode: ConnectMode,
}

enum DeviceScanState {
    Idle,
    Scanning(Receiver<devices::DeviceScanResult>),
    Ready(devices::DeviceScanResult),
}

struct SendOutcome {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    req_body: String,
    status: u16,
    latency_ms: u64,
    size: u64,
    resp_headers: Vec<(String, String)>,
    body: String,
    error: Option<String>,
}

/// A real request is sent through the proxy to a local target; the proxy
/// answers it and records it. Passing means the whole in/out path works, not
/// just that a port is open.
struct CaptureCheck {
    nonce: u64,
    started: Instant,
    phase: CaptureCheckPhase,
    http_status: Option<u16>,
}

#[derive(Clone)]
enum CaptureCheckPhase {
    Running,
    Passed { status: u16, latency_ms: u64 },
    Failed(String),
}

enum CaptureCheckMsg {
    Http(Result<u16, String>),
    Route(Vec<WebProxyState>),
}

/// One scripted practice run. It sends real requests through the listener and
/// reports each one back, so the strip on screen always matches what the gate
/// actually answered.
struct PracticeRun {
    total: usize,
    statuses: Vec<Option<u16>>,
    rx: Receiver<PracticeEvent>,
    done: bool,
    error: Option<String>,
}

enum PracticeEvent {
    Sent { index: usize, status: u16 },
    Failed { index: usize, message: String },
    Done,
}

/// The five calls the practice button sends. They look like a small mobile
/// backend: GraphQL, a catalogue, an unauthorised order, a checkout that hits
/// the breakpoint and a recommendations feed.
const PRACTICE_STEPS: [(&str, &str, &str, &str); 5] = [
    (
        "GraphQL",
        "POST",
        "/demo/graphql",
        r#"{"operationName":"GetProducts","query":"query GetProducts { products(first: 20) { id name price currency inStock } }","variables":{}}"#,
    ),
    ("Mock", "GET", "/demo/products?limit=20&collection=running", ""),
    ("401", "GET", "/demo/orders/8421", ""),
    (
        "Pause",
        "POST",
        "/demo/checkout",
        r#"{"cart":"cart_9f21","totalQuantity":3,"total":2190.00}"#,
    ),
    ("REST", "GET", "/demo/recommendations?userId=u_8841", ""),
];

struct MacRouteSummary {
    text: String,
    detail: String,
    color: egui::Color32,
    points_here: bool,
}

pub struct App {
    captures: Vec<Capture>,
    selected: usize,
    /// When true the newest capture becomes the selection. Clicking a row turns
    /// this off so a live feed keeps filling the list without moving the detail
    /// pane away from the call the person is reading.
    follow_latest: bool,
    view: View,
    filter: TrafficFilter,
    sort_desc: bool,
    inspector_tab: InspectorTab,
    response_format: ResponseFormat,
    search: String,
    /// The all-features palette is the always-available answer to "where is
    /// that screen?" — especially useful when the rail is scrolled or short.
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    palette_focus: bool,
    recording: bool,
    map_rules: Vec<MapRule>,
    breakpoints: Vec<BreakpointRule>,
    ssl_proxying_enabled: bool,
    ssl_allowlist: Vec<String>,
    /// Explicit full-catch switch. Off by default; the connection card shows
    /// clearly whether HTTPS is being decrypted or only tunnelled.
    intercept_all: bool,
    logs: Vec<LogEntry>,
    composer_method: String,
    composer_url: String,
    composer_headers: String,
    composer_body: String,
    composer_curl_input: String,
    composer_tab: ComposerTab,
    composer_response: Option<ComposerResponse>,
    pending_send: Option<Receiver<SendOutcome>>,
    breakpoint_session: Option<BreakpointSession>,
    map_editor: Option<MapEditor>,
    bp_editor: Option<BpEditor>,
    map_enabled: bool,
    catch_enabled: bool,
    map_filter: String,
    map_selected_folder: Option<String>,
    map_folder_collapsed: std::collections::HashSet<String>,
    map_test_url: String,
    quick_mock_url: String,
    quick_mock_method: RuleMethod,
    quick_mock_scenario: MockScenario,
    quick_mock_body: String,
    bp_filter: String,
    bp_test_url: String,
    new_ssl_domain: String,
    mac_proxy_confirm_open: bool,
    /// True while Proxzy itself has pointed the macOS web proxy at this app.
    /// The flag makes quitting safe: closing the window restores the network
    /// instead of leaving the Mac with a proxy that no longer answers.
    mac_proxy_on: bool,
    share_open: bool,
    /// The active guided-tour stop. None means the tour is closed; the Tutorial
    /// page is a normal screen and does not depend on this flag.
    tour: Option<usize>,
    toast: Option<Toast>,
    frames: u32,
    screenshot_path: Option<String>,
    screenshot_frames: u32,
    next_id: u64,
    bp_stage_override: BpStage,
    log_level_filter: Option<Level>,
    proxy: Option<proxy::ProxyHandle>,
    proxy_rx: Option<Receiver<proxy::ProxyEvent>>,
    proxy_error: Option<String>,
    proxy_port: u16,
    lan_enabled: bool,
    control_token: String,
    lan_ip: String,
    ca_info: Option<proxy::CaInfo>,
    live_tab: usize,
    capture_check: Option<CaptureCheck>,
    capture_check_rx: Option<Receiver<CaptureCheckMsg>>,
    practice: Option<PracticeRun>,
    mac_route: Option<Vec<WebProxyState>>,
    sim_reboot: Option<Receiver<Result<String, String>>>,
    device_scan: DeviceScanState,
    device_prompt: Option<DevicePrompt>,
    last_device_scan: Option<Instant>,
    samples_auto_cleared: bool,
    /// A leftover web proxy found at startup: it points at a local port that
    /// no listener answers, which is what a crash or force-quit leaves.
    stale_proxy: Option<Vec<WebProxyState>>,
    stale_proxy_scan: Option<Receiver<Vec<WebProxyState>>>,
    /// The person declined the one-click fix once; keep the quiet banner
    /// instead of nagging with the modal on every frame.
    stale_proxy_dismissed: bool,
    // ---- automation studio -------------------------------------------------
    map_view: MapViewMode,
    tests: Vec<ApiTest>,
    test_selected: usize,
    test_target: TargetPlatform,
    test_queue: Vec<usize>,
    test_send: Option<TestSend>,
    test_export: String,
    test_import: String,
    test_io: bool,
    test_headers: String,
    test_injects: String,
    agent_feed: Vec<ControlEvent>,
    agent_folder: Option<String>,
    agent_rule: Option<u64>,
    agent_inject_key: String,
    agent_inject_value: String,
    agent_session_name: String,
    agent_session_env: String,
    agent_session_version: String,
    agent_port_input: String,
    screen_plays: Vec<ScreenPlay>,
    screen_selected: usize,
    screen_target: TargetPlatform,
    screen_run: Option<ScreenRun>,
    screen_tab: usize,
    screen_export: String,
    screen_import: String,
    android_transport: AndroidTransport,
    // ---- ACP concept bridge -----------------------------------------------
    /// Last revealed message in the simulated ACP session.
    acp_progress: usize,
    acp_selected: usize,
    acp_permission: AcpPermission,
    acp_playing: bool,
    acp_entered: Instant,
    /// 0 = editor config, 1 = MCP entry, 2 = agent prompt.
    acp_tab: usize,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        screenshot_path: Option<String>,
        screenshot_frames: u32,
        start_port: u16,
    ) -> Self {
        configure(&cc.egui_ctx);
        let (method, url, headers, body) = composer_preset_graphql();
        let start_view = match std::env::var("PROXZY_VIEW").unwrap_or_default().as_str() {
            "live" | "capture" | "proxy" => View::Live,
            "composer" => View::Composer,
            "map" | "map-local" => View::MapLocal,
            "map-pipeline" => View::MapLocal,
            "breakpoints" => View::Breakpoints,
            "logs" | "log-activity" | "activity" => View::Logs,
            "agent" | "agent-control" => View::Agent,
            "tests" | "test-bench" => View::Tests,
            "screen" | "screen-play" | "mcp" => View::Screen,
            "acp" | "agent-protocol" | "protocol" => View::Acp,
            "guide" => View::Guide,
            "tutorial" | "learn" | "tour" => View::Tutorial,
            _ => View::Traffic,
        };
        let (proxy_tx, proxy_rx) = channel();
        let mut proxy_handle = None;
        let mut proxy_error = None;
        let mut proxy_port = start_port.max(1);
        let mut lan_enabled = false;
        let mut control_token = String::new();
        let mut lan_ip = String::new();
        for candidate in [proxy_port, 9090, 9091, 9092, 9093] {
            match proxy::start_with_options(
                proxy::ProxyStartOptions {
                    port: candidate,
                    ..proxy::ProxyStartOptions::default()
                },
                proxy_tx.clone(),
            ) {
                Ok(handle) => {
                    proxy_port = handle.port;
                    lan_enabled = handle.binding.policy.is_lan();
                    control_token = handle
                        .binding
                        .policy
                        .control_token()
                        .unwrap_or_default()
                        .to_string();
                    lan_ip = handle
                        .binding
                        .policy
                        .advertised_ip()
                        .unwrap_or("127.0.0.1")
                        .to_string();
                    proxy_handle = Some(handle);
                    break;
                }
                Err(e) => {
                    proxy_error = Some(e);
                }
            }
        }
        let ca_info = proxy_handle.as_ref().map(|h| h.ca.clone());
        let mut app = Self {
            captures: sample_captures(),
            selected: 0,
            follow_latest: true,
            view: start_view,
            filter: TrafficFilter::All,
            sort_desc: true,
            inspector_tab: InspectorTab::Overview,
            response_format: ResponseFormat::Json,
            search: String::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focus: false,
            recording: true,
            map_rules: sample_map_rules(),
            breakpoints: sample_breakpoints(),
            ssl_proxying_enabled: true,
            ssl_allowlist: Vec::new(),
            intercept_all: std::env::var("PROXZY_FULL_CATCH")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            logs: sample_logs(),
            composer_method: method,
            composer_url: url,
            composer_headers: headers,
            composer_body: body,
            composer_curl_input: String::new(),
            composer_tab: ComposerTab::Body,
            composer_response: None,
            pending_send: None,
            breakpoint_session: None,
            map_editor: None,
            bp_editor: None,
            // Sample rules stay visible, but they must not rewrite or pause
            // real simulator traffic until the person turns them on.
            map_enabled: false,
            catch_enabled: false,
            map_filter: String::new(),
            map_selected_folder: None,
            map_folder_collapsed: std::collections::HashSet::new(),
            map_test_url: String::new(),
            quick_mock_url: String::new(),
            quick_mock_method: RuleMethod::Any,
            quick_mock_scenario: MockScenario::Success,
            quick_mock_body: MockScenario::Success.default_body(),
            bp_filter: String::new(),
            bp_test_url: String::new(),
            new_ssl_domain: String::new(),
            mac_proxy_confirm_open: false,
            mac_proxy_on: false,
            share_open: false,
            tour: None,
            toast: None,
            frames: 0,
            screenshot_path,
            screenshot_frames,
            next_id: 500,
            bp_stage_override: BpStage::Request,
            log_level_filter: None,
            proxy: proxy_handle,
            proxy_rx: Some(proxy_rx),
            proxy_error,
            proxy_port,
            lan_enabled,
            control_token,
            lan_ip,
            ca_info,
            live_tab: 0,
            capture_check: None,
            capture_check_rx: None,
            practice: None,
            mac_route: None,
            sim_reboot: None,
            device_scan: DeviceScanState::Idle,
            last_device_scan: None,
            device_prompt: None,
            samples_auto_cleared: false,
            stale_proxy: None,
            stale_proxy_scan: None,
            stale_proxy_dismissed: false,
            map_view: if std::env::var("PROXZY_VIEW").as_deref() == Ok("map-pipeline") {
                MapViewMode::Pipeline
            } else {
                MapViewMode::Table
            },
            tests: studio::sample_tests(),
            test_selected: 0,
            test_target: TargetPlatform::from_slug(
                &std::env::var("PROXZY_TARGET").unwrap_or_default(),
            ),
            test_queue: Vec::new(),
            test_send: None,
            test_export: String::new(),
            test_import: String::new(),
            test_io: false,
            test_headers: String::new(),
            test_injects: String::new(),
            agent_feed: Vec::new(),
            agent_folder: None,
            agent_rule: None,
            agent_inject_key: "USERNAME".into(),
            agent_inject_value: "qa.agent".into(),
            agent_session_name: "TC_WALLET_01".into(),
            agent_session_env: "preprod".into(),
            agent_session_version: "1.1.0".into(),
            agent_port_input: proxy_port.to_string(),
            screen_plays: studio::sample_screen_plays(),
            screen_selected: 0,
            screen_target: TargetPlatform::from_slug(
                &std::env::var("PROXZY_TARGET").unwrap_or_default(),
            ),
            screen_run: None,
            screen_tab: 2,
            screen_export: String::new(),
            screen_import: String::new(),
            android_transport: AndroidTransport::from_slug(
                &std::env::var("PROXZY_ANDROID_TRANSPORT").unwrap_or_default(),
            ),
            acp_progress: ACP_GATE_INDEX,
            acp_selected: ACP_GATE_INDEX,
            acp_permission: AcpPermission::Pending,
            acp_playing: false,
            acp_entered: Instant::now(),
            acp_tab: 0,
        };
        app.agent_rule = app.map_rules.first().map(|r| r.id);
        app.sync_proxy_config();
        app.request_device_scan();
        app.request_stale_proxy_scan();
        if matches!(
            std::env::var("PROXZY_VIEW").unwrap_or_default().as_str(),
            "features" | "palette" | "all-features"
        ) || std::env::var("PROXZY_DEMO").as_deref() == Ok("palette")
        {
            app.palette_open = true;
            app.palette_focus = true;
        }
        match std::env::var("PROXZY_DEMO").unwrap_or_default().as_str() {
            "map-edit" => {
                let rule = app.map_rules.first().cloned();
                app.open_map_editor(rule);
            }
            "bp-edit" => {
                let rule = app.breakpoints.first().cloned();
                app.open_bp_editor(rule);
            }
            "pause" => {
                let rule = app.breakpoints.first().cloned();
                if let Some(rule) = rule {
                    app.pause_with_rule(&rule, &cc.egui_ctx);
                }
            }
            "share" => app.share_open = true,
            "check" | "doctor" => {
                if app.view != View::Live {
                    app.view = View::Live;
                }
                app.start_capture_check(&cc.egui_ctx);
            }
            "practice" => {
                app.start_practice_session(&cc.egui_ctx);
            }
            "live-filter" | "empty-live" => {
                app.view = View::Traffic;
                app.filter = TrafficFilter::Live;
                app.follow_latest = false;
            }
            "stale" => {
                app.stale_proxy_scan = None;
                app.stale_proxy = Some(vec![WebProxyState {
                    service: "Wi-Fi".into(),
                    enabled: true,
                    host: "127.0.0.1".into(),
                    port: 9091,
                }]);
            }
            "response" => {
                app.view = View::Traffic;
                app.inspector_tab = InspectorTab::Response;
            }
            "response-raw" => {
                app.view = View::Traffic;
                app.inspector_tab = InspectorTab::Response;
                app.response_format = ResponseFormat::Raw;
            }
            "pinned" => {
                app.follow_latest = false;
                if app.captures.len() > 1 {
                    app.selected = 1;
                }
            }
            "tunnels" | "protected" => {
                app.view = View::Traffic;
                app.filter = TrafficFilter::Tunnels;
                app.follow_latest = false;
                let protected = std::env::var("PROXZY_DEMO").as_deref() == Ok("protected");
                if let Some(idx) = app
                    .captures
                    .iter()
                    .position(|c| c.is_tunnel() && c.is_system_protected() == protected)
                {
                    app.selected = idx;
                }
            }
            "full-catch" => {
                app.view = View::Live;
                app.intercept_all = true;
                app.ssl_proxying_enabled = true;
                app.sync_proxy_config();
            }
            "tour" | "tutorial" => {
                app.begin_tour(0);
            }
            "tour-agent" => {
                app.begin_tour(tour_index_for(View::Agent));
            }
            "tour-agent-hook" => {
                app.begin_tour(tour_index_for_art(TourArt::AgentHook));
            }
            "tour-acp" => {
                app.begin_tour(tour_index_for(View::Acp));
            }
            "acp-replay" => {
                app.view = View::Acp;
                app.acp_reset(true);
            }
            "acp-full" => {
                app.view = View::Acp;
                app.acp_permission = AcpPermission::AllowOnce;
                let len = acp_transcript(app.proxy_port, app.acp_permission).len();
                app.acp_progress = len.saturating_sub(1);
                app.acp_selected = app.acp_progress;
            }
            "acp-deny" => {
                app.view = View::Acp;
                app.acp_permission = AcpPermission::RejectOnce;
                let len = acp_transcript(app.proxy_port, app.acp_permission).len();
                app.acp_progress = len.saturating_sub(1);
                app.acp_selected = app.acp_progress;
            }
            other if other.starts_with("tour-") => {
                if let Ok(index) = other["tour-".len()..].parse::<usize>() {
                    app.begin_tour(index);
                }
            }
            "composer" => {
                app.view = View::Composer;
                let body = r#"{"data":{"cart":{"id":"cart_9f21","totalQuantity":3,"lines":[{"sku":"TEE-01","qty":2},{"sku":"MUG-04","qty":1}]}}}"#;
                app.composer_response = Some(ComposerResponse {
                    status: 200,
                    latency_ms: 84,
                    size: body.len() as u64,
                    headers: vec![
                        ("content-type".into(), "application/json".into()),
                        ("x-request-id".into(), "req_7d1c9a".into()),
                    ],
                    body: body.into(),
                    mocked: None,
                    error: None,
                });
            }
            _ => {}
        }
        app
    }

    fn sync_proxy_config(&self) {
        if let Some(handle) = &self.proxy {
            let (auto_rec, auto_rec_domains, active_session) = handle
                .config
                .read()
                .map(|c| (c.auto_record, c.auto_record_domains.clone(), c.active_session.clone()))
                .unwrap_or((false, Vec::new(), None));
            handle.update(proxy::ProxyConfig {
                recording: self.recording,
                map_enabled: self.map_enabled,
                map_rules: self.map_rules.clone(),
                catch_enabled: self.catch_enabled,
                breakpoints: self.breakpoints.clone(),
                ssl_proxying_enabled: self.ssl_proxying_enabled,
                intercept_all: self.intercept_all,
                ssl_allowlist: self.ssl_allowlist.clone(),
                auto_record: auto_rec,
                auto_record_domains: auto_rec_domains,
                active_session,
            });
        }
    }

    fn poll_proxy(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(rx) = &self.proxy_rx {
            loop {
                match rx.try_recv() {
                    Ok(ev) => events.push(ev),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }
        for event in events {
            match event {
                proxy::ProxyEvent::Started { port, ca } => {
                    self.proxy_port = port;
                    self.ca_info = Some(ca);
                    self.proxy_error = None;
                }
                proxy::ProxyEvent::Stopped => {
                    self.proxy = None;
                    self.ca_info = None;
                    self.log(Level::Warn, "Proxy", "Live proxy stopped");
                }
                proxy::ProxyEvent::Capture(capture) => {
                    let method = capture.method.clone();
                    let url = capture.url.clone();
                    let status = capture.status;
                    let mock = capture.mock.clone();
                    let aborted = capture.aborted;
                    self.note_capture_check(&capture, ctx);
                    self.add_capture(*capture);
                    if let Some(name) = mock {
                        self.log(
                            Level::Warn,
                            "Map Local",
                            format!("Served \"{}\" for {} {}", name, method, url),
                        );
                    } else if aborted {
                        self.log(
                            Level::Warn,
                            "Breakpoints",
                            format!("Aborted {} {}", method, url),
                        );
                    } else {
                        self.log(
                            Level::Info,
                            "Capture",
                            format!("{} {} · {}", method, url, status),
                        );
                    }
                }
                proxy::ProxyEvent::Log { level, message } => {
                    let lvl = match level.as_str() {
                        "Warn" => Level::Warn,
                        "Error" => Level::Error,
                        _ => Level::Info,
                    };
                    self.log(lvl, "Proxy", message);
                }
                proxy::ProxyEvent::Breakpoint(bp) => {
                    self.open_live_breakpoint(*bp);
                }
                proxy::ProxyEvent::RemoteConfigUpdated(cfg) => {
                    let cfg = *cfg;
                    self.map_enabled = cfg.map_enabled;
                    self.map_rules = cfg.map_rules;
                    self.ssl_proxying_enabled = cfg.ssl_proxying_enabled;
                    self.intercept_all = cfg.intercept_all;
                    self.ssl_allowlist = cfg.ssl_allowlist;
                }
                proxy::ProxyEvent::Control { method, path, ok } => {
                    self.agent_feed.insert(
                        0,
                        ControlEvent {
                            at: now_stamp(),
                            method,
                            path,
                            ok,
                        },
                    );
                    if self.agent_feed.len() > 24 {
                        self.agent_feed.truncate(24);
                    }
                }
                proxy::ProxyEvent::Error(message) => {
                    self.proxy_error = Some(message.clone());
                    self.log(Level::Error, "Proxy", message);
                }
            }
        }
        self.sync_proxy_config();
        let _ = ctx;
    }

    fn start_proxy(&mut self, ctx: &egui::Context) {
        if self.proxy.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let mut last_error = None;
        let requested_lan = self.lan_enabled;
        let requested_token = if self.control_token.trim().is_empty() {
            None
        } else {
            Some(self.control_token.clone())
        };
        for candidate in [self.proxy_port, 9090, 9091, 9092, 9093] {
            match proxy::start_with_options(
                proxy::ProxyStartOptions {
                    port: candidate,
                    lan_enabled: requested_lan,
                    bind_ip: None,
                    control_token: requested_token.clone(),
                },
                tx.clone(),
            ) {
                Ok(handle) => {
                    self.proxy_port = handle.port;
                    self.ca_info = Some(handle.ca.clone());
                    self.lan_enabled = handle.binding.policy.is_lan();
                    self.control_token = handle
                        .binding
                        .policy
                        .control_token()
                        .unwrap_or_default()
                        .to_string();
                    self.lan_ip = handle
                        .binding
                        .policy
                        .advertised_ip()
                        .unwrap_or("127.0.0.1")
                        .to_string();
                    self.proxy = Some(handle);
                    self.proxy_rx = Some(rx);
                    self.proxy_error = None;
                    self.log(
                        Level::Info,
                        "Proxy",
                        if self.lan_enabled {
                            format!(
                                "Live capture started on {} (LAN opt-in)",
                                self.binding_label()
                            )
                        } else {
                            format!("Live capture started on {}", self.binding_label())
                        },
                    );
                    self.toast_now(ctx, "Live proxy started");
                    self.sync_proxy_config();
                    return;
                }
                Err(e) => last_error = Some(e),
            }
        }
        let message = last_error.unwrap_or_else(|| "Proxy could not start".to_string());
        self.proxy_error = Some(message.clone());
        self.log(Level::Error, "Proxy", message);
        self.toast_now(ctx, "Could not start the proxy — is another copy running?");
    }

    fn binding_label(&self) -> String {
        if self.lan_enabled {
            format!("{}:{}", self.lan_ip, self.proxy_port)
        } else {
            format!("127.0.0.1:{}", self.proxy_port)
        }
    }

    /// Restart the listener with or without LAN access. The safe default is
    /// local only; turning LAN on is always an explicit action.
    fn set_lan_enabled(&mut self, ctx: &egui::Context, enabled: bool) {
        if self.lan_enabled == enabled && self.proxy.is_some() {
            return;
        }
        let was_running = self.proxy.is_some();
        self.lan_enabled = enabled;
        if self.proxy.is_some() {
            if let Some(mut handle) = self.proxy.take() {
                handle.stop();
            }
            self.proxy_rx = None;
        }
        if was_running {
            self.start_proxy(ctx);
        }
        if enabled {
            self.log(
                Level::Warn,
                "Security",
                format!(
                    "LAN access enabled on {} — the control plane now requires a token",
                    self.binding_label()
                ),
            );
            self.toast_now(ctx, "LAN access on — keep this network trusted");
        } else {
            self.log(Level::Info, "Security", "LAN access off — proxy is local only again");
            self.toast_now(ctx, "LAN access off");
        }
    }

    /// Send one real request through the listener and wait for it to appear in
    /// the traffic list. This is the honest answer to "I connected the
    /// simulator but see nothing": it separates a dead gate from a device that
    /// is still bypassing it.
    fn start_capture_check(&mut self, ctx: &egui::Context) {
        if self.proxy.is_none() {
            self.start_proxy(ctx);
        }
        if self.proxy.is_none() {
            self.toast_now(ctx, "Start the proxy first");
            return;
        }
        let port = self.proxy_port;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        let (tx, rx) = channel();
        self.capture_check_rx = Some(rx);
        self.capture_check = Some(CaptureCheck {
            nonce,
            started: Instant::now(),
            phase: CaptureCheckPhase::Running,
            http_status: None,
        });
        self.mac_route = None;
        self.log(
            Level::Info,
            "Capture check",
            format!(
                "Sent a real request through 127.0.0.1:{} and waiting for it to be recorded",
                port
            ),
        );
        std::thread::spawn(move || {
            let result = capture_check_request(port, nonce);
            let _ = tx.send(CaptureCheckMsg::Http(result));
            let states = read_all_web_proxies();
            let _ = tx.send(CaptureCheckMsg::Route(states));
        });
        ctx.request_repaint();
    }

    fn poll_capture_check(&mut self, ctx: &egui::Context) {
        let mut messages = Vec::new();
        if let Some(rx) = &self.capture_check_rx {
            loop {
                match rx.try_recv() {
                    Ok(message) => messages.push(message),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }
        for message in messages {
            match message {
                CaptureCheckMsg::Http(Ok(status)) => {
                    if let Some(check) = &mut self.capture_check {
                        check.http_status = Some(status);
                    }
                }
                CaptureCheckMsg::Http(Err(error)) => {
                    if let Some(check) = &mut self.capture_check {
                        if matches!(check.phase, CaptureCheckPhase::Running) {
                            check.phase = CaptureCheckPhase::Failed(error.clone());
                        }
                    }
                    self.log(
                        Level::Error,
                        "Capture check",
                        format!("The gate did not answer: {}", error),
                    );
                }
                CaptureCheckMsg::Route(states) => {
                    self.mac_route = Some(states);
                }
            }
        }

        if let Some(check) = &mut self.capture_check {
            if matches!(check.phase, CaptureCheckPhase::Running)
                && check.started.elapsed() > Duration::from_secs(6)
            {
                let message = if !self.recording {
                    "Recording is paused, so the request was answered but not added to the list."
                        .to_string()
                } else if let Some(status) = check.http_status {
                    format!(
                        "The gate answered HTTP {}, but no traffic row arrived. Check that recording is on and the port is the one macOS is using.",
                        status
                    )
                } else {
                    "No answer from the gate. Is another app using this port?".to_string()
                };
                check.phase = CaptureCheckPhase::Failed(message.clone());
                self.log(Level::Error, "Capture check", message);
                ctx.request_repaint();
            }
        }
    }

    /// Mark the check as passed when the matching capture arrives. The nonce
    /// makes sure an old or unrelated request cannot turn the check green.
    fn note_capture_check(&mut self, capture: &Capture, ctx: &egui::Context) {
        let Some(check) = &mut self.capture_check else {
            return;
        };
        if !matches!(check.phase, CaptureCheckPhase::Running) {
            return;
        }
        if capture.url.contains(&format!("nonce={}", check.nonce)) {
            check.phase = CaptureCheckPhase::Passed {
                status: capture.status,
                latency_ms: capture.latency_ms,
            };
            self.log(
                Level::Info,
                "Capture check",
                format!(
                    "Passed: {} {} answered with {} in {} ms and was recorded",
                    capture.method, capture.url, capture.status, capture.latency_ms
                ),
            );
            self.toast_now(ctx, "Capture check passed — live traffic is arriving");
            ctx.request_repaint();
        }
    }

    /// Create (or refresh) the two rules the practice session uses. They are
    /// ordinary rules, visible and editable on the Map Local and Breakpoints
    /// pages, so the person can change them afterwards and see the effect.
    fn ensure_practice_rules(&mut self) {
        const MOCK_NAME: &str = "Practice · Products mock";
        const BP_NAME: &str = "Practice · Pause checkout";
        let success = r#"{
  "source": "practice-mock",
  "note": "Answered by Map Local — the server was never called.",
  "products": [
    { "id": "prod_mock_1", "name": "Mocked product", "price": 99.00, "currency": "THB" }
  ]
}"#;
        if !self.map_rules.iter().any(|r| r.name == MOCK_NAME) {
            let id = self.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
            self.map_rules.push(MapRule {
                id,
                name: MOCK_NAME.into(),
                folder: "Practice".into(),
                pattern: "*/demo/products*".into(),
                method: RuleMethod::Get,
                status: 200,
                content_type: "application/json".into(),
                delay_ms: 6,
                body: success.into(),
                local_file: String::new(),
                graphql_name: String::new(),
                enabled: true,
                hits: 0,
                active_scenario: MockScenario::Success,
                success_status: 200,
                success_body: success.into(),
                error_status: 500,
                error_body: r#"{"error":"practice_error","message":"This is the Error scenario from Map Local."}"#.into(),
                empty_status: 200,
                empty_body: r#"{"source":"practice-mock","products":[]}"#.into(),
                inject_props: std::collections::HashMap::new(),
                app_version: String::new(),
                env: String::new(),
            });
        } else if let Some(rule) = self.map_rules.iter_mut().find(|r| r.name == MOCK_NAME) {
            rule.enabled = true;
            rule.status = 200;
            rule.active_scenario = MockScenario::Success;
            rule.body = success.into();
            rule.success_status = 200;
            rule.success_body = success.into();
        }
        if !self.breakpoints.iter().any(|b| b.name == BP_NAME) {
            let id = self.breakpoints.iter().map(|b| b.id).max().unwrap_or(0) + 1;
            self.breakpoints.push(BreakpointRule {
                id,
                name: BP_NAME.into(),
                pattern: "*/demo/checkout*".into(),
                graphql_name: String::new(),
                stage: BpStage::Request,
                enabled: true,
                hits: 0,
            });
        } else if let Some(rule) = self.breakpoints.iter_mut().find(|b| b.name == BP_NAME) {
            rule.enabled = true;
        }
        self.map_enabled = true;
        self.catch_enabled = true;
    }

    /// Start the no-device practice run: five real calls through the listener,
    /// one of them answered by Map Local and one paused by a breakpoint.
    fn start_practice_session(&mut self, ctx: &egui::Context) {
        if self.proxy.is_none() {
            self.start_proxy(ctx);
        }
        if self.proxy.is_none() {
            self.toast_now(ctx, "Start the proxy before the practice session");
            return;
        }
        self.ensure_practice_rules();
        // Each run starts clean: old practice rows go, real captures stay.
        self.captures
            .retain(|c| !c.url.contains(crate::proxy::PRACTICE_HOST));
        self.selected = 0;
        self.follow_latest = true;
        self.filter = TrafficFilter::All;
        self.search.clear();
        self.view = View::Traffic;
        self.inspector_tab = InspectorTab::Overview;

        let (tx, rx) = channel();
        let port = self.proxy_port;
        self.practice = Some(PracticeRun {
            total: PRACTICE_STEPS.len(),
            statuses: vec![None; PRACTICE_STEPS.len()],
            rx,
            done: false,
            error: None,
        });
        self.log(
            Level::Info,
            "Practice",
            "Practice session started: five real calls through the gate, all local",
        );
        self.toast_now(
            ctx,
            "Practice session running — watch the rows arrive",
        );
        std::thread::spawn(move || {
            for (index, (_, method, path, body)) in PRACTICE_STEPS.iter().enumerate() {
                if index > 0 {
                    std::thread::sleep(Duration::from_millis(340));
                }
                match practice_request(port, method, path, body) {
                    Ok(status) => {
                        let _ = tx.send(PracticeEvent::Sent { index, status });
                    }
                    Err(message) => {
                        let _ = tx.send(PracticeEvent::Failed { index, message });
                    }
                }
            }
            let _ = tx.send(PracticeEvent::Done);
        });
        ctx.request_repaint();
    }

    fn poll_practice(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(run) = &self.practice {
            loop {
                match run.rx.try_recv() {
                    Ok(event) => events.push(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if !run.done {
                            events.push(PracticeEvent::Done);
                        }
                        break;
                    }
                }
            }
        }
        let mut finished_now = false;
        for event in events {
            let Some(run) = &mut self.practice else {
                break;
            };
            match event {
                PracticeEvent::Sent { index, status } => {
                    if let Some(slot) = run.statuses.get_mut(index) {
                        *slot = Some(status);
                    }
                }
                PracticeEvent::Failed { index, message } => {
                    if let Some(slot) = run.statuses.get_mut(index) {
                        *slot = Some(0);
                    }
                    run.error = Some(message.clone());
                    self.log(
                        Level::Error,
                        "Practice",
                        format!("Practice call {} failed: {}", index + 1, message),
                    );
                }
                PracticeEvent::Done => {
                    if !run.done {
                        run.done = true;
                        finished_now = true;
                    }
                }
            }
        }
        if finished_now {
            let answered = self
                .practice
                .as_ref()
                .map(|r| r.statuses.iter().filter(|s| s.is_some()).count())
                .unwrap_or(0);
            self.log(
                Level::Info,
                "Practice",
                format!(
                    "Practice session finished: {} of {} calls answered. One was mocked and one was paused.",
                    answered,
                    PRACTICE_STEPS.len()
                ),
            );
            self.toast_now(
                ctx,
                "Practice complete — check the Mocked filter and Log Dashboard",
            );
            ctx.request_repaint();
        }
    }

    fn set_full_catch(&mut self, ctx: &egui::Context, enabled: bool) {        self.intercept_all = enabled;
        if enabled {
            self.ssl_proxying_enabled = true;
            self.log(
                Level::Warn,
                "SSL Proxying",
                "Full HTTPS catch is on. Every host is decrypted except protected system services; trust the Proxzy CA on the device first.",
            );
            self.toast_now(ctx, "Full HTTPS catch on — install the CA on the device");
        } else {
            // A lingering "*" entry from an older session would keep full
            // catch effectively on, so clear it with the explicit switch.
            self.ssl_allowlist.retain(|domain| domain != "*");
            self.log(
                Level::Info,
                "SSL Proxying",
                "Full HTTPS catch is off. HTTPS is tunneled unless a domain is listed.",
            );
            self.toast_now(ctx, "Full HTTPS catch off");
        }
        self.sync_proxy_config();
    }

    /// What macOS currently uses for the web proxy. The iOS Simulator inherits
    /// this route, so it is the difference between "the gate is broken" and
    /// "the simulator is still going direct".
    fn mac_route_summary(&self) -> MacRouteSummary {
        let Some(states) = &self.mac_route else {
            return MacRouteSummary {
                text: "not checked".into(),
                detail: "Run the check to read the Mac's proxy route".into(),
                color: FAINT,
                points_here: false,
            };
        };
        let enabled: Vec<&WebProxyState> = states.iter().filter(|s| s.enabled).collect();
        if enabled.is_empty() {
            return MacRouteSummary {
                text: "Direct (off)".into(),
                detail: "The simulator will bypass Proxzy until the Mac proxy is on".into(),
                color: AMBER,
                points_here: false,
            };
        }
        let here: Vec<&&WebProxyState> = enabled
            .iter()
            .filter(|s| s.host == "127.0.0.1" && s.port == self.proxy_port)
            .collect();
        if !here.is_empty() {
            let services = here
                .iter()
                .map(|s| s.service.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return MacRouteSummary {
                text: "Points here".into(),
                detail: format!("{} → 127.0.0.1:{}", services, self.proxy_port),
                color: GREEN,
                points_here: true,
            };
        }
        let first = enabled[0];
        let same_machine = first.host == "127.0.0.1" || first.host == "localhost";
        MacRouteSummary {
            text: if same_machine {
                "Other port".into()
            } else {
                "Other proxy".into()
            },
            detail: if same_machine {
                format!(
                    "{} → {}:{} . This window is on {}. Traffic is going to another proxy.",
                    first.service, first.host, first.port, self.proxy_port
                )
            } else {
                format!("{} → {}:{}", first.service, first.host, first.port)
            },
            color: RED,
            points_here: false,
        }
    }

    fn capture_check_phase(&self) -> Option<CaptureCheckPhase> {
        self.capture_check.as_ref().map(|c| c.phase.clone())
    }

    /// A command the person can paste into Terminal if macOS refuses the
    /// in-app network change. It names the first network service Proxzy can
    /// see and uses this window's real port.
    fn mac_proxy_fix_command(&self) -> String {
        let service = self
            .mac_route
            .as_ref()
            .and_then(|states| states.first())
            .map(|s| s.service.clone())
            .unwrap_or_else(|| "Wi-Fi".to_string());
        format!(
            "sudo networksetup -setwebproxy \"{service}\" 127.0.0.1 {port}\nsudo networksetup -setsecurewebproxy \"{service}\" 127.0.0.1 {port}",
            service = service,
            port = self.proxy_port
        )
    }

    fn install_ca_ios_sim(&mut self, ctx: &egui::Context) -> bool {
        match booted_simulator_udid() {
            Some(udid) => self.install_ca_ios_sim_udid(ctx, &udid),
            None => {
                self.toast_now(ctx, "Open a simulator in Xcode first");
                false
            }
        }
    }

    /// Trust the local CA in one specific simulator. Using the UDID instead of
    /// `booted` matters when more than one simulator is running: the person
    /// clicked one row, so only that device should change.
    fn install_ca_ios_sim_udid(&mut self, ctx: &egui::Context, udid: &str) -> bool {
        let Some(ca) = self.ca_info.clone() else {
            self.toast_now(ctx, "No certificate yet");
            return false;
        };
        let output = std::process::Command::new("xcrun")
            .args(["simctl", "keychain", udid, "add-root-cert"])
            .arg(&ca.pem_path)
            .output();
        match output {
            Ok(out) if out.status.success() => {
                self.log(
                    Level::Info,
                    "Certificate",
                    format!("Installed the Proxzy CA in iOS Simulator {}", udid),
                );
                self.toast_now(ctx, "Certificate installed in the simulator");
                true
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.log(
                    Level::Error,
                    "Certificate",
                    format!("Simulator install failed: {}", if err.is_empty() { "xcrun returned an error" } else { &err }),
                );
                self.toast_now(ctx, "Simulator install failed — open Xcode and boot a simulator first");
                false
            }
            Err(e) => {
                self.log(
                    Level::Error,
                    "Certificate",
                    format!("Could not run xcrun: {}", e),
                );
                self.toast_now(ctx, "Xcode command line tools are needed for the simulator install");
                false
            }
        }
    }

    /// Turn the macOS web proxy on (or off) for every enabled network
    /// service. The iOS Simulator inherits these settings from the Mac, which
    /// lets a local MITM proxy capture simulator traffic without touching the app.
    fn set_mac_proxy(&mut self, ctx: &egui::Context, clear: bool) -> bool {
        if !clear && self.proxy.is_none() {
            self.start_proxy(ctx);
            if self.proxy.is_none() {
                self.toast_now(ctx, "Start the proxy before changing the Mac proxy");
                return false;
            }
        }
        let services = enabled_network_services();
        if services.is_empty() {
            self.log(
                Level::Error,
                "Proxy",
                "No enabled network services were found; set the proxy by hand",
            );
            self.toast_now(ctx, "Could not find a network service");
            return false;
        }
        let port = self.proxy_port.to_string();
        let mut changed = 0usize;
        let mut failures = Vec::new();
        for service in &services {
            let mut ok = true;
            if clear {
                ok &= run_networksetup(&["-setwebproxystate", service, "off"]);
                ok &= run_networksetup(&["-setsecurewebproxystate", service, "off"]);
            } else {
                ok &= run_networksetup(&["-setwebproxy", service, "127.0.0.1", &port]);
                ok &= run_networksetup(&["-setsecurewebproxy", service, "127.0.0.1", &port]);
            }
            if ok {
                changed += 1;
            } else {
                failures.push(service.clone());
            }
        }
        if changed > 0 {
            // Remember that this app changed the Mac. Quitting then restores
            // the network automatically (see the on_exit handler).
            self.mac_proxy_on = !clear;
            self.log(
                Level::Info,
                "Proxy",
                if clear {
                    format!("Turned the macOS web proxy off on {}", changed)
                } else {
                    format!("Pointed the macOS web proxy at 127.0.0.1:{} on {}", self.proxy_port, changed)
                },
            );
        }
        if !failures.is_empty() {
            self.log(
                Level::Error,
                "Proxy",
                format!(
                    "macOS refused the proxy change for {}; run networksetup manually or allow it in System Settings",
                    failures.join(", ")
                ),
            );
            self.toast_now(ctx, "Some network services could not be changed");
            return false;
        }
        self.toast_now(
            ctx,
            if clear {
                "macOS web proxy turned off"
            } else {
                "macOS web proxy now points at Proxzy"
            },
        );
        if !clear {
            // Prove the new route immediately: one real request through the
            // gate, plus a fresh read of the Mac proxy the simulator uses.
            self.start_capture_check(ctx);
        }
        true
    }

    /// Put the macOS web proxy back to Direct before the window disappears.
    /// Only touches the network when this app was the one that changed it, so
    /// a person's own proxy settings are never overwritten by a plain quit.
    fn restore_mac_proxy_for_exit(&mut self) -> bool {
        if !self.mac_proxy_on {
            return false;
        }
        let services = enabled_network_services();
        let mut all_ok = !services.is_empty();
        for service in &services {
            let mut ok = true;
            ok &= run_networksetup(&["-setwebproxystate", service, "off"]);
            ok &= run_networksetup(&["-setsecurewebproxystate", service, "off"]);
            all_ok &= ok;
        }
        if all_ok {
            self.mac_proxy_on = false;
        }
        all_ok
    }

    /// One click for the common case: trust the local CA inside the booted
    /// simulator and point the Mac's proxy at this app.
    fn setup_ios_simulator(&mut self, ctx: &egui::Context) {
        match booted_simulator_udid() {
            Some(udid) => self.setup_ios_simulator_udid(ctx, &udid),
            None => self.toast_now(ctx, "Open a simulator in Xcode first"),
        }
    }

    fn setup_ios_simulator_udid(&mut self, ctx: &egui::Context, udid: &str) {
        if self.ca_info.is_none() {
            match proxy::ensure_ca() {
                Ok(ca) => self.ca_info = Some(ca),
                Err(e) => {
                    self.log(Level::Error, "Certificate", e);
                    self.toast_now(ctx, "Could not create the local certificate");
                    return;
                }
            }
        }
        let cert_ok = self.install_ca_ios_sim_udid(ctx, udid);
        if !cert_ok {
            return;
        }
        let proxy_ok = self.set_mac_proxy(ctx, false);
        // A running simulator keeps the old trust store in memory, so restart
        // it after installing the CA to pick up the new root certificate.
        self.restart_ios_simulator_udid(ctx, udid);
        if proxy_ok {
            self.log(
                Level::Info,
                "Proxy",
                "iOS Simulator setup finished; it is restarting to load the CA",
            );
        } else {
            let service = enabled_network_services()
                .into_iter()
                .next()
                .unwrap_or_else(|| "Wi-Fi".into());
            self.log(
                Level::Error,
                "Proxy",
                format!(
                    "macOS refused the proxy change. Run: sudo networksetup -setwebproxy \"{}\" 127.0.0.1 {} && sudo networksetup -setsecurewebproxy \"{}\" 127.0.0.1 {}",
                    service, self.proxy_port, service, self.proxy_port
                ),
            );
            self.toast_now(
                ctx,
                "Certificate installed, but macOS refused the proxy. Use Copy fix in the check strip.",
            );
        }
    }

    fn restart_ios_simulator(&mut self, ctx: &egui::Context) {
        match booted_simulator_udid() {
            Some(udid) => self.restart_ios_simulator_udid(ctx, &udid),
            None => {
                self.log(
                    Level::Warn,
                    "Certificate",
                    "No booted simulator found; open one in Xcode, then install the certificate",
                );
                self.toast_now(ctx, "Open a simulator in Xcode first");
            }
        }
    }

    fn restart_ios_simulator_udid(&mut self, ctx: &egui::Context, udid: &str) {
        let udid = udid.to_string();
        let (tx, rx) = channel();
        self.sim_reboot = Some(rx);
        std::thread::spawn(move || {
            let result = reboot_simulator(&udid);
            let _ = tx.send(result);
        });
        self.log(
            Level::Info,
            "Certificate",
            "Restarting the booted iOS Simulator so it trusts the new certificate",
        );
        self.toast_now(ctx, "Restarting the simulator…");
    }

    fn poll_sim_reboot(&mut self, ctx: &egui::Context) {
        let mut result = None;
        if let Some(rx) = &self.sim_reboot {
            if let Ok(value) = rx.try_recv() {
                result = Some(value);
            }
        }
        if let Some(value) = result {
            self.sim_reboot = None;
            match value {
                Ok(message) => {
                    self.log(Level::Info, "Certificate", message);
                    self.toast_now(ctx, "Simulator is ready — run the app now");
                }
                Err(message) => {
                    self.log(Level::Error, "Certificate", message);
                    self.toast_now(ctx, "Could not restart the simulator");
                }
            }
        }
    }

    /// Refresh the device list in the background so `xcrun` and `adb` never
    /// stall the window while the person is looking at the Live Capture page.
    fn request_device_scan(&mut self) {
        if matches!(&self.device_scan, DeviceScanState::Scanning(_)) {
            return;
        }
        let (tx, rx) = channel();
        self.device_scan = DeviceScanState::Scanning(rx);
        std::thread::spawn(move || {
            let _ = tx.send(devices::detect_all());
        });
    }

    /// Look once, right after launch, for a web proxy a previous run (or a
    /// crash) left pointing at a dead local port. Reading `networksetup`
    /// is safe; nothing is changed until the person confirms.
    fn request_stale_proxy_scan(&mut self) {
        if self.stale_proxy_scan.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.stale_proxy_scan = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(find_local_web_proxies());
        });
    }

    fn poll_stale_proxy_scan(&mut self) {
        let mut result = None;
        if let Some(rx) = &self.stale_proxy_scan {
            match rx.try_recv() {
                Ok(value) => result = Some(value),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => result = Some(Vec::new()),
            }
        }
        if let Some(value) = result {
            self.stale_proxy_scan = None;
            let mut stale = Vec::new();
            let mut adopted = false;
            for state in value {
                if state.port == self.proxy_port {
                    // The Mac is already routed into this very session (for
                    // example after a relaunch). Adopt the setting so quitting
                    // restores the network instead of breaking the browser.
                    adopted = true;
                } else if proxy_is_stale(&state) {
                    stale.push(state);
                }
            }
            if adopted {
                self.mac_proxy_on = true;
                self.log(
                    Level::Info,
                    "Proxy",
                    format!(
                        "The macOS web proxy already points at Proxzy on 127.0.0.1:{}; it will be restored when you quit",
                        self.proxy_port
                    ),
                );
            }
            if stale.is_empty() {
                self.stale_proxy = None;
            } else {
                let ports: Vec<String> = stale
                    .iter()
                    .map(|s| format!("{}:{}", s.host, s.port))
                    .collect();
                self.log(
                    Level::Warn,
                    "Proxy",
                    format!(
                        "Found a leftover macOS web proxy pointing at {} with nothing listening. Browsers cannot reach the internet until it is cleared.",
                        ports.join(", ")
                    ),
                );
                self.stale_proxy = Some(stale);
            }
        }
    }

    /// Turn off the leftover web proxy on the services it was found on.
    /// Runs only after the person presses the button in the banner.
    fn clear_stale_proxy(&mut self, ctx: &egui::Context) {
        let Some(states) = self.stale_proxy.clone() else {
            return;
        };
        let mut cleared = 0usize;
        let mut failed = Vec::new();
        for state in &states {
            let ok = run_networksetup(&["-setwebproxystate", &state.service, "off"])
                & run_networksetup(&["-setsecurewebproxystate", &state.service, "off"]);
            if ok {
                cleared += 1;
            } else {
                failed.push(state.service.clone());
            }
        }
        if cleared > 0 {
            self.log(
                Level::Info,
                "Proxy",
                format!("Cleared the leftover macOS web proxy on {cleared} service(s)"),
            );
        }
        if !failed.is_empty() {
            self.log(
                Level::Error,
                "Proxy",
                format!(
                    "macOS refused to clear the proxy for {}. Run: networksetup -setwebproxystate \"{}\" off",
                    failed.join(", "),
                    failed[0]
                ),
            );
            self.toast_now(ctx, "Could not clear the leftover proxy; see Logs");
            return;
        }
        self.stale_proxy = None;
        self.stale_proxy_dismissed = false;
        self.mac_proxy_on = false;
        self.toast_now(ctx, "Leftover proxy cleared — the internet is back");
    }

    fn poll_device_scan(&mut self) {
        let mut result = None;
        if let DeviceScanState::Scanning(rx) = &self.device_scan {
            match rx.try_recv() {
                Ok(value) => result = Some(value),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    result = Some(devices::DeviceScanResult::default())
                }
            }
        }
        if let Some(value) = result {
            if std::env::var("PROXZY_DEMO").as_deref() == Ok("connect")
                && self.device_prompt.is_none()
            {
                if let Some(device) = value.devices.iter().find(|d| d.ready).cloned() {
                    self.device_prompt = Some(DevicePrompt {
                        device,
                        mode: ConnectMode::Full,
                    });
                }
            }
            self.device_scan = DeviceScanState::Ready(value);
            self.last_device_scan = Some(Instant::now());
        }
    }

    fn device_result(&self) -> Option<&devices::DeviceScanResult> {
        match &self.device_scan {
            DeviceScanState::Ready(result) => Some(result),
            _ => None,
        }
    }

    fn device_scanning(&self) -> bool {
        matches!(&self.device_scan, DeviceScanState::Scanning(_))
    }

    fn maybe_scan_devices(&mut self) {
        if self.view != View::Live || self.device_scanning() {
            return;
        }
        let due = self
            .last_device_scan
            .map(|at| at.elapsed() >= Duration::from_secs(4))
            .unwrap_or(true);
        if due {
            self.request_device_scan();
        }
    }

    fn ios_device_ready(&self) -> bool {
        self.device_result()
            .map(|result| {
                result
                    .devices
                    .iter()
                    .any(|d| d.kind == devices::DeviceKind::IosSimulator && d.ready)
            })
            .unwrap_or(false)
    }

    fn android_device_ready(&self) -> bool {
        self.device_result()
            .map(|result| {
                result
                    .devices
                    .iter()
                    .any(|d| d.kind.is_android() && d.ready)
            })
            .unwrap_or(false)
    }

    /// Run the confirmed action from the "connect this device" dialog.
    fn connect_detected_device(&mut self, prompt: DevicePrompt, ctx: &egui::Context) {
        let serial = prompt.device.id.as_str();
        match prompt.device.kind {
            devices::DeviceKind::IosSimulator => {
                if prompt.mode == ConnectMode::Full {
                    self.setup_ios_simulator_udid(ctx, serial);
                } else {
                    self.install_ca_ios_sim_udid(ctx, serial);
                }
            }
            kind => {
                if prompt.mode == ConnectMode::Full {
                    if self.proxy.is_none() {
                        self.start_proxy(ctx);
                    }
                    if kind == devices::DeviceKind::AndroidEmulator {
                        let _ = self.setup_android_reverse_serial(ctx, serial);
                        self.set_android_proxy_serial(ctx, Some(serial), "127.0.0.1", false);
                    } else if self.android_transport == AndroidTransport::UsbReverse {
                        if self.setup_android_reverse_serial(ctx, serial) {
                            self.set_android_proxy_serial(ctx, Some(serial), "127.0.0.1", false);
                        }
                    } else {
                        if !self.lan_enabled {
                            self.set_lan_enabled(ctx, true);
                        }
                        if let Some(ip) = devices::host_ip() {
                            self.lan_ip = ip.clone();
                            self.set_android_proxy_serial(ctx, Some(serial), &ip, false);
                        } else {
                            self.toast_now(
                                ctx,
                                "Could not find this Mac's LAN address; set the proxy by hand",
                            );
                        }
                    }
                }
                self.push_ca_android_serial(ctx, Some(serial));
                self.log(
                    Level::Info,
                    "Certificate",
                    "Certificate copied to Download; finish installing it in Settings",
                );
            }
        }
    }

    fn setup_android_reverse_serial(&mut self, ctx: &egui::Context, serial: &str) -> bool {
        let port = self.proxy_port;
        let spec = format!("tcp:{}", port);
        match std::process::Command::new("adb")
            .args(["-s", serial, "reverse", &spec, &spec])
            .output()
        {
            Ok(out) if out.status.success() => {
                self.log(
                    Level::Info,
                    "Proxy",
                    format!("USB bridge ready: adb -s {} reverse {} {}", serial, spec, spec),
                );
                true
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.log(
                    Level::Error,
                    "Proxy",
                    format!("adb reverse failed: {}", err),
                );
                self.toast_now(ctx, "adb reverse failed — check USB debugging connection");
                false
            }
            Err(e) => {
                self.log(Level::Error, "Proxy", format!("Could not run adb: {}", e));
                self.toast_now(ctx, "Android platform tools (adb) were not found");
                false
            }
        }
    }

    fn push_ca_android(&mut self, ctx: &egui::Context) {
        self.push_ca_android_serial(ctx, None);
    }

    fn push_ca_android_serial(&mut self, ctx: &egui::Context, serial: Option<&str>) {
        let Some(ca) = self.ca_info.clone() else {
            self.toast_now(ctx, "No certificate yet");
            return;
        };
        let mut command = std::process::Command::new("adb");
        if let Some(serial) = serial {
            command.args(["-s", serial]);
        }
        let output = command
            .args(["push"])
            .arg(&ca.cer_path)
            .arg("/sdcard/Download/proxzy-ca.cer")
            .output();
        match output {
            Ok(out) if out.status.success() => {
                self.log(
                    Level::Info,
                    "Certificate",
                    "Copied the certificate to /sdcard/Download on the Android device",
                );
                self.toast_now(ctx, "Certificate copied to the Android device");
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.log(
                    Level::Error,
                    "Certificate",
                    format!("adb push failed: {}", if err.is_empty() { "is a device connected?" } else { &err }),
                );
                self.toast_now(ctx, "adb push failed — connect an emulator or phone first");
            }
            Err(e) => {
                self.log(Level::Error, "Certificate", format!("Could not run adb: {}", e));
                self.toast_now(ctx, "Android platform tools (adb) were not found");
            }
        }
    }

    fn set_android_proxy(&mut self, ctx: &egui::Context, clear: bool) {
        self.set_android_proxy_host(ctx, "10.0.2.2", clear);
    }

    fn set_android_proxy_host(&mut self, ctx: &egui::Context, host: &str, clear: bool) {
        self.set_android_proxy_serial(ctx, None, host, clear);
    }

    fn set_android_proxy_serial(
        &mut self,
        ctx: &egui::Context,
        serial: Option<&str>,
        host: &str,
        clear: bool,
    ) {
        let value = if clear {
            ":0".to_string()
        } else {
            format!("{}:{}", host, self.proxy_port)
        };
        let mut command = std::process::Command::new("adb");
        if let Some(serial) = serial {
            command.args(["-s", serial]);
        }
        let output = command
            .args(["shell", "settings", "put", "global", "http_proxy", &value])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                self.log(
                    Level::Info,
                    "Proxy",
                    if clear {
                        "Cleared the Android device proxy".to_string()
                    } else {
                        format!("Set the Android device proxy to {}", value)
                    },
                );
                self.toast_now(
                    ctx,
                    if clear {
                        "Android proxy cleared"
                    } else {
                        "Android device now points at the proxy"
                    },
                );
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.log(Level::Error, "Proxy", format!("adb settings failed: {}", err));
                self.toast_now(ctx, "Could not change the Android proxy");
            }
            Err(e) => {
                self.log(Level::Error, "Proxy", format!("Could not run adb: {}", e));
                self.toast_now(ctx, "Android platform tools (adb) were not found");
            }
        }
    }

    fn open_ca_folder(&mut self, ctx: &egui::Context) {
        let Some(ca) = self.ca_info.clone() else {
            self.toast_now(ctx, "No certificate yet");
            return;
        };
        let _ = std::process::Command::new("open").arg(&ca.dir).spawn();
        self.toast_now(ctx, "Certificate folder opened");
    }

    fn stop_proxy(&mut self, ctx: &egui::Context) {
        if let Some(mut handle) = self.proxy.take() {
            handle.stop();
        }
        self.proxy_rx = None;
        self.ca_info = None;
        self.log(Level::Warn, "Proxy", "Live capture stopped");
        self.toast_now(ctx, "Live proxy stopped");
    }

    fn log(&mut self, level: Level, source: &str, message: impl Into<String>) {
        self.logs.insert(
            0,
            LogEntry {
                time: now_stamp(),
                level,
                source: source.to_string(),
                message: message.into(),
            },
        );
        if self.logs.len() > 200 {
            self.logs.truncate(200);
        }
    }

    fn copy(&mut self, ctx: &egui::Context, text: String, message: &str) {
        ctx.copy_text(text);
        let until = ctx.input(|i| i.time) + 2.4;
        self.toast = Some(Toast {
            text: message.to_string(),
            until,
        });
    }

    fn toast_now(&mut self, ctx: &egui::Context, message: &str) {
        let until = ctx.input(|i| i.time) + 2.4;
        self.toast = Some(Toast {
            text: message.to_string(),
            until,
        });
    }

    fn set_composer_preset(&mut self, graphql: bool) {
        let (m, u, h, b) = if graphql {
            composer_preset_graphql()
        } else {
            composer_preset_rest()
        };
        self.composer_method = m;
        self.composer_url = u;
        self.composer_headers = h;
        self.composer_tab = if b.is_empty() {
            ComposerTab::Headers
        } else {
            ComposerTab::Body
        };
        self.composer_body = b;
        self.composer_response = None;
    }

    fn session_mix_label(&self) -> String {
        let live = self.captures.iter().filter(|c| c.is_live()).count();
        let samples = self.captures.len().saturating_sub(live);
        match (live, samples) {
            (0, 0) => "empty session".to_string(),
            (_, 0) => "live capture".to_string(),
            (0, _) => "sample session".to_string(),
            (live, samples) => format!("{} live · {} sample", live, samples),
        }
    }

    fn count_of(&self, filter: TrafficFilter) -> usize {
        self.captures
            .iter()
            .filter(|c| match filter {
                TrafficFilter::All => true,
                TrafficFilter::Live => c.is_live(),
                TrafficFilter::Pinned => c.pinned,
                TrafficFilter::Graphql => c.kind == ApiKind::Graphql,
                TrafficFilter::Rest => c.kind == ApiKind::Rest,
                TrafficFilter::Errors => c.is_error(),
                TrafficFilter::Mocked => c.mock.is_some(),
                TrafficFilter::Tunnels => c.is_tunnel(),
            })
            .count()
    }

    fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search.trim().to_lowercase();
        let mut out: Vec<usize> = self
            .captures
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let ok_filter = match self.filter {
                    TrafficFilter::All => true,
                    TrafficFilter::Live => c.is_live(),
                    TrafficFilter::Pinned => c.pinned,
                    TrafficFilter::Graphql => c.kind == ApiKind::Graphql,
                    TrafficFilter::Rest => c.kind == ApiKind::Rest,
                    TrafficFilter::Errors => c.is_error(),
                    TrafficFilter::Mocked => c.mock.is_some(),
                    TrafficFilter::Tunnels => c.is_tunnel(),
                };
                let ok_search = q.is_empty()
                    || c.url.to_lowercase().contains(&q)
                    || c.method.to_lowercase().contains(&q)
                    || c.status.to_string().contains(&q)
                    || c.mock
                        .as_ref()
                        .map(|m| m.to_lowercase().contains(&q))
                        .unwrap_or(false);
                ok_filter && ok_search
            })
            .map(|(i, _)| i)
            .collect();
        if !self.sort_desc {
            out.reverse();
        }
        out
    }

    fn traffic_title(&self) -> &'static str {
        match self.filter {
            TrafficFilter::All => "All Traffic",
            TrafficFilter::Live => "Live capture",
            TrafficFilter::Pinned => "Pinned requests",
            TrafficFilter::Graphql => "GraphQL",
            TrafficFilter::Rest => "REST API",
            TrafficFilter::Errors => "Errors",
            TrafficFilter::Mocked => "Mocked responses",
            TrafficFilter::Tunnels => "Raw tunnels",
        }
    }

    fn add_capture(&mut self, mut c: Capture) {
        c.id = self.next_id;
        self.next_id += 1;
        let live = c.is_live();
        // Remember which row the person is reading before the list shifts.
        let keep_id = if self.follow_latest {
            None
        } else {
            self.captures.get(self.selected).map(|row| row.id)
        };
        self.captures.insert(0, c);
        // The seeded rows are there to make the empty app legible. Once real
        // traffic arrives, keep the list honest and show only what was
        // actually captured.
        if live && !self.samples_auto_cleared {
            let before = self.captures.len();
            self.captures.retain(|row| row.is_live());
            let removed = before.saturating_sub(self.captures.len());
            if removed > 0 {
                self.log(
                    Level::Info,
                    "Capture",
                    format!("First live request arrived; cleared {} sample rows", removed),
                );
            }
            self.samples_auto_cleared = true;
        }
        // Keep the detail pane on the same call even though every new response
        // is inserted above it. If that row was a sample and the first live
        // capture cleared it, fall back to the newest row.
        self.selected = resolve_selection(self.follow_latest, keep_id, &self.captures);
    }

    fn clear_sample_captures(&mut self, ctx: &egui::Context) {
        let before = self.captures.len();
        let keep_id = if self.follow_latest {
            None
        } else {
            self.captures.get(self.selected).map(|row| row.id)
        };
        self.captures.retain(|c| c.is_live());
        let removed = before.saturating_sub(self.captures.len());
        self.selected = resolve_selection(self.follow_latest, keep_id, &self.captures);
        self.log(
            Level::Info,
            "System",
            format!("Cleared {} sample rows; live captures kept", removed),
        );
        self.toast_now(ctx, "Sample rows cleared — only live captures remain");
    }

    fn repeat_selected(&mut self, ctx: &egui::Context) {
        let Some(c) = self.captures.get(self.selected).cloned() else {
            return;
        };
        if self.catch_enabled {
            if let Some(rule) = self
                .breakpoints
                .iter()
                .find(|r| r.matches(&c.method, &c.url, &c.req_body))
                .cloned()
            {
                self.open_breakpoint(c, rule, false);
                return;
            }
        }
        if self.map_enabled {
            if let Some(rule) = self
                .map_rules
                .iter()
                .find(|r| r.matches(&c.method, &c.url, &c.req_body))
                .cloned()
            {
                let mut nc = c.clone();
                let served_body = rule.response_body();
                nc.status = rule.status;
                nc.latency_ms = rule.delay_ms;
                nc.size_bytes = served_body.len() as u64;
                nc.resp_body = served_body;
                nc.mock = Some(rule.name.clone());
                nc.started = now_stamp();
                nc.resp_headers = vec![
                    ("content-type".into(), rule.content_type.clone()),
                    ("x-proxzy-rule".into(), rule.name.clone()),
                ];
                let name = rule.name.clone();
                if let Some(r) = self.map_rules.iter_mut().find(|r| r.id == rule.id) {
                    r.hits += 1;
                }
                self.add_capture(nc);
                self.log(
                    Level::Warn,
                    "Map Local",
                    format!("Served \"{}\" for repeated {}", name, c.url),
                );
                let until = ctx.input(|i| i.time) + 2.4;
                self.toast = Some(Toast {
                    text: format!("Replayed through Map Local: {}", name),
                    until,
                });
                return;
            }
        }
        if c.is_live() && self.proxy.is_some() {
            let port = self.proxy_port;
            let method = c.method.clone();
            let url = c.url.clone();
            let headers = c.req_headers.clone();
            let body = c.req_body.clone();
            let ca_path = self.ca_info.as_ref().map(|ca| ca.pem_path.clone());
            std::thread::spawn(move || {
                let mut cmd = std::process::Command::new("curl");
                cmd.arg("-sS")
                    .arg("--max-time")
                    .arg("20")
                    .arg("-x")
                    .arg(format!("http://127.0.0.1:{}", port))
                    .arg("-X")
                    .arg(&method)
                    .arg("-o")
                    .arg("/dev/null");
                // curl does not know about our local CA unless we tell it,
                // otherwise every HTTPS replay fails the certificate check.
                if url.starts_with("https://") {
                    if let Some(path) = &ca_path {
                        cmd.arg("--cacert").arg(path);
                    } else {
                        cmd.arg("-k");
                    }
                }
                for (k, v) in &headers {
                    cmd.arg("-H").arg(format!("{}: {}", k, v));
                }
                if !body.is_empty() {
                    cmd.arg("--data-binary").arg(&body);
                }
                cmd.arg(&url);
                let _ = cmd.output();
            });
            self.log(
                Level::Info,
                "Capture",
                format!("Replaying {} {} through the live proxy", c.method, c.url),
            );
            let until = ctx.input(|i| i.time) + 2.4;
            self.toast = Some(Toast {
                text: "Request sent again through the live proxy".into(),
                until,
            });
            return;
        }
        let mut nc = c.clone();
        nc.latency_ms = (c.latency_ms + 4).min(999);
        nc.started = now_stamp();
        nc.mock = None;
        self.add_capture(nc);
        self.log(
            Level::Info,
            "Capture",
            format!("Replayed {} {} (local POC replay)", c.method, c.url),
        );
        let until = ctx.input(|i| i.time) + 2.4;
        self.toast = Some(Toast {
            text: "Request replayed in the local POC".into(),
            until,
        });
    }

    /// Ensure any domain present in a URL or pattern is added to `ssl_allowlist`
    /// so HTTPS Device UI Tests intercept and mock it without manual allowlist edits.
    fn ensure_ssl_domain_for_pattern(&mut self, pattern: &str) -> Option<String> {
        let domain = extract_domain_from_pattern(pattern)?;
        if !self
            .ssl_allowlist
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&domain) || domain_matches(d, &domain))
        {
            self.ssl_allowlist.push(domain.clone());
            self.log(
                Level::Info,
                "SSL Allowlist",
                format!(
                    "Auto-added \"{}\" to SSL Proxying Allowlist for Device UI Test interception",
                    domain
                ),
            );
            return Some(domain);
        }
        None
    }

    pub fn enable_ssl_for_domain(&mut self, domain: &str, ctx: &egui::Context) {
        let clean = domain
            .split(':')
            .next()
            .unwrap_or(domain)
            .trim()
            .to_lowercase();
        if clean.is_empty() {
            return;
        }
        if !self
            .ssl_allowlist
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&clean) || domain_matches(d, &clean))
        {
            self.ssl_allowlist.push(clean.clone());
            self.ssl_proxying_enabled = true;
            self.sync_proxy_config();
            self.toast_now(ctx, &format!("Enabled TLS decryption for {}", clean));
            self.log(
                Level::Info,
                "SSL Proxying",
                format!("Added \"{}\" to SSL Allowlist", clean),
            );
        } else {
            self.toast_now(ctx, &format!("{} is already in SSL Allowlist", clean));
        }
    }

    fn import_curl_to_composer(&mut self, ctx: &egui::Context) {
        let Some(parsed) = parse_curl(&self.composer_curl_input) else {
            self.toast_now(ctx, "Paste a valid curl command or http(s):// URL first");
            return;
        };
        self.composer_method = parsed.method.clone();
        self.composer_url = parsed.url.clone();
        self.composer_headers = parsed
            .headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join("\n");
        self.composer_body = parsed.body.clone();
        self.composer_tab = if parsed.body.is_empty() {
            ComposerTab::Headers
        } else {
            ComposerTab::Body
        };
        let _ = self.ensure_ssl_domain_for_pattern(&parsed.url);
        self.sync_proxy_config();
        self.log(
            Level::Info,
            "Composer",
            format!("Imported cURL: {} {}", parsed.method, parsed.url),
        );
        self.toast_now(ctx, "Imported cURL into Composer — click Send to inspect response");
    }

    fn save_composer_as_mock(&mut self, ctx: &egui::Context) {
        let url = self.composer_url.trim().to_string();
        if url.is_empty() {
            self.toast_now(ctx, "Enter or import a URL in Composer first");
            return;
        }
        let base = url.split('?').next().unwrap_or(&url).trim_end_matches('/');
        let pattern = if url.contains('?') {
            format!("{}?*", base)
        } else {
            format!("{}*", base)
        };
        let leaf = base
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or("endpoint");
        let name = format!("Mock {}", leaf);
        let method = rule_method_for(&self.composer_method);
        let (status, raw_body) = match &self.composer_response {
            Some(resp) if resp.status > 0 && !resp.body.is_empty() => {
                (resp.status, resp.body.clone())
            }
            _ => (200, MockScenario::Success.default_body()),
        };
        let body = sanitize_mock_payload(&raw_body);
        let scenario = if status >= 400 {
            MockScenario::Error
        } else {
            MockScenario::Success
        };
        let gql_name = graphql_info(&self.composer_method, &self.composer_body)
            .map(|g| {
                if g.operation == "Anonymous" {
                    String::new()
                } else {
                    g.operation
                }
            })
            .unwrap_or_default();

        let folder = if let Some(active_f) = &self.map_selected_folder {
            active_f.clone()
        } else if let Some(domain) = extract_domain_from_pattern(&pattern) {
            domain
        } else {
            "General".to_string()
        };

        let id = self.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        let mut rule = MapRule {
            id,
            name: name.clone(),
            folder,
            pattern: pattern.clone(),
            method,
            status,
            content_type: "application/json".into(),
            delay_ms: 20,
            body: body.clone(),
            graphql_name: gql_name,
            enabled: true,
            hits: 0,
            active_scenario: scenario,
            empty_body: auto_derive_empty_scenario(&body),
            ..MapRule::default()
        };
        match scenario {
            MockScenario::Success => {
                rule.success_status = status;
                rule.success_body = body;
            }
            MockScenario::Error => {
                rule.error_status = status;
                rule.error_body = body;
            }
            MockScenario::Empty => {
                rule.empty_status = status;
                rule.empty_body = body;
            }
        }
        self.map_rules.insert(0, rule);
        self.map_enabled = true;
        let _ = self.ensure_ssl_domain_for_pattern(&pattern);
        self.sync_proxy_config();
        self.log(
            Level::Info,
            "Map Local",
            format!("Saved Composer response as mock rule \"{}\" ({})", name, pattern),
        );
        self.toast_now(
            ctx,
            &format!("Saved as Map Local mock: {} (Engine ON + SSL Allowlisted)", name),
        );
    }

    fn add_quick_direct_mock(&mut self, ctx: &egui::Context) {
        let raw = self.quick_mock_url.trim().to_string();
        if raw.is_empty() {
            self.toast_now(ctx, "Enter a direct URL, domain/path pattern, or paste a cURL command");
            return;
        }
        let (method, pattern, gql_name) = if raw.to_ascii_lowercase().starts_with("curl ") {
            if let Some(parsed) = parse_curl(&raw) {
                let base = parsed.url.split('?').next().unwrap_or(&parsed.url);
                let pat = if parsed.url.contains('?') {
                    format!("{}?*", base)
                } else {
                    format!("{}*", base)
                };
                let gql = graphql_info(&parsed.method, &parsed.body)
                    .map(|g| if g.operation == "Anonymous" { String::new() } else { g.operation })
                    .unwrap_or_default();
                (rule_method_for(&parsed.method), pat, gql)
            } else {
                (self.quick_mock_method, raw.clone(), String::new())
            }
        } else {
            (self.quick_mock_method, raw.clone(), String::new())
        };

        let leaf = pattern
            .split('?')
            .next()
            .unwrap_or(&pattern)
            .trim_end_matches('*')
            .trim_end_matches('/')
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or("direct");
        let name = format!("Direct Mock {}", leaf);
        let scenario = self.quick_mock_scenario;
        let status = scenario.default_status();
        let body = if self.quick_mock_body.trim().is_empty() {
            scenario.default_body()
        } else {
            self.quick_mock_body.clone()
        };

        let folder = if let Some(active_f) = &self.map_selected_folder {
            active_f.clone()
        } else if let Some(domain) = extract_domain_from_pattern(&pattern) {
            domain
        } else {
            "General".to_string()
        };

        let id = self.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        let mut rule = MapRule {
            id,
            name: name.clone(),
            folder,
            pattern: pattern.clone(),
            method,
            status,
            content_type: "application/json".into(),
            delay_ms: 15,
            body: body.clone(),
            graphql_name: gql_name,
            enabled: true,
            hits: 0,
            active_scenario: scenario,
            ..MapRule::default()
        };
        match scenario {
            MockScenario::Success => {
                rule.success_status = status;
                rule.success_body = body;
            }
            MockScenario::Error => {
                rule.error_status = status;
                rule.error_body = body;
            }
            MockScenario::Empty => {
                rule.empty_status = status;
                rule.empty_body = body;
            }
        }
        self.map_rules.insert(0, rule);
        self.map_enabled = true;
        let _ = self.ensure_ssl_domain_for_pattern(&pattern);
        self.sync_proxy_config();
        self.quick_mock_url.clear();
        self.log(
            Level::Info,
            "Map Local",
            format!(
                "Hooked direct mock \"{}\" ({}) [{} {}]",
                name,
                pattern,
                scenario.label(),
                status
            ),
        );
        self.toast_now(
            ctx,
            &format!(
                "Direct Mock hooked on 127.0.0.1:{} ({})",
                self.proxy_port,
                scenario.label()
            ),
        );
    }

    fn open_map_editor(&mut self, rule: Option<MapRule>) {
        let default_folder = self.map_selected_folder.clone().unwrap_or_else(|| "General".into());
        self.map_editor = Some(match rule {
            Some(r) => {
                let folder = r.folder_name().to_string();
                MapEditor {
                    id: Some(r.id),
                    name: r.name,
                    folder,
                    pattern: r.pattern,
                    method: r.method,
                    status: r.status.to_string(),
                    content_type: r.content_type,
                    delay: r.delay_ms.to_string(),
                    body: r.body,
                    local_file: r.local_file,
                    graphql_name: r.graphql_name,
                    enabled: r.enabled,
                    active_scenario: r.active_scenario,
                    success_status: r.success_status.to_string(),
                    success_body: r.success_body,
                    error_status: r.error_status.to_string(),
                    error_body: r.error_body,
                    empty_status: r.empty_status.to_string(),
                    empty_body: r.empty_body,
                    curl_paste: String::new(),
                }
            },
            None => {
                let default_rule = MapRule::default();
                let default_body = r#"{
  "items": [
    { "id": "prd_local_1", "name": "Local Test Product", "price": 42.00 }
  ],
  "source": "map-local"
}"#
                .to_string();
                MapEditor {
                    id: None,
                    name: "Local fixture".into(),
                    folder: default_folder,
                    pattern: "*/v1/products?*".into(),
                    method: RuleMethod::Get,
                    status: "200".into(),
                    content_type: "application/json".into(),
                    delay: "20".into(),
                    body: default_body.clone(),
                    local_file: String::new(),
                    graphql_name: String::new(),
                    enabled: true,
                    active_scenario: MockScenario::Success,
                    success_status: "200".into(),
                    success_body: default_body,
                    error_status: default_rule.error_status.to_string(),
                    error_body: default_rule.error_body,
                    empty_status: default_rule.empty_status.to_string(),
                    empty_body: default_rule.empty_body,
                    curl_paste: String::new(),
                }
            }
        });
    }

    fn open_map_editor_for_capture(&mut self, c: &Capture) {
        let content_type = header_value(&c.resp_headers, "content-type")
            .unwrap_or_else(|| "application/json".into());
        let status = if c.status == 0 { 200 } else { c.status };
        let body = if c.resp_body.is_empty() {
            "{\n  \"source\": \"map-local\"\n}".into()
        } else {
            c.resp_body.clone()
        };
        let active_scenario = if status >= 400 {
            MockScenario::Error
        } else {
            MockScenario::Success
        };
        let mut rule = MapRule {
            id: u64::MAX,
            name: capture_rule_name(c, "Mock"),
            pattern: capture_pattern(c),
            method: rule_method_for(&c.method),
            status,
            content_type,
            delay_ms: c.latency_ms,
            body: body.clone(),
            graphql_name: c
                .graphql
                .as_ref()
                .map(|g| g.operation.clone())
                .unwrap_or_default(),
            enabled: true,
            hits: 0,
            active_scenario,
            ..MapRule::default()
        };
        if active_scenario == MockScenario::Error {
            rule.error_status = status;
            rule.error_body = body;
        } else {
            rule.success_status = status;
            rule.success_body = body;
        }
        self.open_map_editor(Some(rule));
        // Mark as a new rule even though it was filled from a capture.
        if let Some(ed) = &mut self.map_editor {
            ed.id = None;
        }
    }

    fn save_map_editor(&mut self, ctx: &egui::Context) {
        let Some(mut ed) = self.map_editor.take() else {
            return;
        };
        if ed.name.trim().is_empty() || ed.pattern.trim().is_empty() {
            self.map_editor = Some(ed);
            self.toast_now(ctx, "Give the mapping a name and a URL pattern");
            return;
        }
        ed.sync_current_to_slot();
        let folder = if ed.folder.trim().is_empty() {
            if let Some(active_f) = &self.map_selected_folder {
                active_f.clone()
            } else if let Some(domain) = extract_domain_from_pattern(&ed.pattern) {
                domain
            } else {
                "General".to_string()
            }
        } else {
            ed.folder.trim().to_string()
        };
        let rule = MapRule {
            id: ed.id.unwrap_or(0),
            name: ed.name.trim().to_string(),
            folder,
            pattern: ed.pattern.trim().to_string(),
            method: ed.method,
            status: ed.status.trim().parse::<u16>().unwrap_or(200),
            content_type: if ed.content_type.trim().is_empty() {
                "application/json".into()
            } else {
                ed.content_type.trim().to_string()
            },
            delay_ms: ed.delay.trim().parse::<u64>().unwrap_or(0),
            body: ed.body,
            local_file: ed.local_file.trim().to_string(),
            graphql_name: ed.graphql_name.trim().to_string(),
            enabled: ed.enabled,
            hits: 0,
            active_scenario: ed.active_scenario,
            success_status: ed.success_status.trim().parse::<u16>().unwrap_or(200),
            success_body: ed.success_body,
            error_status: ed.error_status.trim().parse::<u16>().unwrap_or(500),
            error_body: ed.error_body,
            empty_status: ed.empty_status.trim().parse::<u16>().unwrap_or(200),
            empty_body: ed.empty_body,
            ..MapRule::default()
        };
        let _ = self.ensure_ssl_domain_for_pattern(&rule.pattern);
        if let Some(id) = ed.id {
            if let Some(existing) = self.map_rules.iter_mut().find(|r| r.id == id) {
                let hits = existing.hits;
                *existing = MapRule { hits, ..rule.clone() };
            } else {
                let id = self.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                self.map_rules.push(MapRule { id, ..rule.clone() });
            }
            self.log(Level::Info, "Map Local", format!("Updated mapping \"{}\"", rule.name));
        } else {
            let id = self.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
            self.map_rules.push(MapRule { id, ..rule.clone() });
            self.log(Level::Info, "Map Local", format!("New mapping created: {}", rule.name));
        }
        self.map_enabled = true;
        self.sync_proxy_config();
        self.toast_now(ctx, &format!("Mapping saved: {}", rule.name));
    }

    fn open_bp_editor(&mut self, rule: Option<BreakpointRule>) {
        self.bp_editor = Some(match rule {
            Some(r) => BpEditor {
                id: Some(r.id),
                name: r.name,
                pattern: r.pattern,
                graphql_name: r.graphql_name,
                stage: r.stage,
                enabled: r.enabled,
            },
            None => BpEditor {
                id: None,
                name: "Catch a request".into(),
                pattern: "*/v1/*?*".into(),
                graphql_name: String::new(),
                stage: BpStage::Request,
                enabled: true,
            },
        });
    }

    fn open_bp_editor_for_capture(&mut self, c: &Capture) {
        self.open_bp_editor(Some(BreakpointRule {
            id: u64::MAX,
            name: capture_rule_name(c, "Catch"),
            pattern: capture_pattern(c),
            graphql_name: c
                .graphql
                .as_ref()
                .map(|g| g.operation.clone())
                .unwrap_or_default(),
            stage: BpStage::Request,
            enabled: true,
            hits: 0,
        }));
        if let Some(ed) = &mut self.bp_editor {
            ed.id = None;
        }
    }

    fn save_bp_editor(&mut self, ctx: &egui::Context, catch_next: bool) {
        let Some(mut ed) = self.bp_editor.take() else {
            return;
        };
        if ed.name.trim().is_empty() || ed.pattern.trim().is_empty() {
            self.bp_editor = Some(ed);
            self.toast_now(ctx, "Give the breakpoint a name and a URL pattern");
            return;
        }
        if catch_next {
            ed.enabled = true;
        }
        let mut rule = BreakpointRule {
            id: ed.id.unwrap_or(0),
            name: ed.name.trim().to_string(),
            pattern: ed.pattern.trim().to_string(),
            graphql_name: ed.graphql_name.trim().to_string(),
            stage: ed.stage,
            enabled: ed.enabled,
            hits: 0,
        };
        if let Some(id) = ed.id {
            if let Some(existing) = self.breakpoints.iter_mut().find(|r| r.id == id) {
                let hits = existing.hits;
                *existing = BreakpointRule { hits, ..rule.clone() };
            } else {
                let id = self.breakpoints.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                rule.id = id;
                self.breakpoints.push(rule.clone());
            }
            self.log(Level::Info, "Breakpoints", format!("Updated rule \"{}\"", rule.name));
        } else {
            let id = self.breakpoints.iter().map(|r| r.id).max().unwrap_or(0) + 1;
            rule.id = id;
            self.breakpoints.push(rule.clone());
            self.log(Level::Info, "Breakpoints", format!("New catch rule: {}", rule.name));
        }
        if catch_next {
            self.catch_with_rule(&rule, ctx);
        } else {
            self.toast_now(ctx, &format!("Breakpoint saved: {}", rule.name));
        }
    }

    fn catch_with_rule(&mut self, rule: &BreakpointRule, ctx: &egui::Context) {
        self.catch_enabled = true;
        let selected_matches = self
            .captures
            .get(self.selected)
            .map(|c| rule.matches(&c.method, &c.url, &c.req_body))
            .unwrap_or(false);
        let index = if selected_matches {
            Some(self.selected)
        } else {
            self.captures
                .iter()
                .position(|c| rule.matches(&c.method, &c.url, &c.req_body))
        };
        let Some(index) = index else {
            self.toast_now(
                ctx,
                "No captured request matches yet — send one from Composer or loosen the pattern",
            );
            return;
        };
        self.selected = index;
        let capture = self.captures[index].clone();
        if let Some(r) = self.breakpoints.iter_mut().find(|r| r.id == rule.id) {
            r.hits += 1;
        }
        self.log(
            Level::Warn,
            "Breakpoints",
            format!("Caught {} {} with \"{}\"", capture.method, capture.host_path(), rule.name),
        );
        self.open_breakpoint(capture, rule.clone(), false);
    }

    fn catch_capture_now(&mut self, c: &Capture, ctx: &egui::Context) {
        let existing = self
            .breakpoints
            .iter()
            .find(|r| r.matches(&c.method, &c.url, &c.req_body))
            .cloned();
        let rule = existing.unwrap_or_else(|| BreakpointRule {
            id: 0,
            name: capture_rule_name(c, "Catch"),
            pattern: capture_pattern(c),
            graphql_name: c
                .graphql
                .as_ref()
                .map(|g| g.operation.clone())
                .unwrap_or_default(),
            stage: BpStage::Request,
            enabled: true,
            hits: 0,
        });
        if rule.id == 0 {
            let id = self.breakpoints.iter().map(|r| r.id).max().unwrap_or(0) + 1;
            let mut saved = rule.clone();
            saved.id = id;
            self.breakpoints.push(saved.clone());
            self.log(
                Level::Info,
                "Breakpoints",
                format!("Armed a new catch rule: {}", saved.name),
            );
            self.catch_with_rule(&saved, ctx);
        } else {
            self.catch_with_rule(&rule, ctx);
        }
        self.view = View::Breakpoints;
    }

    fn open_breakpoint(
        &mut self,
        capture: Capture,
        rule: BreakpointRule,
        from_composer: bool,
    ) {
        let stage = if from_composer {
            self.bp_stage_override
        } else {
            rule.stage
        };
        self.breakpoint_session = Some(BreakpointSession {
            rule_name: rule.name,
            stage,
            live_bp_id: None,
            method: capture.method.clone(),
            url: capture.url.clone(),
            headers: capture
                .req_headers
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            body: if stage == BpStage::Request {
                capture.req_body.clone()
            } else {
                capture.resp_body.clone()
            },
            base_status: capture.status,
            base_resp: capture.resp_body.clone(),
            base_latency: capture.latency_ms,
        });
    }

    fn open_live_breakpoint(&mut self, bp: proxy::LiveBreakpoint) {
        self.breakpoint_session = Some(BreakpointSession {
            rule_name: bp.rule_name.clone(),
            stage: BpStage::Request,
            live_bp_id: Some(bp.id),
            method: bp.method,
            url: bp.url,
            headers: bp.headers,
            body: bp.body,
            base_status: 200,
            base_resp: String::new(),
            base_latency: 0,
        });
        self.log(
            Level::Warn,
            "Breakpoints",
            format!("Live request paused by \"{}\"", bp.rule_name),
        );
    }

    fn continue_breakpoint(&mut self, ctx: &egui::Context) {
        let Some(s) = self.breakpoint_session.take() else {
            return;
        };
        if let Some(id) = s.live_bp_id {
            if let Some(handle) = &self.proxy {
                handle.resolve(
                    id,
                    proxy::BpDecision::Continue {
                        method: s.method.clone(),
                        url: s.url.clone(),
                        headers: s.headers.clone(),
                        body: s.body.clone(),
                    },
                );
            }
            self.log(
                Level::Info,
                "Breakpoints",
                format!("Live request continued from \"{}\"", s.rule_name),
            );
            let until = ctx.input(|i| i.time) + 2.6;
            self.toast = Some(Toast {
                text: "Breakpoint continued — the real request was released".into(),
                until,
            });
            return;
        }
        let mut c = Capture {
            id: 0,
            method: s.method.clone(),
            url: s.url.clone(),
            status: if s.stage == BpStage::Response {
                s.base_status
            } else {
                200
            },
            latency_ms: s.base_latency.max(24) + 12,
            size_bytes: 0,
            kind: detect_kind(&s.method, &s.url, &s.body),
            started: now_stamp(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: parse_headers(&s.headers),
            req_body: if s.stage == BpStage::Request {
                s.body.clone()
            } else {
                String::new()
            },
            resp_headers: vec![
                ("content-type".into(), "application/json".into()),
                ("x-proxzy-breakpoint".into(), s.rule_name.clone()),
            ],
            resp_body: if s.stage == BpStage::Response {
                s.body.clone()
            } else {
                s.base_resp.clone()
            },
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        };
        c.size_bytes = c.resp_body.len() as u64;
        if c.kind == ApiKind::Graphql {
            c.graphql = graphql_info(&c.method, &c.req_body);
        }
        if s.stage == BpStage::Request && c.req_body.is_empty() {
            c.req_body = "{}".into();
        }
        if s.stage == BpStage::Response {
            c.req_body = "{}".into();
        }
        self.add_capture(c);
        self.view = View::Traffic;
        self.inspector_tab = InspectorTab::Overview;
        self.log(
            Level::Info,
            "Breakpoints",
            format!(
                "Paused {} · {} edited and continued",
                s.rule_name,
                s.stage.label().to_lowercase()
            ),
        );
        let until = ctx.input(|i| i.time) + 2.6;
        self.toast = Some(Toast {
            text: "Breakpoint continued — edited request recorded".into(),
            until,
        });
    }

    fn abort_breakpoint(&mut self, ctx: &egui::Context) {
        let Some(s) = self.breakpoint_session.take() else {
            return;
        };
        if let Some(id) = s.live_bp_id {
            if let Some(handle) = &self.proxy {
                handle.resolve(id, proxy::BpDecision::Abort);
            }
            self.log(
                Level::Warn,
                "Breakpoints",
                format!("Live request aborted by \"{}\"", s.rule_name),
            );
            let until = ctx.input(|i| i.time) + 2.4;
            self.toast = Some(Toast {
                text: "Live request aborted — the app receives a 403".into(),
                until,
            });
            return;
        }
        let c = Capture {
            id: 0,
            method: s.method.clone(),
            url: s.url.clone(),
            status: 0,
            latency_ms: 0,
            size_bytes: 0,
            kind: detect_kind(&s.method, &s.url, ""),
            started: now_stamp(),
            remote: "-".into(),
            protocol: "-".into(),
            req_headers: parse_headers(&s.headers),
            req_body: s.body.clone(),
            resp_headers: vec![("x-proxzy-breakpoint".into(), s.rule_name.clone())],
            resp_body: String::new(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: true,
        };
        self.add_capture(c);
        self.view = View::Traffic;
        self.inspector_tab = InspectorTab::Overview;
        self.log(
            Level::Warn,
            "Breakpoints",
            format!("Request aborted at breakpoint \"{}\"", s.rule_name),
        );
        let until = ctx.input(|i| i.time) + 2.4;
        self.toast = Some(Toast {
            text: "Request aborted — nothing was sent".into(),
            until,
        });
    }

    fn send_composer(&mut self, ctx: &egui::Context) {
        let method = self.composer_method.clone();
        let url = self.composer_url.trim().to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            let until = ctx.input(|i| i.time) + 2.6;
            self.toast = Some(Toast {
                text: "Enter a full http:// or https:// URL".into(),
                until,
            });
            return;
        }
        let headers = parse_headers(&self.composer_headers);
        let body = self.composer_body.clone();

        if self.map_enabled {
            if let Some(rule) = self
                .map_rules
                .iter()
                .find(|r| r.matches(&method, &url, &body))
                .cloned()
            {
                let served_body = rule.response_body();
                let mut c = Capture {
                    id: 0,
                    method: method.clone(),
                    url: url.clone(),
                    status: rule.status,
                    latency_ms: rule.delay_ms,
                    size_bytes: served_body.len() as u64,
                    kind: detect_kind(&method, &url, &body),
                    started: now_stamp(),
                    remote: "map-local".into(),
                    protocol: "Local".into(),
                    req_headers: headers.clone(),
                    req_body: body.clone(),
                    resp_headers: vec![
                        ("content-type".into(), rule.content_type.clone()),
                        ("x-proxzy-rule".into(), rule.name.clone()),
                    ],
                    resp_body: served_body,
                    graphql: None,
                    mock: Some(rule.name.clone()),
                    pinned: false,
                    aborted: false,
                };
                if c.kind == ApiKind::Graphql {
                    c.graphql = graphql_info(&method, &body);
                }
                if let Some(r) = self.map_rules.iter_mut().find(|r| r.id == rule.id) {
                    r.hits += 1;
                }
                self.composer_response = Some(ComposerResponse {
                    status: rule.status,
                    latency_ms: rule.delay_ms,
                    size: c.size_bytes,
                    headers: c.resp_headers.clone(),
                    body: c.resp_body.clone(),
                    mocked: Some(rule.name.clone()),
                    error: None,
                });
                self.add_capture(c);
                self.log(
                    Level::Warn,
                    "Map Local",
                    format!("Composer request matched \"{}\"", rule.name),
                );
                let until = ctx.input(|i| i.time) + 2.4;
                self.toast = Some(Toast {
                    text: format!("Served by Map Local: {}", rule.name),
                    until,
                });
                return;
            }
        }

        if self.catch_enabled {
            if let Some(rule) = self
                .breakpoints
                .iter()
                .find(|r| r.matches(&method, &url, &body))
                .cloned()
            {
                let mut capture = Capture {
                    id: 0,
                    method: method.clone(),
                    url: url.clone(),
                    status: 200,
                    latency_ms: 40,
                    size_bytes: 0,
                    kind: detect_kind(&method, &url, &body),
                    started: now_stamp(),
                    remote: "-".into(),
                    protocol: "HTTP/2".into(),
                    req_headers: headers.clone(),
                    req_body: body.clone(),
                    resp_headers: Vec::new(),
                    resp_body: String::new(),
                    graphql: None,
                    mock: None,
                    pinned: false,
                    aborted: false,
                };
                if capture.kind == ApiKind::Graphql {
                    capture.graphql = graphql_info(&method, &body);
                }
                self.open_breakpoint(capture, rule, true);
                return;
            }
        }

        let (tx, rx) = channel::<SendOutcome>();
        let ctx2 = ctx.clone();
        let method2 = method.clone();
        let url2 = url.clone();
        let headers2 = headers.clone();
        let body2 = body.clone();
        std::thread::spawn(move || {
            let outcome = run_curl(&method2, &url2, &headers2, &body2, None);
            let _ = tx.send(outcome);
            ctx2.request_repaint();
        });
        self.pending_send = Some(rx);
        self.composer_response = None;
    }

    fn poll_send(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.pending_send.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(out) => {
                let mut c = Capture {
                    id: 0,
                    method: out.method.clone(),
                    url: out.url.clone(),
                    status: out.status,
                    latency_ms: out.latency_ms,
                    size_bytes: out.size,
                    kind: detect_kind(&out.method, &out.url, &out.req_body),
                    started: now_stamp(),
                    remote: host_port_of(&out.url),
                    protocol: "HTTP/1.1".into(),
                    req_headers: out.headers.clone(),
                    req_body: out.req_body.clone(),
                    resp_headers: out.resp_headers.clone(),
                    resp_body: out.body.clone(),
                    graphql: None,
                    mock: None,
                    pinned: false,
                    aborted: false,
                };
            if c.kind == ApiKind::Graphql {
                c.graphql = graphql_info(&out.method, &out.req_body);
            }
                self.composer_response = Some(ComposerResponse {
                    status: out.status,
                    latency_ms: out.latency_ms,
                    size: out.size,
                    headers: out.resp_headers.clone(),
                    body: out.body.clone(),
                    mocked: None,
                    error: out.error.clone(),
                });
                self.add_capture(c);
                if let Some(err) = &out.error {
                    self.log(
                        Level::Error,
                        "Composer",
                        format!("{} {} failed: {}", out.method, out.url, err),
                    );
                } else {
                    self.log(
                        Level::Info,
                        "Composer",
                        format!(
                            "{} {} → {} in {} ms",
                            out.method, out.url, out.status, out.latency_ms
                        ),
                    );
                }
            }
            Err(TryRecvError::Empty) => {
                self.pending_send = Some(rx);
                ctx.request_repaint_after(Duration::from_millis(120));
            }
            Err(TryRecvError::Disconnected) => {
                self.composer_response = Some(ComposerResponse {
                    status: 0,
                    latency_ms: 0,
                    size: 0,
                    headers: Vec::new(),
                    body: String::new(),
                    mocked: None,
                    error: Some("The send worker stopped unexpectedly.".into()),
                });
            }
        }
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        if self.screenshot_path.is_none() {
            return;
        }
        self.frames += 1;
        ctx.request_repaint();
        if self.frames == self.screenshot_frames.max(30) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let path = self.screenshot_path.clone().unwrap_or_default();
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let w = image.size[0] as u32;
            let h = image.size[1] as u32;
            let mut bytes = Vec::with_capacity((w * h * 4) as usize);
            for px in &image.pixels {
                bytes.extend_from_slice(&px.to_array());
            }
            let _ = image::save_buffer(
                &path,
                &bytes,
                w,
                h,
                image::ColorType::Rgba8,
            );
            std::process::exit(0);
        }
    }

    fn open_palette(&mut self) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_focus = true;
    }

    fn palette_entries(&self) -> Vec<PaletteEntry> {
        vec![
            PaletteEntry {
                group: "Screens",
                label: "All Traffic",
                hint: "Every captured request, with filters and search",
                icon: NavIcon::Traffic,
                action: PaletteAction::Go(View::Traffic),
            },
            PaletteEntry {
                group: "Screens",
                label: "Log Dashboard",
                hint: "Timeline and dashboard for traffic, mocks and breakpoints",
                icon: NavIcon::Logs,
                action: PaletteAction::Go(View::Logs),
            },
            PaletteEntry {
                group: "Screens",
                label: "Live Capture",
                hint: "Start the real proxy and connect a device",
                icon: NavIcon::Live,
                action: PaletteAction::Go(View::Live),
            },
            PaletteEntry {
                group: "Screens",
                label: "Composer",
                hint: "Write and send a GraphQL or REST request",
                icon: NavIcon::Composer,
                action: PaletteAction::Go(View::Composer),
            },
            PaletteEntry {
                group: "Screens",
                label: "Map Local",
                hint: "Answer matching requests with local JSON",
                icon: NavIcon::Map,
                action: PaletteAction::Go(View::MapLocal),
            },
            PaletteEntry {
                group: "Screens",
                label: "Breakpoints",
                hint: "Pause a request, edit it, then continue or abort",
                icon: NavIcon::Breakpoint,
                action: PaletteAction::Go(View::Breakpoints),
            },
            PaletteEntry {
                group: "Screens",
                label: "Agent Control",
                hint: "Loopback control plane for an IDE or agent",
                icon: NavIcon::Agent,
                action: PaletteAction::Go(View::Agent),
            },
            PaletteEntry {
                group: "Screens",
                label: "ACP Bridge",
                hint: "Editor-to-agent session, permission gate and setup",
                icon: NavIcon::Bridge,
                action: PaletteAction::Go(View::Acp),
            },
            PaletteEntry {
                group: "Screens",
                label: "API Test Bench",
                hint: "Run saved requests with pass or fail checks",
                icon: NavIcon::Tests,
                action: PaletteAction::Go(View::Tests),
            },
            PaletteEntry {
                group: "Screens",
                label: "Screen Play & MCP",
                hint: "Script engine and device steps into one play",
                icon: NavIcon::Play,
                action: PaletteAction::Go(View::Screen),
            },
            PaletteEntry {
                group: "Screens",
                label: "Tutorial",
                hint: "A guided tour of every main screen",
                icon: NavIcon::Compass,
                action: PaletteAction::Go(View::Tutorial),
            },
            PaletteEntry {
                group: "Screens",
                label: "Guide & Stack",
                hint: "How the app, proxy and certificates fit together",
                icon: NavIcon::Help,
                action: PaletteAction::Go(View::Guide),
            },
            PaletteEntry {
                group: "Quick actions",
                label: "Run practice session",
                hint: "Send five real calls through the local proxy",
                icon: NavIcon::Play,
                action: PaletteAction::RunPractice,
            },
            PaletteEntry {
                group: "Quick actions",
                label: "Start guided tutorial",
                hint: "Open the guided tour from the first screen",
                icon: NavIcon::Compass,
                action: PaletteAction::StartTour,
            },
            PaletteEntry {
                group: "Quick actions",
                label: "Clear traffic history",
                hint: "Remove captured rows but keep the activity log",
                icon: NavIcon::Error,
                action: PaletteAction::ClearTraffic,
            },
            PaletteEntry {
                group: "Quick actions",
                label: "Copy session summary",
                hint: "Put the activity summary on the clipboard",
                icon: NavIcon::Logs,
                action: PaletteAction::CopySummary,
            },
        ]
    }

    fn palette_badge(&self, action: PaletteAction) -> Option<String> {
        match action {
            PaletteAction::Go(View::Traffic) => Some(format!("{} rows", self.captures.len())),
            PaletteAction::Go(View::Logs) => Some(format!("{} events", self.logs.len())),
            PaletteAction::Go(View::MapLocal) => Some(format!(
                "{} on",
                self.map_rules.iter().filter(|r| r.enabled).count()
            )),
            PaletteAction::Go(View::Breakpoints) => Some(format!(
                "{} armed",
                self.breakpoints.iter().filter(|b| b.enabled).count()
            )),
            _ => None,
        }
    }

    fn palette_activate(&mut self, action: PaletteAction, ctx: &egui::Context) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_index = 0;
        match action {
            PaletteAction::Go(view) => {
                self.tour = None;
                self.share_open = false;
                if view == View::Traffic {
                    self.filter = TrafficFilter::All;
                }
                if view == View::Logs {
                    self.log_level_filter = None;
                }
                self.view = view;
            }
            PaletteAction::StartTour => {
                self.share_open = false;
                self.begin_tour(0);
            }
            PaletteAction::RunPractice => {
                self.view = View::Traffic;
                self.filter = TrafficFilter::All;
                self.start_practice_session(ctx);
            }
            PaletteAction::ClearTraffic => {
                self.captures.clear();
                self.selected = 0;
                self.follow_latest = true;
                self.log(
                    Level::Warn,
                    "System",
                    "Traffic cleared from All features",
                );
            }
            PaletteAction::CopySummary => {
                let summary = self.summary_text();
                self.copy(ctx, summary, "Session summary copied");
            }
        }
    }

    /// A searchable sheet over the whole window. It is deliberately available
    /// from the top bar, the bottom of the rail and Cmd/Ctrl+K, so a hidden or
    /// scrolled-away screen can always be selected without hunting.
    fn feature_palette(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.palette_open = false;
            return;
        }

        let screen = ctx.screen_rect();
        let mut close = false;
        egui::Area::new(Id::new("features-backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let resp = ui.allocate_rect(screen, Sense::click());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(150));
                if resp.clicked() {
                    close = true;
                }
            });

        let width = 640.0_f32.min((screen.width() - 40.0).max(340.0));
        let max_height = 620.0_f32.min((screen.height() - 44.0).max(320.0));
        let panel_pos = screen.center() - Vec2::new(width / 2.0, max_height / 2.0);
        let mut chosen: Option<PaletteAction> = None;
        let mut submit = false;

        let query = self.palette_query.trim().to_lowercase();
        let entries: Vec<PaletteEntry> = self
            .palette_entries()
            .into_iter()
            .filter(|entry| {
                query.is_empty()
                    || entry.label.to_lowercase().contains(&query)
                    || entry.hint.to_lowercase().contains(&query)
                    || entry.group.to_lowercase().contains(&query)
            })
            .collect();
        if self.palette_index >= entries.len() {
            self.palette_index = entries.len().saturating_sub(1);
        }
        let badges: Vec<Option<String>> = entries
            .iter()
            .map(|entry| self.palette_badge(entry.action))
            .collect();

        egui::Area::new(Id::new("features-palette"))
            .order(egui::Order::Tooltip)
            .fixed_pos(panel_pos)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(14.0)
                    .inner_margin(Margin::same(18.0))
                    .show(ui, |ui| {
                        ui.set_width(width - 36.0);
                        ui.set_max_height(max_height - 36.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(
                                    RichText::new("All features")
                                        .size(20.0)
                                        .strong()
                                        .color(TEXT),
                                );
                                ui.label(
                                    RichText::new(
                                        "Every screen and action, in one list. Scroll or search to reach any.",
                                    )
                                    .size(10.5)
                                    .color(MUTED),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "Close").clicked() {
                                    close = true;
                                }
                                pill(
                                    ui,
                                    "ALWAYS AVAILABLE",
                                    ACCENT,
                                    Color32::from_rgba_unmultiplied(88, 140, 255, 24),
                                );
                            });
                        });
                        ui.add_space(12.0);

                        Frame::none()
                            .fill(INPUT)
                            .stroke(Stroke::new(1.0, BORDER))
                            .rounding(9.0)
                            .inner_margin(Margin::symmetric(10.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Search")
                                            .size(9.5)
                                            .color(FAINT)
                                            .strong(),
                                    );
                                    let search_id = Id::new("palette-search");
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut self.palette_query)
                                            .id(search_id)
                                            .hint_text("Type a screen, tool or action")
                                            .desired_width(f32::INFINITY)
                                            .frame(false)
                                            .text_color(TEXT)
                                            .font(FontId::proportional(13.0)),
                                    );
                                    if self.palette_focus {
                                        resp.request_focus();
                                        self.palette_focus = false;
                                    }
                                    if resp.changed() {
                                        self.palette_index = 0;
                                    }
                                    if resp.has_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        submit = true;
                                    }
                                });
                            });
                        ui.add_space(8.0);

                        if entries.is_empty() {
                            Frame::none()
                                .fill(CARD)
                                .rounding(9.0)
                                .inner_margin(Margin::same(18.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("No screen matches that search")
                                            .size(12.5)
                                            .color(TEXT),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "Try \"traffic\", \"mock\", \"log\", \"agent\" or \"tutorial\".",
                                        )
                                        .size(10.0)
                                        .color(FAINT),
                                    );
                                });
                        } else {
                            let list_h = (max_height - 250.0).clamp(160.0, 390.0);
                            subtle_scroll(ui);
                            ScrollArea::vertical()
                                .max_height(list_h)
                                .auto_shrink([false, false])
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                                )
                                .show(ui, |ui| {
                                    let mut group = "";
                                    for (index, entry) in entries.iter().enumerate() {
                                        if entry.group != group {
                                            if !group.is_empty() {
                                                ui.add_space(7.0);
                                            }
                                            section_label(ui, entry.group);
                                            ui.add_space(2.0);
                                            group = entry.group;
                                        }
                                        let badge = badges[index].as_deref();
                                        if palette_row(
                                            ui,
                                            entry,
                                            index == self.palette_index,
                                            badge,
                                        )
                                        .clicked()
                                        {
                                            chosen = Some(entry.action);
                                        }
                                    }
                                });
                        }

                        ui.add_space(8.0);
                        hsep(ui);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(
                                    "Cmd K opens this anywhere. Enter picks the first match. Esc closes.",
                                )
                                .size(9.5)
                                .color(FAINT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if !entries.is_empty()
                                    && primary_button(ui, "Open selected").clicked()
                                {
                                    submit = true;
                                }
                            });
                        });
                    });
            });

        if submit && !entries.is_empty() {
            chosen = Some(entries[self.palette_index].action);
        }
        if let Some(action) = chosen {
            self.palette_activate(action, ctx);
        } else if close {
            self.palette_open = false;
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar")
            .exact_height(56.0)
            .frame(
                Frame::none()
                    .fill(RAIL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(Margin::symmetric(16.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    draw_mark(ui, 28.0);
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new("Proxzy")
                                .size(16.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new("NETWORK INSPECTOR")
                                .size(8.0)
                                .color(FAINT),
                        );
                    });
                    ui.add_space(10.0);
                    vsep(ui, 24.0);
                    let features =
                        secondary_button(ui, "Features").on_hover_text(
                            "Every screen and action in one list (Cmd K)",
                        );
                    if features.clicked() {
                        self.open_palette();
                    }

                    let (rec_rect, rec_resp) =
                        ui.allocate_exact_size(Vec2::new(106.0, 28.0), Sense::click());
                    let (rec_fg, rec_bg) = if self.recording {
                        (GREEN, Color32::from_rgba_unmultiplied(61, 214, 149, 18))
                    } else {
                        (MUTED, CARD)
                    };
                    ui.painter().rect(
                        rec_rect,
                        7.0,
                        rec_bg,
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(rec_fg.r(), rec_fg.g(), rec_fg.b(), 90)),
                    );
                    ui.painter().circle_filled(
                        egui::pos2(rec_rect.left() + 13.0, rec_rect.center().y),
                        3.5,
                        rec_fg,
                    );
                    ui.painter().text(
                        egui::pos2(rec_rect.left() + 24.0, rec_rect.center().y),
                        Align2::LEFT_CENTER,
                        if self.recording { "Recording" } else { "Paused" },
                        FontId::proportional(11.5),
                        rec_fg,
                    );
                    if rec_resp
                        .on_hover_text("Pause or resume the demo capture feed")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.recording = !self.recording;
                        let msg = if self.recording {
                            "Capture resumed"
                        } else {
                            "Capture paused"
                        };
                        self.log(
                            if self.recording { Level::Info } else { Level::Warn },
                            "System",
                            msg,
                        );
                    }

                    if secondary_button(ui, "Clear").clicked() {
                        self.captures.clear();
                        self.selected = 0;
                        self.follow_latest = true;
                        self.log(Level::Warn, "System", "Traffic cleared by user");
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (avatar_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        ui.painter().circle_filled(
                            avatar_rect.center(),
                            14.0,
                            Color32::from_rgb(41, 57, 84),
                        );
                        ui.painter().circle_stroke(
                            avatar_rect.center(),
                            14.0,
                            Stroke::new(1.0, Color32::from_rgb(68, 91, 130)),
                        );
                        ui.painter().text(
                            avatar_rect.center(),
                            Align2::CENTER_CENTER,
                            "PX",
                            FontId::monospace(9.5),
                            Color32::from_rgb(173, 199, 242),
                        );
                        ui.add_space(4.0);
                        let help = icon_button(ui, "?", "Open the guide");
                        if help.clicked() {
                            self.share_open = false;
                            self.view = View::Guide;
                        }
                        let tutorial = tool_chip(
                            ui,
                            "Tutorial",
                            None,
                            self.view == View::Tutorial || self.tour.is_some(),
                        );
                        if tutorial.clicked() {
                            self.share_open = false;
                            self.tour = None;
                            self.view = View::Tutorial;
                        }
                        ui.add_space(2.0);
                        vsep(ui, 22.0);
                        ui.add_space(2.0);
                        let live = tool_chip(
                            ui,
                            "Live",
                            None,
                            self.view == View::Live,
                        );
                        if live.clicked() {
                            self.view = View::Live;
                        }
                        let bp = tool_chip(
                            ui,
                            "Breakpoints",
                            Some(
                                self.breakpoints.iter().filter(|b| b.enabled).count(),
                            ),
                            self.view == View::Breakpoints,
                        );
                        if bp.clicked() {
                            self.view = View::Breakpoints;
                        }
                        let map = tool_chip(
                            ui,
                            "Map Local",
                            Some(self.map_rules.iter().filter(|r| r.enabled).count()),
                            self.view == View::MapLocal,
                        );
                        if map.clicked() {
                            self.view = View::MapLocal;
                        }
                        let logs = tool_chip(
                            ui,
                            "Logs",
                            Some(self.logs.len()),
                            self.view == View::Logs,
                        )
                        .on_hover_text(
                            "Log Dashboard — every capture, mock and breakpoint event",
                        );
                        if logs.clicked() {
                            self.view = View::Logs;
                        }
                        ui.add_space(4.0);
                        search_box(ui, &mut self.search, 176.0);
                    });
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(26.0)
            .frame(
                Frame::none()
                    .fill(RAIL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(Margin::symmetric(14.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let live_on = self.proxy.is_some();
                    dot(ui, if live_on && self.recording { GREEN } else { FAINT }, 6.0);
                    ui.label(
                        RichText::new(if live_on {
                            "Live proxy"
                        } else {
                            "Proxy stopped"
                        })
                        .size(10.0)
                        .color(if live_on { MUTED } else { AMBER }),
                    );
                    ui.label(RichText::new("·").size(10.0).color(FAINT));
                    ui.label(
                        RichText::new(format!("127.0.0.1:{}", self.proxy_port))
                            .font(mono(10.0))
                            .color(FAINT),
                    );
                    ui.label(RichText::new("·").size(10.0).color(FAINT));
                    ui.label(
                        RichText::new(if self.ca_info.is_some() {
                            "HTTPS certificate ready · every tool unlocked"
                        } else {
                            "Certificate not ready"
                        })
                        .size(10.0)
                        .color(FAINT),
                    );
                    if self.mac_proxy_on {
                        ui.label(RichText::new("·").size(10.0).color(FAINT));
                        dot(ui, AMBER, 6.0);
                        ui.label(
                            RichText::new("Mac proxy on · restored when Proxzy quits")
                                .size(10.0)
                                .color(AMBER),
                        );
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Restore now").size(10.0).color(AMBER),
                                )
                                .frame(false),
                            )
                            .on_hover_text(
                                "Point the macOS web proxy back at Direct without quitting",
                            )
                            .clicked()
                        {
                            if self.restore_mac_proxy_for_exit() {
                                self.log(
                                    Level::Info,
                                    "Proxy",
                                    "Restored the macOS web proxy from the status bar",
                                );
                                self.toast_now(ctx, "macOS web proxy turned off");
                            } else {
                                self.toast_now(
                                    ctx,
                                    "Could not restore the proxy; use Clear Mac proxy on Live Capture",
                                );
                            }
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("Rust + egui POC")
                                .size(10.0)
                                .color(FAINT),
                        );
                        ui.label(RichText::new("·").size(10.0).color(FAINT));
                        ui.label(
                            RichText::new(format!("{} requests", self.captures.len()))
                                .size(10.0)
                                .color(MUTED),
                        );
                        ui.label(RichText::new("·").size(10.0).color(FAINT));
                        ui.label(
                            RichText::new(format!(
                                "{} errors",
                                self.captures.iter().filter(|c| c.is_error()).count()
                            ))
                            .size(10.0)
                            .color(if self.captures.iter().any(|c| c.is_error()) {
                                AMBER
                            } else {
                                FAINT
                            }),
                        );
                    });
                });
            });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        // A solid scrollbar is part of the short-window cue: the macOS-style
        // floating bar is nearly invisible until the pointer is already there.
        subtle_scroll(ui);
        // The footer stays outside the scroll region, so "All features" is
        // reachable even when a short window hides the bottom of the rail.
        // When the rail is clipped, the footer grows by one short line that
        // says so; without it a floating scrollbar is too easy to miss.
        let more_id = Id::new("sidebar-more");
        let jump_id = Id::new("sidebar-jump-bottom");
        let show_more = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(more_id).unwrap_or(false));
        let footer_h = if show_more { 76.0 } else { 46.0 };
        let content_h = (ui.available_height() - footer_h).max(120.0);
        let mut overflow = false;
        let jump = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(jump_id).unwrap_or(false));
        if jump {
            ui.ctx().data_mut(|d| d.remove_temp::<bool>(jump_id));
        }
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), content_h),
            Layout::top_down(Align::Min),
            |ui| {
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("sidebar-scroll")
                    .max_height(content_h)
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                    );
                if jump {
                    scroll = scroll.vertical_scroll_offset(f32::MAX);
                }
                let output = scroll.show(ui, |ui| {
                    self.sidebar_inner(ui);
                });
                let viewport = output.inner_rect;
                let at_bottom = output.state.offset.y
                    >= (output.content_size.y - viewport.height() - 2.0).max(0.0);
                overflow =
                    output.content_size.y > viewport.height() + 2.0 && !at_bottom;
            },
        );
        ui.ctx()
            .data_mut(|d| d.insert_temp(more_id, overflow));

        ui.add_space(2.0);
        hsep(ui);
        ui.add_space(1.0);
        if overflow {
            let (row, row_resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), 14.0),
                Sense::click(),
            );
            if row_resp.clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(jump_id, true));
            }
            let p = ui.painter();
            let c = egui::pos2(row.left() + 7.0, row.center().y);
            p.line_segment(
                [egui::pos2(c.x - 3.0, c.y - 1.5), egui::pos2(c.x, c.y + 1.5)],
                Stroke::new(1.2, MUTED),
            );
            p.line_segment(
                [egui::pos2(c.x, c.y + 1.5), egui::pos2(c.x + 3.0, c.y - 1.5)],
                Stroke::new(1.2, MUTED),
            );
            p.text(
                egui::pos2(row.left() + 16.0, row.center().y),
                Align2::LEFT_CENTER,
                "More screens below",
                FontId::proportional(9.2),
                if row_resp.hovered() { TEXT } else { MUTED },
            );
            p.text(
                egui::pos2(row.right(), row.center().y),
                Align2::RIGHT_CENTER,
                "click to scroll",
                FontId::proportional(8.4),
                FAINT,
            );
            if row_resp
                .on_hover_text("Scroll to the bottom of the rail")
                .hovered()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.add_space(2.0);
        } else {
            ui.add_space(2.0);
        }
        if all_features_button(ui).clicked() {
            self.open_palette();
        }
    }

    fn sidebar_inner(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            draw_mark(ui, 30.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Proxzy").size(17.0).strong().color(TEXT));
                ui.label(
                    RichText::new("POC WORKSPACE")
                        .size(8.5)
                        .color(FAINT),
                );
            });
        });
        ui.add_space(10.0);
        section_label(ui, "Start here");
        ui.add_space(3.0);
        let tutorial = nav_row(
            ui,
            NavIcon::Compass,
            "Tutorial",
            None,
            self.view == View::Tutorial,
        )
        .on_hover_text("A guided tour of every main feature, including Agent Control");
        if tutorial.clicked() {
            self.tour = None;
            self.view = View::Tutorial;
        }

        ui.add_space(9.0);
        section_label(ui, "Traffic");
        ui.add_space(3.0);
        let counts = [
            (TrafficFilter::All, "All Traffic", NavIcon::Traffic),
            (TrafficFilter::Live, "Live", NavIcon::Live),
            (TrafficFilter::Pinned, "Pinned", NavIcon::Pin),
            (TrafficFilter::Graphql, "GraphQL", NavIcon::Graph),
            (TrafficFilter::Rest, "REST API", NavIcon::Rest),
            (TrafficFilter::Errors, "Errors", NavIcon::Error),
            (TrafficFilter::Mocked, "Mocked", NavIcon::Map),
            (TrafficFilter::Tunnels, "Tunnels", NavIcon::Shield),
        ];
        for (filter, label, icon) in counts {
            let selected = self.view == View::Traffic && self.filter == filter;
            let resp = nav_row(
                ui,
                icon,
                label,
                Some(self.count_of(filter)),
                selected,
            );
            if resp.clicked() {
                self.view = View::Traffic;
                self.filter = filter;
            }
        }

        // Logs sit above the utilities: this is the screen that explains what
        // happened in the session, not a hidden extra at the bottom of Tools.
        ui.add_space(9.0);
        section_label(ui, "Monitor");
        ui.add_space(3.0);
        let logs = nav_row(
            ui,
            NavIcon::Logs,
            "Log Dashboard",
            Some(self.logs.len()),
            self.view == View::Logs,
        )
        .on_hover_text("Timeline and dashboard: traffic, mocks, breakpoints and system events");
        if logs.clicked() {
            self.view = View::Logs;
        }

        // Automation sits above Tools deliberately: Agent Control is a main
        // feature, not an appendix, and it was easy to miss at the bottom of
        // a short window. The tour also points here.
        ui.add_space(9.0);
        section_label(ui, "Automation & agents");
        ui.add_space(3.0);
        let automation = [
            (
                View::Agent,
                "Agent Control",
                NavIcon::Agent,
                Some(self.map_rules.iter().filter(|r| r.enabled).count()),
            ),
            (
                View::Acp,
                "ACP Bridge",
                NavIcon::Bridge,
                None,
            ),
            (
                View::Tests,
                "API Test Bench",
                NavIcon::Tests,
                Some(self.tests.len()),
            ),
            (
                View::Screen,
                "Screen Play & MCP",
                NavIcon::Play,
                Some(self.screen_plays.len()),
            ),
        ];
        for (view, label, icon, count) in automation {
            let resp = nav_row(ui, icon, label, count, self.view == view);
            if resp.clicked() {
                self.view = view;
            }
        }

        ui.add_space(8.0);
        section_label(ui, "Tools");
        ui.add_space(3.0);
        let tools = [
            (
                View::Live,
                "Live Capture",
                NavIcon::Live,
                if self.proxy.is_some() { Some(1) } else { None },
            ),
            (View::Composer, "Composer", NavIcon::Composer, None),
            (
                View::MapLocal,
                "Map Local",
                NavIcon::Map,
                Some(self.map_rules.iter().filter(|r| r.enabled).count()),
            ),
            (
                View::Breakpoints,
                "Breakpoints",
                NavIcon::Breakpoint,
                Some(self.breakpoints.iter().filter(|b| b.enabled).count()),
            ),
            (View::Guide, "Guide & Stack", NavIcon::Help, None),
        ];
        for (view, label, icon, count) in tools {
            let resp = nav_row(ui, icon, label, count, self.view == view);
            if resp.clicked() {
                self.view = view;
            }
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new("Proxzy POC 0.1 · local only")
                .size(9.0)
                .color(FAINT),
        );
    }

    /// The thin strip above the traffic table. It replaces the old dead-end
    /// "these are samples" banner with something the person can press, and it
    /// narrates the practice run while the real calls arrive.
    fn practice_strip(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(run) = &self.practice else {
            let live = self.captures.iter().filter(|c| c.is_live()).count();
            if self.captures.is_empty() || live > 0 {
                return;
            }
            let mut start = false;
            let mut go_live = false;
            let mut start_tour = false;
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 10))
                .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 181, 84, 55)))
                .rounding(8.0)
                .inner_margin(Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        dot(ui, AMBER, 6.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.label(
                                RichText::new(
                                    "These rows are samples — no device has sent anything yet.",
                                )
                                .size(10.5)
                                .color(TEXT),
                            );
                            ui.label(
                                RichText::new(
                                    "Five real calls: one Map Local answer and one paused breakpoint, all on this Mac.",
                                )
                                .size(9.8)
                                .color(MUTED),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, "Run practice session").clicked() {
                                start = true;
                            }
                            if secondary_button(ui, "Open Live Capture").clicked() {
                                go_live = true;
                            }
                            if secondary_button(ui, "Start tutorial").clicked() {
                                start_tour = true;
                            }
                        });
                    });
                });
            ui.add_space(8.0);
            if start {
                self.start_practice_session(ctx);
            } else if go_live {
                self.view = View::Live;
            } else if start_tour {
                self.begin_tour(0);
            }
            return;
        };

        let done = run.done;
        let total = run.total;
        let statuses = run.statuses.clone();
        let error = run.error.clone();
        let answered = statuses.iter().filter(|s| s.is_some()).count();
        let paused_at_breakpoint = self
            .breakpoint_session
            .as_ref()
            .map(|s| s.live_bp_id.is_some())
            .unwrap_or(false);
        let (fill, stroke) = if done {
            (
                Color32::from_rgba_unmultiplied(61, 214, 149, 10),
                Color32::from_rgba_unmultiplied(61, 214, 149, 55),
            )
        } else {
            (
                Color32::from_rgba_unmultiplied(88, 140, 255, 10),
                Color32::from_rgba_unmultiplied(88, 140, 255, 60),
            )
        };
        let mut hide = false;
        let mut again = false;
        let mut open_logs = false;
        Frame::none()
            .fill(fill)
            .stroke(Stroke::new(1.0, stroke))
            .rounding(8.0)
            .inner_margin(Margin::symmetric(10.0, 7.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(ui, if done { GREEN } else { ACCENT }, 6.0);
                    ui.label(
                        RichText::new(if done {
                            "Practice session complete"
                        } else {
                            "Practice session running"
                        })
                        .size(10.8)
                        .strong()
                        .color(TEXT),
                    );
                    ui.label(
                        RichText::new(format!(
                            "· {} of {} calls answered · all local, nothing leaves this Mac",
                            answered, total
                        ))
                        .size(10.0)
                        .color(MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, "Hide").clicked() {
                            hide = true;
                        }
                        if secondary_button(ui, "Open logs").clicked() {
                            open_logs = true;
                        }
                        if done && secondary_button(ui, "Run again").clicked() {
                            again = true;
                        }
                    });
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    for (index, (label, _, _, _)) in PRACTICE_STEPS.iter().enumerate() {
                        let status = statuses.get(index).and_then(|s| *s);
                        let (text, color) = match status {
                            Some(0) => (format!("{} failed", label), RED),
                            Some(code) => (format!("{} {}", label, code), GREEN),
                            None if paused_at_breakpoint && index == 3 => {
                                (format!("{} paused", label), AMBER)
                            }
                            None => (format!("{} …", label), FAINT),
                        };
                        pill(
                            ui,
                            &text,
                            color,
                            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 16),
                        );
                    }
                });
                let progress = if total == 0 {
                    0.0
                } else {
                    answered as f32 / total as f32
                };
                ui.add_space(4.0);
                let (bar, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 3.0),
                    Sense::hover(),
                );
                ui.painter().rect_filled(bar, 2.0, CARD);
                let mut filled = bar;
                filled.set_width(bar.width() * progress.clamp(0.0, 1.0));
                ui.painter()
                    .rect_filled(filled, 2.0, if done { GREEN } else { ACCENT });
                if paused_at_breakpoint {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "A call is paused by the breakpoint editor — press Continue there to release it.",
                        )
                        .size(9.5)
                        .color(AMBER),
                    );
                } else if let Some(message) = &error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(message).size(9.5).color(RED));
                } else if done {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "One call was answered by Map Local and one was paused by a breakpoint. Try the Mocked filter, or open Map Local and Log Dashboard to see the same events.",
                        )
                        .size(9.5)
                        .color(MUTED),
                    );
                }
            });
        ui.add_space(8.0);
        if hide {
            self.practice = None;
        } else if again {
            self.start_practice_session(ctx);
        } else if open_logs {
            self.view = View::Logs;
        }
    }

    fn traffic(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new(self.traffic_title())
                        .size(24.0)
                        .strong()
                        .color(TEXT),
                );
                let shown = self.filtered_indices().len();
                ui.label(
                    RichText::new(format!(
                        "{} of {} requests · {}",
                        shown,
                        self.captures.len(),
                        self.session_mix_label()
                    ))
                    .size(11.5)
                    .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let follow = chip(
                    ui,
                    if self.follow_latest {
                        "● Following live"
                    } else {
                        "Follow live"
                    },
                    self.follow_latest,
                )
                .on_hover_text(if self.follow_latest {
                    "New captures become the selection. Click a row to pin it."
                } else {
                    "Selection is pinned. Click to jump back to the newest capture."
                });
                if follow.clicked() {
                    self.follow_latest = !self.follow_latest;
                    if self.follow_latest {
                        self.selected = 0;
                    }
                }
                ui.add_space(4.0);
                if secondary_button(
                    ui,
                    if self.sort_desc {
                        "Newest first"
                    } else {
                        "Oldest first"
                    },
                )
                .clicked()
                {
                    self.sort_desc = !self.sort_desc;
                }
                ui.add_space(2.0);
                if secondary_button(ui, "Clear history").clicked() {
                    self.captures.clear();
                    self.selected = 0;
                    self.follow_latest = true;
                }
            });
        });
        ui.add_space(10.0);

        let live_count = self.captures.iter().filter(|c| c.is_live()).count();
        self.practice_strip(ui, ctx);

        ui.horizontal(|ui| {
            for (filter, label) in [
                (TrafficFilter::All, "All"),
                (TrafficFilter::Live, "Live"),
                (TrafficFilter::Pinned, "Pinned"),
                (TrafficFilter::Rest, "REST"),
                (TrafficFilter::Graphql, "GraphQL"),
                (TrafficFilter::Errors, "Errors"),
                (TrafficFilter::Mocked, "Mocked"),
                (TrafficFilter::Tunnels, "Tunnels"),
            ] {
                if chip(ui, label, self.filter == filter).clicked() {
                    self.filter = filter;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (hint, color) = if self.follow_latest {
                    ("click a row to pin it", FAINT)
                } else {
                    ("selection pinned · new calls won't change it", AMBER)
                };
                ui.label(RichText::new(hint).size(10.0).color(color));
            });
        });
        ui.add_space(8.0);

        let table_width = ui.available_width();
        let header_h = 26.0;
        let (header_rect, _) =
            ui.allocate_exact_size(Vec2::new(table_width, header_h), Sense::hover());
        table_header(ui.painter(), header_rect);

        let indices = self.filtered_indices();
        if indices.is_empty() {
            ui.add_space(50.0);
            ui.vertical_centered(|ui| {
                if self.captures.is_empty() {
                    ui.label(RichText::new("No traffic has reached the gate yet").size(15.0).color(TEXT));
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(if self.proxy.is_some() {
                            "The proxy is listening, but nothing has used it. A simulator uses this Mac's proxy route, so it must be switched on too."
                        } else {
                            "Start the proxy first, then point the simulator or app at this Mac."
                        })
                        .size(11.5)
                        .color(MUTED),
                    );
                    ui.add_space(8.0);
                    if primary_button(ui, "Run capture check").clicked() {
                        self.start_capture_check(ctx);
                    }
                    if secondary_button(ui, "Run practice session").clicked() {
                        self.start_practice_session(ctx);
                    }
                    if secondary_button(ui, "Open Live Capture").clicked() {
                        self.view = View::Live;
                    }
                } else if self.filter == TrafficFilter::Live && live_count == 0 {
                    ui.label(RichText::new("No live calls yet").size(15.0).color(TEXT));
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!(
                            "The {} rows under All Traffic are sample rows for learning — no device has sent anything. Press Run capture check to send one real request, or run the practice session to see capture, mock and breakpoint work.",
                            self.captures.len()
                        ))
                        .size(11.5)
                        .color(MUTED),
                    );
                    ui.add_space(8.0);
                    if primary_button(ui, "Run practice session").clicked() {
                        self.start_practice_session(ctx);
                    }
                    if secondary_button(ui, "Show sample session").clicked() {
                        self.filter = TrafficFilter::All;
                        self.search.clear();
                    }
                    if secondary_button(ui, "Run capture check").clicked() {
                        self.start_capture_check(ctx);
                    }
                } else {
                    ui.label(RichText::new("No requests here").size(15.0).color(TEXT));
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Try another filter, clear the search, or capture again.")
                            .size(11.5)
                            .color(MUTED),
                    );
                    ui.add_space(8.0);
                    if secondary_button(ui, "Show all traffic").clicked() {
                        self.filter = TrafficFilter::All;
                        self.search.clear();
                    }
                    if secondary_button(ui, "Run practice session").clicked() {
                        self.start_practice_session(ctx);
                    }
                }
            });
            return;
        }

        let row_h = 34.0;
        let max_h = (ui.available_height() - 6.0).max(80.0);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(max_h)
            .show(ui, |ui| {
                for (row_no, idx) in indices.iter().enumerate() {
                    let capture = &self.captures[*idx];
                    let resp = request_row(
                        ui,
                        capture,
                        *idx == self.selected,
                        row_no % 2 == 1,
                        row_h,
                    );
                    if resp.clicked() {
                        self.selected = *idx;
                        self.follow_latest = false;
                        self.inspector_tab = InspectorTab::Overview;
                    }
                }
                ui.add_space(4.0);
            });
        let _ = ctx;
    }

    fn inspector(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(c) = self.captures.get(self.selected).cloned() else {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(RichText::new("Nothing selected").size(15.0).color(TEXT));
                ui.label(
                    RichText::new("Pick a request from the list to see it here.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            return;
        };

        let mut action: Option<Action> = None;

        ui.horizontal(|ui| {
            method_badge(ui, &c.method);
            wave_badge(ui, c.status);
            ui.label(
                RichText::new(status_text(c.status))
                    .size(11.5)
                    .color(status_color(c.status)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} ms", c.latency_ms))
                        .font(mono(11.0))
                        .color(MUTED),
                );
                ui.label(RichText::new("·").size(10.0).color(FAINT));
                ui.label(
                    RichText::new(c.started_clock())
                        .font(mono(10.5))
                        .color(FAINT),
                );
            });
        });
        ui.add_space(8.0);

        Frame::none()
            .fill(INPUT)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(8.0)
            .inner_margin(Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(&c.url)
                            .font(mono(11.0))
                            .color(Color32::from_rgb(196, 208, 228)),
                    )
                    .selectable(true)
                    .wrap(),
                );
            });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let w = ((ui.available_width() - 24.0) / 5.0).max(58.0);
            if inspector_action(ui, "Copy URL", w, false).clicked() {
                action = Some(Action::CopyUrl);
            }
            if inspector_action(ui, "Copy cURL", w, false).clicked() {
                action = Some(Action::CopyCurl);
            }
            if inspector_action(ui, "Share", w, false).clicked() {
                action = Some(Action::Share);
            }
            if inspector_action(ui, if c.pinned { "Pinned" } else { "Pin" }, w, false).clicked() {
                action = Some(Action::Pin);
            }
            if inspector_action(ui, "Repeat", w, true).clicked() {
                action = Some(Action::Repeat);
            }
        });
        ui.add_space(4.0);

        // One-click automation for the request in front of you. If a rule
        // already covers this URL, the button edits that rule instead of
        // creating a near-duplicate.
        let map_match = self
            .map_rules
            .iter()
            .find(|r| r.enabled && r.matches(&c.method, &c.url, &c.req_body))
            .cloned();
        let bp_match = self
            .breakpoints
            .iter()
            .find(|r| r.enabled && r.matches(&c.method, &c.url, &c.req_body))
            .cloned();
        ui.horizontal(|ui| {
            let w = ((ui.available_width() - 8.0) / 2.0).max(90.0);
            let mock_label = if map_match.is_some() { "Edit mock" } else { "Mock this" };
            let catch_label = if bp_match.is_some() { "Edit breakpoint" } else { "Catch this" };
            if inspector_action(ui, mock_label, w, false).clicked() {
                action = Some(Action::MockThis);
            }
            if inspector_action(ui, catch_label, w, false).clicked() {
                action = Some(Action::CatchThis);
            }
        });
        if map_match.is_some() || bp_match.is_some() {
            ui.horizontal_wrapped(|ui| {
                if let Some(rule) = &map_match {
                    pill(
                        ui,
                        &format!("MAP LOCAL · {}", rule.name),
                        GREEN,
                        Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                    );
                }
                if let Some(rule) = &bp_match {
                    pill(
                        ui,
                        &format!("BREAKPOINT · {}", rule.name),
                        AMBER,
                        Color32::from_rgba_unmultiplied(255, 181, 84, 18),
                    );
                }
                ui.label(
                    RichText::new("matches the selected request")
                        .size(9.5)
                        .color(FAINT),
                );
            });
        }
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            for (tab, label, width) in [
                (InspectorTab::Overview, "Overview", 72.0),
                (InspectorTab::Request, "Request", 64.0),
                (InspectorTab::Response, "Response", 74.0),
                (InspectorTab::Headers, "Headers", 66.0),
                (InspectorTab::Curl, "cURL", 52.0),
            ] {
                if inspector_tab(ui, label, width, self.inspector_tab == tab).clicked() {
                    self.inspector_tab = tab;
                }
            }
        });
        ui.add_space(2.0);
        hsep(ui);

        let content_h = (ui.available_height() - 4.0).max(100.0);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(content_h)
            .show(ui, |ui| {
                ui.add_space(10.0);
                match self.inspector_tab {
                    InspectorTab::Overview => self.inspector_overview(ui, &c),
                    InspectorTab::Request => self.inspector_request(ui, &c, &mut action),
                    InspectorTab::Response => self.inspector_response(ui, &c, &mut action),
                    InspectorTab::Headers => inspector_headers(ui, &c),
                    InspectorTab::Curl => self.inspector_curl(ui, &c, &mut action),
                }
                ui.add_space(12.0);
            });

        match action {
            Some(Action::CopyUrl) => {
                self.copy(ctx, c.url.clone(), "URL copied to clipboard");
            }
            Some(Action::CopyCurl) => {
                self.copy(ctx, c.curl(), "cURL command copied to clipboard");
            }
            Some(Action::CopyBody) => {
                if c.req_body.is_empty() {
                    self.toast_now(ctx, "This request has no body to copy");
                } else {
                    self.copy(ctx, c.req_body.clone(), "Request body copied");
                }
            }
            Some(Action::CopyResponse) => {
                if c.resp_body.is_empty() {
                    self.toast_now(ctx, "This response has no body to copy");
                } else {
                    self.copy(ctx, c.resp_body.clone(), "Response body copied");
                }
            }
            Some(Action::Share) => {
                self.share_open = true;
            }
            Some(Action::Repeat) => {
                self.repeat_selected(ctx);
            }
            Some(Action::Pin) => {
                if let Some(row) = self.captures.get_mut(self.selected) {
                    row.pinned = !row.pinned;
                    let pinned = row.pinned;
                    let host_path = row.host_path();
                    let message = if pinned {
                        format!("Pinned {}", host_path)
                    } else {
                        format!("Unpinned {}", host_path)
                    };
                    self.log(Level::Info, "Traffic", message);
                    self.toast_now(
                        ctx,
                        if pinned {
                            "Pinned — find it under Pinned in the sidebar"
                        } else {
                            "Removed from pinned requests"
                        },
                    );
                }
            }
            Some(Action::MockThis) => match map_match {
                Some(rule) => self.open_map_editor(Some(rule)),
                None => self.open_map_editor_for_capture(&c),
            },
            Some(Action::CatchThis) => match bp_match {
                Some(rule) => self.open_bp_editor(Some(rule)),
                None => self.open_bp_editor_for_capture(&c),
            },
            None => {}
        }
    }

    fn inspector_overview(&mut self, ui: &mut Ui, c: &Capture) {
        let w = (ui.available_width() - 16.0) / 3.0;
        ui.horizontal(|ui| {
            stat_box(ui, w, "LATENCY", &format!("{} ms", c.latency_ms), MUTED);
            stat_box(ui, w, "SIZE", &c.size_label(), MUTED);
            stat_box(
                ui,
                w,
                "STATUS",
                status_text(c.status),
                status_color(c.status),
            );
        });
        ui.add_space(12.0);
        section_label(ui, "General");
        ui.add_space(5.0);
        Frame::none()
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(9.0)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                kv(ui, "Request URL", &c.url);
                kv(ui, "Method", &c.method);
                kv(ui, "Remote address", &c.remote);
                kv(ui, "Protocol", &c.protocol);
                kv(ui, "Type", c.kind.label());
                kv(
                    ui,
                    "Content-Type",
                    &header_value(&c.resp_headers, "content-type")
                        .unwrap_or_else(|| "application/json".into()),
                );
                if !c.req_body.is_empty() {
                    kv(ui, "Request size", &format!("{} B", c.req_body.len()));
                }
            });

        if c.is_tunnel() {
            ui.add_space(12.0);
            let is_protected = c.is_system_protected();
            let banner_color = if is_protected { GREEN } else { CYAN };
            let host_to_enable = c.host();
            let mut do_enable = false;
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(
                    banner_color.r(),
                    banner_color.g(),
                    banner_color.b(),
                    16,
                ))
                .stroke(Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        banner_color.r(),
                        banner_color.g(),
                        banner_color.b(),
                        85,
                    ),
                ))
                .rounding(9.0)
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        dot(ui, banner_color, 7.0);
                        ui.label(
                            RichText::new(if is_protected {
                                "🛡️ Protected System Domain (Raw TCP Data Bound)"
                            } else {
                                "🔒 Raw TCP Tunnel (TLS Decryption Bypassed)"
                            })
                            .size(12.0)
                            .strong()
                            .color(banner_color),
                        );
                        if !is_protected {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if primary_button(ui, "Enable TLS Decryption").clicked() {
                                    do_enable = true;
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(if is_protected {
                            "This domain matches ProxzyCraft's Open-Source Safety Policy (Apple APNS / OCSP / OS Update). Raw TCP data is safely bound without TLS decryption to preserve host OS and security certificate integrity."
                        } else {
                            "This HTTPS connection was bound as raw TCP data because the domain is not in your SSL Allowlist. Click 'Enable TLS Decryption' to decrypt headers and JSON payloads on subsequent requests."
                        })
                        .size(10.5)
                        .color(MUTED),
                    );
                });
            if do_enable {
                self.enable_ssl_for_domain(&host_to_enable, ui.ctx());
            }
        }

        if let Some(rule) = &c.mock {
            ui.add_space(12.0);
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 14))
                .stroke(Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(255, 181, 84, 85),
                ))
                .rounding(9.0)
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        dot(ui, AMBER, 7.0);
                        ui.label(
                            RichText::new("Mapped locally")
                                .size(12.0)
                                .strong()
                                .color(AMBER),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if secondary_button(ui, "View rule").clicked() {
                                self.view = View::MapLocal;
                            }
                        });
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("Rule: {}", rule))
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new("This response was served by Map Local and never reached the server.")
                            .size(10.5)
                            .color(FAINT),
                    );
                });
        }

        if let Some(g) = &c.graphql {
            ui.add_space(12.0);
            section_label(ui, "GraphQL operation");
            ui.add_space(5.0);
            Frame::none()
                .fill(CARD)
                .stroke(Stroke::new(1.0, BORDER))
                .rounding(9.0)
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        pill(
                            ui,
                            &g.op_type.to_uppercase(),
                            PURPLE,
                            Color32::from_rgba_unmultiplied(184, 140, 255, 24),
                        );
                        ui.label(
                            RichText::new(&g.operation)
                                .size(14.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new("variables")
                                    .size(10.0)
                                    .color(FAINT),
                            );
                        });
                    });
                    ui.add_space(6.0);
                    code_block(ui, &g.variables, 110.0, true);
                });
        }

        ui.add_space(12.0);
        section_label(ui, "Started");
        ui.add_space(5.0);
        Frame::none()
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(9.0)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Captured at")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new(&c.started)
                            .font(mono(11.5))
                            .color(TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        pill(ui, "DEMO SESSION", FAINT, INPUT);
                    });
                });
            });
    }

    fn inspector_request(&mut self, ui: &mut Ui, c: &Capture, action: &mut Option<Action>) {
        section_label(ui, "Request headers");
        ui.add_space(5.0);
        header_table(ui, &c.req_headers);
        ui.add_space(12.0);
        section_label(ui, "Request body");
        ui.add_space(5.0);
        if c.req_body.is_empty() {
            empty_body(ui, "No request body");
        } else {
            let is_json = c.req_body.trim_start().starts_with('{');
            code_block(ui, &c.req_body, 300.0, is_json);
        }
        if let Some(g) = &c.graphql {
            ui.add_space(12.0);
            section_label(ui, "GraphQL variables");
            ui.add_space(5.0);
            code_block(ui, &g.variables, 130.0, true);
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if secondary_button(ui, "Copy body").clicked() {
                *action = Some(Action::CopyBody);
            }
            if secondary_button(ui, "Copy cURL").clicked() {
                *action = Some(Action::CopyCurl);
            }
        });
    }

    fn inspector_response(&mut self, ui: &mut Ui, c: &Capture, action: &mut Option<Action>) {
        ui.horizontal(|ui| {
            wave_badge(ui, c.status);
            ui.label(
                RichText::new(status_text(c.status))
                    .size(11.5)
                    .color(status_color(c.status)),
            );
            ui.label(
                RichText::new(format!("· {} · {}", c.size_label(), c.latency_ms))
                    .size(10.5)
                    .color(FAINT),
            );
        });
        ui.add_space(10.0);
        section_label(ui, "Response headers");
        ui.add_space(5.0);
        header_table(ui, &c.resp_headers);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            section_label(ui, "Response body");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for format in [
                    ResponseFormat::Html,
                    ResponseFormat::Xml,
                    ResponseFormat::Raw,
                    ResponseFormat::Json,
                ] {
                    if chip(ui, format.label(), self.response_format == format)
                        .on_hover_text(format.hint())
                        .clicked()
                    {
                        self.response_format = format;
                    }
                }
            });
        });
        ui.add_space(5.0);
        if c.resp_body.is_empty() {
            empty_body(ui, "Empty response body");
        } else {
            draw_formatted_body(ui, &c.resp_body, self.response_format, 380.0);
        }
        ui.add_space(10.0);
        if secondary_button(ui, "Copy response").clicked() {
            *action = Some(Action::CopyResponse);
        }
        let _ = action;
    }

    fn inspector_curl(&mut self, ui: &mut Ui, c: &Capture, action: &mut Option<Action>) {
        section_label(ui, "Shareable cURL");
        ui.add_space(5.0);
        code_block(ui, &c.curl(), 260.0, false);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if primary_button(ui, "Copy cURL").clicked() {
                *action = Some(Action::CopyCurl);
            }
            if secondary_button(ui, "Copy URL").clicked() {
                *action = Some(Action::CopyUrl);
            }
            if secondary_button(ui, "Share link").clicked() {
                *action = Some(Action::Share);
            }
        });
        ui.add_space(12.0);
        section_label(ui, "Share link");
        ui.add_space(5.0);
        Frame::none()
            .fill(INPUT)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(8.0)
            .inner_margin(Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(c.share_link())
                        .font(mono(11.0))
                        .color(Color32::from_rgb(154, 183, 235)),
                );
            });
        ui.add_space(6.0);
        ui.label(
            RichText::new("Share links are local placeholders in this POC.")
                .size(10.0)
                .color(FAINT),
        );
    }
}

/// Render a response body in the chosen display format. JSON is re-indented by
/// default; XML/HTML get markup highlighting; Raw stays byte-for-byte. When a
/// format does not apply, the body is shown unchanged with a short amber note.
fn draw_formatted_body(ui: &mut Ui, body: &str, format: ResponseFormat, max_height: f32) {
    let (shown, view, note) = match format {
        ResponseFormat::Json => match json_pretty(body) {
            Some(pretty) => (pretty, BodyView::Json, None),
            None => (
                body.to_string(),
                BodyView::Raw,
                Some("This body is not a JSON object or array; showing it unchanged."),
            ),
        },
        ResponseFormat::Raw => (body.to_string(), BodyView::Raw, None),
        ResponseFormat::Xml => match markup_pretty(body) {
            Some(pretty) => (pretty, BodyView::Markup, None),
            None => (
                body.to_string(),
                BodyView::Raw,
                Some("No XML markup was found; showing the body unchanged."),
            ),
        },
        ResponseFormat::Html => match markup_pretty(body) {
            Some(pretty) => (pretty, BodyView::Markup, None),
            None => (
                body.to_string(),
                BodyView::Raw,
                Some("No HTML markup was found; showing the body unchanged."),
            ),
        },
    };
    if let Some(note) = note {
        ui.label(RichText::new(note).size(10.0).color(AMBER));
        ui.add_space(4.0);
    }
    match view {
        BodyView::Json => code_block(ui, &shown, max_height, true),
        BodyView::Markup => markup_code_block(ui, &shown, max_height),
        BodyView::Raw => code_block(ui, &shown, max_height, false),
    }
}

/// Pick the row that the detail pane should show after the capture list
/// changes. Following live always means the newest row; a pinned selection is
/// looked up by id so inserts above it and sample cleanup do not move it.
fn resolve_selection(follow_latest: bool, keep_id: Option<u64>, captures: &[Capture]) -> usize {
    if follow_latest {
        0
    } else {
        keep_id
            .and_then(|id| captures.iter().position(|row| row.id == id))
            .unwrap_or(0)
    }
}

fn request_row(ui: &mut Ui, c: &Capture, selected: bool, alt: bool, height: f32) -> egui::Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            rect,
            6.0,
            Color32::from_rgba_unmultiplied(88, 140, 255, 30),
        );
        painter.rect_filled(
            Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())),
            1.5,
            ACCENT,
        );
    } else if resp.hovered() {
        painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(255, 255, 255, 8));
    } else if alt {
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 3),
        );
    }

    let cy = rect.center().y;
    let right = rect.right();
    let method_x = rect.left() + 14.0;
    let status_x = rect.left() + 74.0;
    let url_x = rect.left() + 126.0;
    let type_x = right - 198.0;
    let time_right = right - 74.0;
    let size_right = right - 14.0;

    // Method badge
    let mcolor = method_color(&c.method);
    let badge = Rect::from_min_size(
        egui::pos2(method_x, cy - 9.5),
        Vec2::new(46.0, 19.0),
    );
    painter.rect_filled(
        badge,
        5.0,
        Color32::from_rgba_unmultiplied(mcolor.r(), mcolor.g(), mcolor.b(), 20),
    );
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        &c.method,
        FontId::monospace(9.5),
        mcolor,
    );

    // Status
    painter.circle_filled(
        egui::pos2(status_x + 3.0, cy),
        2.6,
        status_color(c.status),
    );
    painter.text(
        egui::pos2(status_x + 10.0, cy),
        Align2::LEFT_CENTER,
        if c.status == 0 {
            "ERR".to_string()
        } else {
            c.status.to_string()
        },
        FontId::monospace(10.5),
        status_color(c.status),
    );

    // URL: host muted + path bright
    if url_x < type_x - 12.0 {
        let mut job = egui::text::LayoutJob::default();
        let host = c.host();
        let path = c.path();
        job.append(
            &host,
            0.0,
            egui::TextFormat {
                font_id: FontId::proportional(11.5),
                color: if selected { Color32::from_rgb(158, 176, 205) } else { MUTED },
                ..Default::default()
            },
        );
        job.append(
            &path,
            0.0,
            egui::TextFormat {
                font_id: FontId::proportional(11.8),
                color: if selected { TEXT } else { Color32::from_rgb(210, 219, 233) },
                ..Default::default()
            },
        );
        if c.is_live() {
            job.append(
                "  LIVE",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(8.5),
                    color: GREEN,
                    ..Default::default()
                },
            );
        } else {
            job.append(
                "  SAMPLE",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(8.0),
                    color: FAINT,
                    ..Default::default()
                },
            );
        }
        if c.mock.is_some() {
            job.append(
                "  MOCK",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(8.5),
                    color: AMBER,
                    ..Default::default()
                },
            );
        }
        if c.is_tunnel() {
            if c.is_system_protected() {
                job.append(
                    "  PROTECTED",
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::monospace(8.5),
                        color: Color32::from_rgb(148, 163, 184),
                        ..Default::default()
                    },
                );
            } else {
                job.append(
                    "  RAW TUNNEL",
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::monospace(8.5),
                        color: CYAN,
                        ..Default::default()
                    },
                );
            }
        }
        if c.pinned {
            job.append(
                "  PIN",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(8.5),
                    color: PURPLE,
                    ..Default::default()
                },
            );
        }
        job.wrap.max_width = type_x - url_x - 14.0;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        let galley = ui.fonts(|f| f.layout_job(job));
        let p = painter.with_clip_rect(Rect::from_min_max(
            egui::pos2(url_x, rect.top()),
            egui::pos2(type_x - 8.0, rect.bottom()),
        ));
        p.galley(
            egui::pos2(url_x, cy - galley.size().y / 2.0),
            galley,
            TEXT,
        );
    }

    // Type
    let kc = c.kind.color();
    painter.circle_filled(egui::pos2(type_x + 3.0, cy), 2.4, kc);
    painter.text(
        egui::pos2(type_x + 10.0, cy),
        Align2::LEFT_CENTER,
        c.kind.label(),
        FontId::proportional(10.0),
        if selected { kc } else { Color32::from_rgb(120, 139, 165) },
    );

    painter.text(
        egui::pos2(time_right, cy),
        Align2::RIGHT_CENTER,
        format!("{} ms", c.latency_ms),
        FontId::monospace(10.0),
        MUTED,
    );
    painter.text(
        egui::pos2(size_right, cy),
        Align2::RIGHT_CENTER,
        c.size_label(),
        FontId::monospace(10.0),
        FAINT,
    );

    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn table_header(painter: &egui::Painter, rect: Rect) {
    let cy = rect.center().y;
    let right = rect.right();
    let color = FAINT;
    let font = FontId::proportional(9.0);
    painter.text(
        egui::pos2(rect.left() + 14.0, cy),
        Align2::LEFT_CENTER,
        "METHOD",
        font.clone(),
        color,
    );
    painter.text(
        egui::pos2(rect.left() + 74.0, cy),
        Align2::LEFT_CENTER,
        "STATUS",
        font.clone(),
        color,
    );
    painter.text(
        egui::pos2(rect.left() + 126.0, cy),
        Align2::LEFT_CENTER,
        "REQUEST",
        font.clone(),
        color,
    );
    painter.text(
        egui::pos2(right - 198.0, cy),
        Align2::LEFT_CENTER,
        "TYPE",
        font.clone(),
        color,
    );
    painter.text(
        egui::pos2(right - 74.0, cy),
        Align2::RIGHT_CENTER,
        "TIME",
        font.clone(),
        color,
    );
    painter.text(
        egui::pos2(right - 14.0, cy),
        Align2::RIGHT_CENTER,
        "SIZE",
        font,
        color,
    );
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, BORDER),
    );
}

fn nav_row(
    ui: &mut Ui,
    icon: NavIcon,
    label: &str,
    count: Option<usize>,
    selected: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 28.0),
        Sense::click(),
    );
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            rect,
            7.0,
            Color32::from_rgba_unmultiplied(88, 140, 255, 28),
        );
        painter.rect_filled(
            Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())),
            1.5,
            ACCENT,
        );
    } else if resp.hovered() {
        painter.rect_filled(rect, 7.0, HOVER);
    }
    let icon_rect =
        Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), Vec2::splat(18.0));
    paint_nav_icon(
        painter,
        icon_rect,
        icon,
        if selected {
            Color32::from_rgb(165, 195, 255)
        } else {
            MUTED
        },
    );
    painter.text(
        egui::pos2(rect.left() + 33.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        if selected { TEXT } else { MUTED },
    );
    if let Some(c) = count {
        painter.text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            c.to_string(),
            FontId::monospace(10.0),
            if selected { Color32::from_rgb(165, 195, 255) } else { FAINT },
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The pinned "All features" control at the bottom of the rail. It is always
/// visible, even when the scrollable list is clipped on a short window.
fn all_features_button(ui: &mut Ui) -> egui::Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
    let painter = ui.painter();
    painter.rect(
        rect,
        8.0,
        if resp.hovered() { HOVER } else { CARD },
        Stroke::new(1.0, BORDER),
    );
    // A small four-square "all screens" glyph, drawn rather than using a font.
    let origin = egui::pos2(rect.left() + 11.0, rect.center().y - 5.5);
    for (dx, dy) in [(0.0, 0.0), (6.0, 0.0), (0.0, 6.0), (6.0, 6.0)] {
        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(origin.x + dx, origin.y + dy),
                Vec2::splat(4.5),
            ),
            1.2,
            ACCENT,
        );
    }
    painter.text(
        egui::pos2(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        "All features",
        FontId::proportional(11.5),
        TEXT,
    );
    painter.text(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        Align2::RIGHT_CENTER,
        if cfg!(target_os = "macos") { "Cmd K" } else { "Ctrl K" },
        FontId::monospace(8.5),
        FAINT,
    );
    resp.on_hover_text("Every screen and action in one list")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One two-line row in the all-features palette. Painted directly so the
/// label and its explanation share the same click target.
fn palette_row(
    ui: &mut Ui,
    entry: &PaletteEntry,
    selected: bool,
    badge: Option<&str>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 46.0),
        Sense::click(),
    );
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            rect,
            8.0,
            Color32::from_rgba_unmultiplied(88, 140, 255, 28),
        );
        painter.rect_filled(
            Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())),
            1.5,
            ACCENT,
        );
    } else if resp.hovered() {
        painter.rect_filled(rect, 8.0, HOVER);
    }
    let icon_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        Vec2::splat(18.0),
    );
    paint_nav_icon(
        painter,
        icon_rect,
        entry.icon,
        if selected {
            Color32::from_rgb(165, 195, 255)
        } else {
            MUTED
        },
    );
    painter.text(
        egui::pos2(rect.left() + 38.0, rect.center().y - 7.0),
        Align2::LEFT_CENTER,
        entry.label,
        FontId::proportional(12.5),
        if selected {
            Color32::from_rgb(220, 232, 255)
        } else {
            TEXT
        },
    );
    painter.text(
        egui::pos2(rect.left() + 38.0, rect.center().y + 8.0),
        Align2::LEFT_CENTER,
        entry.hint,
        FontId::proportional(9.5),
        FAINT,
    );
    if let Some(badge) = badge {
        painter.text(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            badge,
            FontId::monospace(9.0),
            if selected { ACCENT } else { FAINT },
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn tool_chip(ui: &mut Ui, label: &str, count: Option<usize>, active: bool) -> egui::Response {
    let (fg, bg, stroke) = if active {
        (
            Color32::from_rgb(206, 222, 250),
            Color32::from_rgba_unmultiplied(88, 140, 255, 32),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(88, 140, 255, 110)),
        )
    } else {
        (MUTED, CARD, Stroke::new(1.0, BORDER))
    };
    Frame::none()
        .fill(bg)
        .stroke(stroke)
        .rounding(7.0)
        .inner_margin(Margin::symmetric(9.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                dot(ui, if active { ACCENT } else { FAINT }, 6.0);
                ui.label(RichText::new(label).size(11.0).color(fg));
                if let Some(c) = count {
                    ui.label(
                        RichText::new(c.to_string())
                            .font(mono(9.0))
                            .color(FAINT),
                    );
                }
            });
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn method_badge(ui: &mut Ui, method: &str) {
    let color = method_color(method);
    Frame::none()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            22,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70),
        ))
        .rounding(5.0)
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(method)
                    .font(mono(9.5))
                    .color(color)
                    .strong(),
            );
        });
}

fn inspector_action(ui: &mut Ui, label: &str, width: f32, primary: bool) -> egui::Response {
    let (fill, stroke, color) = if primary {
        (ACCENT, Stroke::NONE, Color32::WHITE)
    } else {
        (CARD, Stroke::new(1.0, BORDER), TEXT)
    };
    ui.add_sized(
        [width, 28.0],
        egui::Button::new(RichText::new(label).size(10.5).color(color))
            .fill(fill)
            .stroke(stroke),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn inspector_tab(ui: &mut Ui, label: &str, width: f32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left() + 5.0, rect.bottom() - 2.0),
                egui::pos2(rect.right() - 5.0, rect.bottom()),
            ),
            1.0,
            ACCENT,
        );
    }
    painter.text(
        rect.center() - Vec2::new(0.0, 1.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.2),
        if selected { TEXT } else { MUTED },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn stat_box(ui: &mut Ui, width: f32, label: &str, value: &str, color: Color32) {
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(8.0)
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_width((width - 22.0).max(40.0));
            ui.label(RichText::new(label).size(8.5).color(FAINT).strong());
            ui.label(RichText::new(value).size(14.0).strong().color(color));
        });
}

fn header_table(ui: &mut Ui, headers: &[(String, String)]) {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(9.0)
        .inner_margin(Margin::same(10.0))
        .show(ui, |ui| {
            if headers.is_empty() {
                ui.label(RichText::new("No headers").size(11.0).color(FAINT));
                return;
            }
            for (k, v) in headers {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    ui.label(
                        RichText::new(k)
                            .font(mono(10.5))
                            .color(Color32::from_rgb(126, 178, 255)),
                    );
                    ui.label(RichText::new(":").font(mono(10.5)).color(FAINT));
                    ui.add(
                        egui::Label::new(RichText::new(v).font(mono(10.5)).color(TEXT))
                            .selectable(true)
                            .wrap(),
                    );
                });
                ui.add_space(1.0);
            }
        });
}

fn inspector_headers(ui: &mut Ui, c: &Capture) {
    section_label(ui, "Request headers");
    ui.add_space(5.0);
    header_table(ui, &c.req_headers);
    ui.add_space(12.0);
    section_label(ui, "Response headers");
    ui.add_space(5.0);
    header_table(ui, &c.resp_headers);
    ui.add_space(12.0);
    section_label(ui, "Matched rules");
    ui.add_space(5.0);
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(9.0)
        .inner_margin(Margin::same(10.0))
        .show(ui, |ui| {
            if let Some(rule) = &c.mock {
                ui.horizontal(|ui| {
                    dot(ui, AMBER, 7.0);
                    ui.label(
                        RichText::new("Map Local")
                            .size(11.0)
                            .strong()
                            .color(AMBER),
                    );
                    ui.label(RichText::new(rule).size(11.0).color(MUTED));
                });
            } else {
                ui.label(
                    RichText::new("No Map Local rule matched this request")
                        .size(11.0)
                        .color(FAINT),
                );
            }
        });
}

fn empty_body(ui: &mut Ui, text: &str) {
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(9.0)
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(FAINT));
        });
}

fn status_text(status: u16) -> &'static str {
    match status {
        0 => "Failed",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        422 => "Unprocessable",
        429 => "Too Many Requests",
        500 => "Server Error",
        502 => "Bad Gateway",
        503 => "Unavailable",
        _ => "Response",
    }
}

fn header_value(headers: &[(String, String)], key: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
}

/// Ask macOS for a response fixture. The picker is optional: the editor also
/// accepts a typed path, and a failed picker simply leaves the field alone.
fn pick_local_fixture() -> Option<String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("POSIX path of (choose file with prompt \"Choose a response fixture\")")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn parse_headers(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (k, v) = line.split_once(':')?;
            let k = k.trim();
            if k.is_empty() {
                None
            } else {
                Some((k.to_string(), v.trim().to_string()))
            }
        })
        .collect()
}

fn host_port_of(url: &str) -> String {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("");
    if host.contains(':') {
        host.to_string()
    } else if url.starts_with("https") {
        format!("{}:443", host)
    } else {
        format!("{}:80", host)
    }
}

fn capture_pattern(c: &Capture) -> String {
    let base = c.url.split('?').next().unwrap_or(&c.url);
    let base = base.trim_end_matches('/');
    if c.url.contains('?') {
        format!("{}?*", base)
    } else {
        base.to_string()
    }
}

fn capture_rule_name(c: &Capture, prefix: &str) -> String {
    let path = c.path();
    let clean = path.split('?').next().unwrap_or("/");
    let leaf = clean
        .trim_end_matches('/')
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("");
    if leaf.is_empty() {
        format!("{} {}", prefix, c.host())
    } else {
        format!("{} {}", prefix, leaf)
    }
}

fn rule_method_for(method: &str) -> RuleMethod {
    match method {
        "GET" => RuleMethod::Get,
        "POST" => RuleMethod::Post,
        "PUT" => RuleMethod::Put,
        "DELETE" => RuleMethod::Delete,
        _ => RuleMethod::Any,
    }
}

fn run_curl(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    proxy: Option<(u16, Option<std::path::PathBuf>)>,
) -> SendOutcome {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("proxzy_send_{}", stamp));
    let _ = std::fs::create_dir_all(&dir);
    let body_path = dir.join("request_body");
    let header_path = dir.join("response_headers");
    let out_path = dir.join("response_body");
    let _ = std::fs::write(&body_path, body);

    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-sS")
        .arg("--max-time")
        .arg("20")
        .arg("-X")
        .arg(method)
        .arg("-D")
        .arg(&header_path)
        .arg("-o")
        .arg(&out_path)
        .arg("-w")
        .arg("%{http_code}");
    if let Some((port, ca_path)) = &proxy {
        cmd.arg("-x").arg(format!("http://127.0.0.1:{}", port));
        if url.starts_with("https://") {
            match ca_path {
                Some(path) if path.exists() => {
                    cmd.arg("--cacert").arg(path);
                }
                _ => {
                    cmd.arg("-k");
                }
            }
        }
    }
    for (k, v) in headers {
        cmd.arg("-H").arg(format!("{}: {}", k, v));
    }
    if !body.is_empty() {
        cmd.arg("--data-binary")
            .arg(format!("@{}", body_path.display()));
    }
    cmd.arg(url);

    let started = Instant::now();
    let output = cmd.output();
    let latency_ms = started.elapsed().as_millis() as u64;
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let status: u16 = stdout.parse().unwrap_or(0);
            let resp_body = std::fs::read_to_string(&out_path).unwrap_or_default();
            let raw_headers = std::fs::read_to_string(&header_path).unwrap_or_default();
            let resp_headers = raw_headers
                .lines()
                .filter_map(|line| {
                    if line.starts_with("HTTP/") || line.trim().is_empty() {
                        return None;
                    }
                    if line.starts_with(' ') {
                        return None;
                    }
                    let (k, v) = line.split_once(':')?;
                    Some((k.trim().to_string(), v.trim().to_string()))
                })
                .collect::<Vec<_>>();
            let error = if status == 0 {
                Some(if stderr.is_empty() {
                    "No HTTP response".to_string()
                } else {
                    stderr
                })
            } else {
                None
            };
            let outcome = SendOutcome {
                method: method.to_string(),
                url: url.to_string(),
                headers: headers.to_vec(),
                req_body: body.to_string(),
                status,
                latency_ms: latency_ms.max(1),
                size: resp_body.len() as u64,
                resp_headers,
                body: resp_body,
                error,
            };
            let _ = std::fs::remove_dir_all(&dir);
            outcome
        }
        Err(err) => {
            let _ = std::fs::remove_dir_all(&dir);
            SendOutcome {
                method: method.to_string(),
                url: url.to_string(),
                headers: headers.to_vec(),
                req_body: body.to_string(),
                status: 0,
                latency_ms: latency_ms.max(1),
                size: 0,
                resp_headers: Vec::new(),
                body: String::new(),
                error: Some(format!(
                    "Could not start curl: {}. Check that curl is installed.",
                    err
                )),
            }
        }
    }
}

/// Find the UDID of the simulator that is currently booted, if any.
fn booted_simulator_udid() -> Option<String> {
    let out = std::process::Command::new("xcrun")
        .args(["simctl", "list", "devices", "booted"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if !line.contains("(Booted)") {
            continue;
        }
        if let Some(udid) = find_uuid(line) {
            return Some(udid);
        }
    }
    None
}

fn find_uuid(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    for start in 0..bytes.len() {
        if start + 36 <= bytes.len() {
            let candidate = &line[start..start + 36];
            if is_uuid(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn reboot_simulator(udid: &str) -> Result<String, String> {
    // Ignore a shutdown error: the device may already be shutting down.
    let _ = std::process::Command::new("xcrun")
        .args(["simctl", "shutdown", udid])
        .status();
    let boot = std::process::Command::new("xcrun")
        .args(["simctl", "boot", udid])
        .output()
        .map_err(|e| format!("Could not run simctl boot: {}", e))?;
    if !boot.status.success() {
        let err = String::from_utf8_lossy(&boot.stderr).trim().to_string();
        return Err(format!(
            "Simulator did not boot: {}",
            if err.is_empty() { "simctl returned an error" } else { &err }
        ));
    }
    let status = std::process::Command::new("xcrun")
        .args(["simctl", "bootstatus", udid, "-b"])
        .output()
        .map_err(|e| format!("Could not wait for the simulator: {}", e))?;
    if !status.status.success() {
        let err = String::from_utf8_lossy(&status.stderr).trim().to_string();
        return Err(format!(
            "Simulator did not finish booting: {}",
            if err.is_empty() { "simctl returned an error" } else { &err }
        ));
    }
    Ok("Simulator restarted and now trusts the Proxzy CA".to_string())
}

/// Network services that are currently enabled on this Mac. `networksetup`
/// prefixes disabled services with an asterisk, which we skip.
fn enabled_network_services() -> Vec<String> {
    let Ok(out) = std::process::Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('*')
                && !line.to_ascii_lowercase().starts_with("an asterisk")
        })
        .map(str::to_string)
        .collect()
}

fn run_networksetup(args: &[&str]) -> bool {
    std::process::Command::new("networksetup")
        .args(args)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// What `networksetup -getwebproxy <service>` reports for one service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebProxyState {
    pub service: String,
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

/// Parse the `networksetup -getwebproxy` block for one service.
///
/// ```text
/// Enabled: No
/// Server: 127.0.0.1
/// Port: 9090
/// Authenticated Proxy: No
/// ```
pub fn parse_web_proxy_block(service: &str, text: &str) -> WebProxyState {
    let mut enabled = false;
    let mut host = String::new();
    let mut port = 0u16;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Enabled:") {
            enabled = rest.trim().eq_ignore_ascii_case("yes");
        } else if let Some(rest) = line.strip_prefix("Server:") {
            host = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("Port:") {
            port = rest.trim().parse().unwrap_or(0);
        }
    }
    WebProxyState {
        service: service.to_string(),
        enabled,
        host,
        port,
    }
}

/// True when the web proxy points at this machine on a port that no
/// listener owns. That is the leftover a crash leaves behind: the Mac
/// keeps routing browsers through 127.0.0.1:<port> that nothing answers.
pub fn proxy_is_stale(state: &WebProxyState) -> bool {
    proxy_is_stale_with(state, port_has_listener)
}

/// Pure helper: evaluate whether a proxy configuration is stale given a listener check.
pub fn proxy_is_stale_with(state: &WebProxyState, has_listener: impl Fn(u16) -> bool) -> bool {
    state.enabled
        && state.host == "127.0.0.1"
        && state.port > 0
        && !has_listener(state.port)
}

/// A TCP connect probe: a listener answers, a dead port refuses at once.
pub fn port_has_listener(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// The round trip used by the connection card: one absolute-form request goes
/// through the real listener, the proxy answers it locally and records it.
fn capture_check_request(port: u16, nonce: u64) -> Result<u16, String> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|e| format!("Could not reach 127.0.0.1:{} ({})", port, e))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(6)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    let request = format!(
        "GET http://proxzy.gateway/__proxzy/check?nonce={} HTTP/1.1\r\nHost: proxzy.gateway\r\nUser-Agent: Proxzy-Capture-Check\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        nonce
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Could not write the check request: {}", e))?;
    let mut buffer = Vec::new();
    // The status line is enough to prove the gate answered. A read timeout
    // after the body is harmless, so the result is deliberately ignored.
    let _ = stream.read_to_end(&mut buffer);
    let text = String::from_utf8_lossy(&buffer);
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "The gate closed without an HTTP answer".to_string())?;
    if status == 200 {
        Ok(status)
    } else {
        Err(format!("The gate answered HTTP {}", status))
    }
}

/// One call from the practice session. It uses the same absolute-form request
/// shape as a device: through the listener, closed after the answer.
fn practice_request(port: u16, method: &str, path: &str, body: &str) -> Result<u16, String> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|e| format!("Could not reach 127.0.0.1:{} ({})", port, e))?;
    // A paused breakpoint can hold the call for up to three minutes, so the
    // read timeout is patient; the person is the one who releases it.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(185)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut request = format!(
        "{method} http://{host}{path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Proxzy-Practice/1.0\r\nAccept: application/json\r\nConnection: close\r\n",
        method = method,
        host = crate::proxy::PRACTICE_HOST,
        path = path
    );
    if !body.is_empty() {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Could not write the practice call: {}", e))?;
    let mut buffer = Vec::new();
    let _ = stream.read_to_end(&mut buffer);
    let text = String::from_utf8_lossy(&buffer);
    text.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "The gate closed without an HTTP answer".to_string())
}

/// Read the web proxy for every enabled network service. This is read-only:
/// it never changes the Mac's network settings.
pub fn read_all_web_proxies() -> Vec<WebProxyState> {
    enabled_network_services()
        .into_iter()
        .filter_map(|service| {
            let out = std::process::Command::new("networksetup")
                .args(["-getwebproxy", &service])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            Some(parse_web_proxy_block(&service, &text))
        })
        .collect()
}

/// Scan every enabled network service and report the ones whose web proxy
/// points at one of Proxzy's own ports. Both cases matter at launch: a live
/// port is an older Proxzy session whose setting we should adopt and restore
/// on quit, while a dead port is the leftover that breaks the browser.
pub fn find_local_web_proxies() -> Vec<WebProxyState> {
    enabled_network_services()
        .into_iter()
        .filter_map(|service| {
            let out = std::process::Command::new("networksetup")
                .args(["-getwebproxy", &service])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let state = parse_web_proxy_block(&service, &text);
            let is_ours = state.enabled
                && state.host == "127.0.0.1"
                && (9090..=9093).contains(&state.port);
            is_ours.then_some(state)
        })
        .collect()
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn practice_session_is_a_real_round_trip_through_the_gate() {
        let port = {
            let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            probe.local_addr().unwrap().port()
        };
        let (tx, rx) = channel();
        let mut handle = crate::proxy::start_with_options(
            crate::proxy::ProxyStartOptions {
                port,
                ..crate::proxy::ProxyStartOptions::default()
            },
            tx,
        )
        .expect("practice test proxy should start");

        let success = r#"{"source":"practice-mock","products":[]}"#;
        let mock = MapRule {
            id: 1,
            name: "Practice · Products mock".into(),
            folder: "Practice".into(),
            pattern: "*/demo/products*".into(),
            method: RuleMethod::Get,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 0,
            body: success.into(),
            local_file: String::new(),
            graphql_name: String::new(),
            enabled: true,
            hits: 0,
            active_scenario: MockScenario::Success,
            success_status: 200,
            success_body: success.into(),
            error_status: 500,
            error_body: "{}".into(),
            empty_status: 200,
            empty_body: "{}".into(),
            inject_props: std::collections::HashMap::new(),
            app_version: String::new(),
            env: String::new(),
        };
        let bp = BreakpointRule {
            id: 1,
            name: "Practice · Pause checkout".into(),
            pattern: "*/demo/checkout*".into(),
            graphql_name: String::new(),
            stage: BpStage::Request,
            enabled: true,
            hits: 0,
        };
        let mut cfg = crate::proxy::ProxyConfig::default();
        cfg.recording = true;
        cfg.map_enabled = true;
        cfg.map_rules = vec![mock];
        cfg.catch_enabled = true;
        cfg.breakpoints = vec![bp];
        handle.update(cfg);

        let client_port = handle.port;
        let client = std::thread::spawn(move || {
            let mut statuses = Vec::new();
            for (index, (_, method, path, body)) in PRACTICE_STEPS.iter().enumerate() {
                if index > 0 {
                    std::thread::sleep(Duration::from_millis(40));
                }
                statuses.push(practice_request(client_port, method, path, body).unwrap_or(0));
            }
            statuses
        });

        let mut captures = Vec::new();
        let mut mocked = false;
        let mut paused_request = None;
        let deadline = Instant::now() + Duration::from_secs(20);
        while captures.len() < PRACTICE_STEPS.len() && Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(crate::proxy::ProxyEvent::Capture(capture)) => {
                    if capture.mock.is_some() {
                        mocked = true;
                    }
                    captures.push(*capture);
                }
                Ok(crate::proxy::ProxyEvent::Breakpoint(bp)) => {
                    paused_request = Some(*bp);
                }
                Ok(_) => {}
                Err(_) => {}
            }
            if let Some(bp) = paused_request.take() {
                handle.resolve(
                    bp.id,
                    crate::proxy::BpDecision::Continue {
                        method: bp.method,
                        url: bp.url,
                        headers: bp.headers,
                        body: bp.body,
                    },
                );
            }
        }
        let statuses = client.join().unwrap_or_default();
        handle.stop();

        assert_eq!(
            captures.len(),
            PRACTICE_STEPS.len(),
            "every practice call should be recorded"
        );
        assert!(mocked, "one practice call must be answered by Map Local");
        assert!(
            captures.iter().any(|c| c.status == 401),
            "the practice orders call should be a real 401"
        );
        assert_eq!(statuses.len(), PRACTICE_STEPS.len());
        assert!(statuses.iter().all(|s| *s > 0));
    }

    #[test]
    fn finds_uuid_in_simctl_line() {
        let line = "    iPhone 16 Pro (044AEE75-8148-4976-9B50-EAFB2C887E55) (Booted)";
        assert_eq!(
            find_uuid(line).as_deref(),
            Some("044AEE75-8148-4976-9B50-EAFB2C887E55")
        );
    }

    #[test]
    fn ignores_text_that_is_not_a_uuid() {
        assert_eq!(find_uuid("iPhone 16 Pro (Booted)"), None);
        assert_eq!(find_uuid("044AEE75-8148-4976-9B50"), None);
    }

    #[test]
    fn parses_networksetup_web_proxy_block() {
        let text = "Enabled: Yes\nServer: 127.0.0.1\nPort: 9091\nAuthenticated Proxy: No\n";
        let state = parse_web_proxy_block("Wi-Fi", text);
        assert!(state.enabled);
        assert_eq!(state.host, "127.0.0.1");
        assert_eq!(state.port, 9091);
        assert_eq!(state.service, "Wi-Fi");
    }

    #[test]
    fn disabled_proxy_is_never_stale() {
        let state = WebProxyState {
            service: "Wi-Fi".into(),
            enabled: false,
            host: "127.0.0.1".into(),
            port: 1,
        };
        assert!(!proxy_is_stale(&state));
    }

    #[test]
    fn pinned_selection_stays_on_the_same_call() {
        let mut rows = sample_captures();
        let pinned_id = rows[2].id;
        let mut newest = rows[0].clone();
        newest.id = 900;
        rows.insert(0, newest);

        let index = resolve_selection(false, Some(pinned_id), &rows);
        assert_eq!(rows[index].id, pinned_id);
        assert_eq!(resolve_selection(true, Some(pinned_id), &rows), 0);
        assert_eq!(resolve_selection(false, Some(12345), &rows), 0);
    }

    #[test]
    fn parses_an_enabled_web_proxy_block() {
        let text = "Enabled: Yes\nServer: 127.0.0.1\nPort: 9090\nAuthenticated Proxy: No\n";
        let state = parse_web_proxy_block("Wi-Fi", text);
        assert_eq!(
            state,
            WebProxyState {
                service: "Wi-Fi".to_string(),
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 9090,
            }
        );
    }

    #[test]
    fn parses_a_disabled_web_proxy_block() {
        let text = "Enabled: No\nServer: 127.0.0.1\nPort: 9090\nAuthenticated Proxy: No\n";
        let state = parse_web_proxy_block("Wi-Fi", text);
        assert!(!state.enabled);
        assert_eq!(state.host, "127.0.0.1");
        assert_eq!(state.port, 9090);
    }

    #[test]
    fn parses_garbage_as_a_disabled_empty_proxy() {
        let state = parse_web_proxy_block("Wi-Fi", "not a networksetup block");
        assert!(!state.enabled);
        assert_eq!(state.host, "");
        assert_eq!(state.port, 0);
    }

    #[test]
    fn disabled_or_remote_proxies_are_never_stale() {
        let disabled = WebProxyState {
            service: "Wi-Fi".to_string(),
            enabled: false,
            host: "127.0.0.1".to_string(),
            port: 9090,
        };
        assert!(!proxy_is_stale(&disabled));

        let remote = WebProxyState {
            service: "Wi-Fi".to_string(),
            enabled: true,
            host: "proxy.corp.example.com".to_string(),
            port: 8080,
        };
        // A remote host is not ours to clear even when unreachable.
        assert!(!proxy_is_stale(&remote));

        let no_port = WebProxyState {
            service: "Wi-Fi".to_string(),
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 0,
        };
        assert!(!proxy_is_stale(&no_port));
    }

    #[test]
    fn a_live_listener_means_the_proxy_is_not_stale() {
        let state = WebProxyState {
            service: "Wi-Fi".to_string(),
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 9090,
        };
        assert!(!proxy_is_stale_with(&state, |_port| true));

        // When environment permissions allow loopback connections, verify end-to-end:
        if let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:0") {
            if let Ok(addr) = listener.local_addr() {
                if port_has_listener(addr.port()) {
                    let live_state = WebProxyState {
                        service: "Wi-Fi".to_string(),
                        enabled: true,
                        host: "127.0.0.1".to_string(),
                        port: addr.port(),
                    };
                    assert!(!proxy_is_stale(&live_state));
                }
            }
        }
    }

    #[test]
    fn a_dead_local_port_means_the_proxy_is_stale() {
        let state = WebProxyState {
            service: "Wi-Fi".to_string(),
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 9091,
        };
        assert!(proxy_is_stale_with(&state, |_port| false));

        // Bind, note the port, then drop the listener: nothing answers there.
        // Tests run in parallel, so another one may grab the freed port between
        // the drop and the probe; retry a few fresh ports before giving up.
        for _ in 0..8 {
            if let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:0") {
                if let Ok(addr) = listener.local_addr() {
                    let port = addr.port();
                    drop(listener);
                    if port_has_listener(port) {
                        continue;
                    }
                    let dead_state = WebProxyState {
                        service: "Wi-Fi".to_string(),
                        enabled: true,
                        host: "127.0.0.1".to_string(),
                        port,
                    };
                    assert!(proxy_is_stale(&dead_state));
                    return;
                }
            }
        }
    }
}

include!("views.rs");
include!("rules_ui.rs");
include!("studio_ui.rs");
include!("tour_ui.rs");
include!("acp.rs");
include!("acp_ui.rs");
