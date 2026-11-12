//! Automation studio model.
//!
//! Three surfaces share this file:
//! - the Agent Control page (scenario switches, injections and sessions),
//! - the API Test Bench (WireMock-style request tests with pass/fail checks),
//! - the Screen Play & MCP page (a runnable list of UI and engine steps that can
//!   be synchronized with an external mobile-mcp / Playwright harness).
//!
//! Everything here is plain data and pure helpers so the rules can be tested
//! without opening a window. The UI lives in `studio_ui.rs`.

use crate::model::{pattern_matches_url, MapRule, MockScenario};

// ---------------------------------------------------------------------------
// Targets and the port configuration
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetPlatform {
    Mac,
    IosSimulator,
    IosDevice,
    AndroidEmulator,
    AndroidDevice,
}

/// How a physical Android phone reaches the proxy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AndroidTransport {
    /// Safe default: `adb reverse` keeps the listener on loopback.
    UsbReverse,
    /// Untethered: the phone uses the Mac's LAN address, which requires the
    /// explicit LAN opt-in and a control token.
    WifiLan,
}

impl AndroidTransport {
    pub const ALL: [AndroidTransport; 2] =
        [AndroidTransport::UsbReverse, AndroidTransport::WifiLan];

    pub fn label(self) -> &'static str {
        match self {
            AndroidTransport::UsbReverse => "USB · adb reverse",
            AndroidTransport::WifiLan => "Wi-Fi · LAN address",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            AndroidTransport::UsbReverse => "usb-reverse",
            AndroidTransport::WifiLan => "wifi-lan",
        }
    }

    pub fn from_slug(value: &str) -> Self {
        match value {
            "wifi-lan" | "wifi" | "lan" => AndroidTransport::WifiLan,
            _ => AndroidTransport::UsbReverse,
        }
    }
}

impl TargetPlatform {
    pub const ALL: [TargetPlatform; 5] = [
        TargetPlatform::Mac,
        TargetPlatform::IosSimulator,
        TargetPlatform::IosDevice,
        TargetPlatform::AndroidEmulator,
        TargetPlatform::AndroidDevice,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TargetPlatform::Mac => "This Mac",
            TargetPlatform::IosSimulator => "iOS Simulator",
            TargetPlatform::IosDevice => "iPhone (Wi-Fi)",
            TargetPlatform::AndroidEmulator => "Android emulator",
            TargetPlatform::AndroidDevice => "Android phone",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            TargetPlatform::Mac => "MAC",
            TargetPlatform::IosSimulator => "iOS SIM",
            TargetPlatform::IosDevice => "IPHONE",
            TargetPlatform::AndroidEmulator => "ANDROID EMU",
            TargetPlatform::AndroidDevice => "ANDROID",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            TargetPlatform::Mac => "mac",
            TargetPlatform::IosSimulator => "ios-sim",
            TargetPlatform::IosDevice => "ios-device",
            TargetPlatform::AndroidEmulator => "android-emu",
            TargetPlatform::AndroidDevice => "android-device",
        }
    }

    pub fn from_slug(value: &str) -> Self {
        match value {
            "ios-sim" | "ios" => TargetPlatform::IosSimulator,
            "ios-device" | "iphone" => TargetPlatform::IosDevice,
            "android-emu" | "android" => TargetPlatform::AndroidEmulator,
            "android-device" => TargetPlatform::AndroidDevice,
            _ => TargetPlatform::Mac,
        }
    }

    /// The bridge needed before the app under test can see the engine.
    #[cfg(test)]
    pub fn bridge_command(self, port: u16) -> Option<String> {
        match self {
            TargetPlatform::AndroidEmulator | TargetPlatform::AndroidDevice => {
                Some(format!("adb reverse tcp:{port} tcp:{port}"))
            }
            _ => None,
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            TargetPlatform::Mac => "Requests from this Mac can use the proxy directly.",
            TargetPlatform::IosSimulator => {
                "The simulator shares this Mac's loopback; no port bridge is needed."
            }
            TargetPlatform::IosDevice => {
                "A physical iPhone uses this Mac's LAN address; turn on LAN access and install the .cer profile."
            }
            TargetPlatform::AndroidEmulator => {
                "Run adb reverse once so the emulator's localhost points at this Mac."
            }
            TargetPlatform::AndroidDevice => {
                "Use adb reverse over USB, or switch to Wi-Fi and the Mac's LAN address."
            }
        }
    }
}

/// The address to write into a device snippet for the chosen target.
pub fn host_for_target(
    platform: TargetPlatform,
    lan_ip: &str,
    transport: AndroidTransport,
) -> String {
    match platform {
        TargetPlatform::IosDevice => lan_ip.to_string(),
        TargetPlatform::AndroidDevice => match transport {
            AndroidTransport::UsbReverse => "127.0.0.1".to_string(),
            AndroidTransport::WifiLan => lan_ip.to_string(),
        },
        _ => "127.0.0.1".to_string(),
    }
}

/// Bridge command for a specific transport. Wi-Fi targets need no adb bridge.
pub fn bridge_command_for(
    platform: TargetPlatform,
    port: u16,
    transport: AndroidTransport,
) -> Option<String> {
    match (platform, transport) {
        (TargetPlatform::AndroidEmulator, _) => Some(format!("adb reverse tcp:{port} tcp:{port}")),
        (TargetPlatform::AndroidDevice, AndroidTransport::UsbReverse) => {
            Some(format!("adb reverse tcp:{port} tcp:{port}"))
        }
        _ => None,
    }
}

/// Fill `{host}` and `{port}` in a test URL from the port configuration.
pub fn resolve_url(template: &str, port: u16) -> String {
    template
        .replace("{host}", "127.0.0.1")
        .replace("{port}", &port.to_string())
}

// ---------------------------------------------------------------------------
// API tests
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct TestResponse {
    pub status: u16,
    pub latency_ms: u64,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub error: Option<String>,
}

impl TestResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The name of the Map Local rule that answered, if any.
    pub fn served_by_mock(&self) -> Option<&str> {
        self.header("x-proxzy-rule")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckKind {
    Status(u16),
    BodyContains(String),
    JsonKey(String),
    Header(String, String),
    FasterThan(u64),
    ServedByMock(bool),
}

impl CheckKind {
    pub fn label(&self) -> String {
        match self {
            CheckKind::Status(status) => format!("Status is {}", status),
            CheckKind::BodyContains(text) => format!("Body contains \"{}\"", text),
            CheckKind::JsonKey(key) => format!("JSON has key \"{}\"", key),
            CheckKind::Header(name, value) => format!("Header {} is {}", name, value),
            CheckKind::FasterThan(ms) => format!("Answers under {} ms", ms),
            CheckKind::ServedByMock(true) => "Answered by Map Local".into(),
            CheckKind::ServedByMock(false) => "Answered by the live server".into(),
        }
    }

    pub fn evaluate(&self, response: &TestResponse) -> CheckOutcome {
        let (passed, detail) = match self {
            CheckKind::Status(want) => (
                response.status == *want,
                format!("got HTTP {}", response.status),
            ),
            CheckKind::BodyContains(needle) => {
                let hit = !needle.is_empty() && response.body.contains(needle);
                (
                    hit,
                    if hit {
                        "found in the response".to_string()
                    } else if response.body.is_empty() {
                        "the response body is empty".to_string()
                    } else {
                        "not found".to_string()
                    },
                )
            }
            CheckKind::JsonKey(key) => match json_key_present(&response.body, key) {
                Ok(found) => (
                    found,
                    if found {
                        format!("\"{}\" is present", key)
                    } else {
                        format!("\"{}\" is missing", key)
                    },
                ),
                Err(err) => (false, err),
            },
            CheckKind::Header(name, want) => {
                let got = response.header(name);
                (
                    got.map(|v| v.eq_ignore_ascii_case(want)).unwrap_or(false),
                    match got {
                        Some(value) => format!("{}: {}", name, value),
                        None => format!("{} header is missing", name),
                    },
                )
            }
            CheckKind::FasterThan(limit) => (
                response.latency_ms <= *limit,
                format!("took {} ms", response.latency_ms),
            ),
            CheckKind::ServedByMock(true) => (
                response.served_by_mock().is_some(),
                match response.served_by_mock() {
                    Some(rule) => format!("served by \"{}\"", rule),
                    None => "the live server answered".to_string(),
                },
            ),
            CheckKind::ServedByMock(false) => (
                response.served_by_mock().is_none(),
                match response.served_by_mock() {
                    Some(rule) => format!("served by \"{}\"", rule),
                    None => "no Map Local rule matched".to_string(),
                },
            ),
        };
        CheckOutcome {
            label: self.label(),
            passed,
            detail,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckOutcome {
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExpectSource {
    Any,
    Mock,
    Live,
}

impl ExpectSource {
    pub fn label(self) -> &'static str {
        match self {
            ExpectSource::Any => "Any",
            ExpectSource::Mock => "Map Local",
            ExpectSource::Live => "Live server",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ExpectSource::Any => "any",
            ExpectSource::Mock => "mock",
            ExpectSource::Live => "live",
        }
    }

    pub fn from_slug(value: &str) -> Self {
        match value {
            "mock" => ExpectSource::Mock,
            "live" => ExpectSource::Live,
            _ => ExpectSource::Any,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ApiTest {
    pub id: u64,
    pub name: String,
    pub suite: String,
    pub method: String,
    /// May contain `{host}` and `{port}`; they are filled from the port config.
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// When set, the runner switches this Map Local rule before sending.
    pub scenario: Option<MockScenario>,
    pub scenario_target: String,
    pub injects: Vec<(String, String)>,
    pub expect_status: String,
    pub expect_body: String,
    pub expect_json_key: String,
    pub expect_header_name: String,
    pub expect_header_value: String,
    pub expect_max_ms: String,
    pub expect_source: ExpectSource,
    pub last: Option<TestRun>,
}

impl ApiTest {
    pub fn new(id: u64, name: &str, suite: &str, method: &str, url: &str) -> Self {
        Self {
            id,
            name: name.into(),
            suite: suite.into(),
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: String::new(),
            scenario: None,
            scenario_target: String::new(),
            injects: Vec::new(),
            expect_status: "200".into(),
            expect_body: String::new(),
            expect_json_key: String::new(),
            expect_header_name: String::new(),
            expect_header_value: String::new(),
            expect_max_ms: String::new(),
            expect_source: ExpectSource::Any,
            last: None,
        }
    }

    pub fn is_graphql(&self) -> bool {
        self.body.contains("\"query\"") || self.body.trim_start().starts_with("query ")
    }

    pub fn checks(&self) -> Vec<CheckKind> {
        let mut checks = Vec::new();
        if let Ok(status) = self.expect_status.trim().parse::<u16>() {
            checks.push(CheckKind::Status(status));
        }
        if !self.expect_body.trim().is_empty() {
            checks.push(CheckKind::BodyContains(self.expect_body.trim().to_string()));
        }
        if !self.expect_json_key.trim().is_empty() {
            checks.push(CheckKind::JsonKey(self.expect_json_key.trim().to_string()));
        }
        if !self.expect_header_name.trim().is_empty() {
            checks.push(CheckKind::Header(
                self.expect_header_name.trim().to_string(),
                self.expect_header_value.trim().to_string(),
            ));
        }
        if let Ok(ms) = self.expect_max_ms.trim().parse::<u64>() {
            checks.push(CheckKind::FasterThan(ms));
        }
        match self.expect_source {
            ExpectSource::Any => {}
            ExpectSource::Mock => checks.push(CheckKind::ServedByMock(true)),
            ExpectSource::Live => checks.push(CheckKind::ServedByMock(false)),
        }
        checks
    }

    pub fn evaluate(&self, response: &TestResponse) -> TestRun {
        let checks = self.checks();
        let outcomes: Vec<CheckOutcome> = checks.iter().map(|c| c.evaluate(response)).collect();
        let passed = response.error.is_none() && !outcomes.is_empty() && outcomes.iter().all(|c| c.passed);
        TestRun {
            passed,
            checks: outcomes,
            status: response.status,
            latency_ms: response.latency_ms,
            body_preview: body_preview(&response.body),
            error: response.error.clone(),
            ran_at: crate::model::now_stamp(),
        }
    }

    pub fn headers_text(&self) -> String {
        headers_to_text(&self.headers)
    }

    pub fn set_headers_text(&mut self, text: &str) {
        self.headers = parse_header_lines(text);
    }
}

#[derive(Clone, Debug)]
pub struct TestRun {
    pub passed: bool,
    pub checks: Vec<CheckOutcome>,
    pub status: u16,
    pub latency_ms: u64,
    pub body_preview: String,
    pub error: Option<String>,
    pub ran_at: String,
}

fn body_preview(body: &str) -> String {
    let clean = body.trim();
    if clean.chars().count() > 420 {
        let mut out: String = clean.chars().take(420).collect();
        out.push('…');
        out
    } else {
        clean.to_string()
    }
}

pub fn parse_header_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), value.trim().to_string()))
        })
        .collect()
}

pub fn headers_to_text(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A small seeded suite so the Test Bench is understandable at a glance.
pub fn sample_tests() -> Vec<ApiTest> {
    let mut wallet_ok = ApiTest::new(
        1,
        "Wallet balance · success",
        "Wallet",
        "GET",
        "http://{host}:{port}/v1/wallet/balance",
    );
    wallet_ok.expect_body = "\"balance\"".into();
    wallet_ok.expect_json_key = "balance".into();
    wallet_ok.expect_header_name = "content-type".into();
    wallet_ok.expect_header_value = "application/json".into();
    wallet_ok.expect_max_ms = "400".into();
    wallet_ok.expect_source = ExpectSource::Mock;
    wallet_ok.scenario = Some(MockScenario::Success);
    wallet_ok.scenario_target = "Wallet balance".into();

    let mut wallet_error = ApiTest::new(
        2,
        "Wallet balance · error state",
        "Wallet",
        "GET",
        "http://{host}:{port}/v1/wallet/balance",
    );
    wallet_error.expect_status = "500".into();
    wallet_error.expect_body = "\"error\"".into();
    wallet_error.expect_source = ExpectSource::Mock;
    wallet_error.scenario = Some(MockScenario::Error);
    wallet_error.scenario_target = "Wallet balance".into();

    let mut graphql = ApiTest::new(
        3,
        "GetProducts · GraphQL",
        "Catalog",
        "POST",
        "http://{host}:{port}/graphql",
    );
    graphql.headers = vec![("content-type".into(), "application/json".into())];
    graphql.body = r#"{"query":"query GetProducts { products(first: 3) { nodes { id name price { amount currency } } } }","variables":{}}"#.into();
    graphql.expect_body = "\"products\"".into();
    graphql.expect_json_key = "data".into();
    graphql.expect_source = ExpectSource::Mock;
    graphql.scenario = Some(MockScenario::Success);
    graphql.scenario_target = "GraphQL products fixture".into();

    let mut increment = ApiTest::new(
        4,
        "Counter · POST body",
        "Agent",
        "POST",
        "http://{host}:{port}/v1/counter",
    );
    increment.headers = vec![("content-type".into(), "application/json".into())];
    increment.body = r#"{"delta":1,"user":"{{USERNAME}}"}"#.into();
    increment.expect_status = "201".into();
    increment.expect_body = "\"count\"".into();
    increment.expect_source = ExpectSource::Mock;
    increment.injects = vec![("USERNAME".into(), "qa.agent".into())];
    increment.scenario = Some(MockScenario::Success);
    increment.scenario_target = "Counter increment".into();

    let mut live = ApiTest::new(
        5,
        "Public API · live title",
        "Live",
        "GET",
        "https://jsonplaceholder.typicode.com/todos/1",
    );
    live.expect_body = "\"title\"".into();
    live.expect_json_key = "title".into();
    live.expect_source = ExpectSource::Live;
    live.expect_max_ms = "5000".into();

    vec![wallet_ok, wallet_error, graphql, increment, live]
}

// ---------------------------------------------------------------------------
// Screen plays
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepKind {
    Launch,
    Tap,
    Type,
    ExpectVisible,
    Scenario,
    Inject,
    ExpectHit,
    Wait,
}

impl StepKind {
    pub fn label(self) -> &'static str {
        match self {
            StepKind::Launch => "Launch app",
            StepKind::Tap => "Tap",
            StepKind::Type => "Type",
            StepKind::ExpectVisible => "Expect on screen",
            StepKind::Scenario => "Switch scenario",
            StepKind::Inject => "Inject value",
            StepKind::ExpectHit => "Wait for mock hit",
            StepKind::Wait => "Wait",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            StepKind::Launch => "launch",
            StepKind::Tap => "tap",
            StepKind::Type => "type",
            StepKind::ExpectVisible => "expect",
            StepKind::Scenario => "scenario",
            StepKind::Inject => "inject",
            StepKind::ExpectHit => "hit",
            StepKind::Wait => "wait",
        }
    }

    pub fn from_slug(value: &str) -> Self {
        match value {
            "tap" => StepKind::Tap,
            "type" => StepKind::Type,
            "expect" => StepKind::ExpectVisible,
            "scenario" => StepKind::Scenario,
            "inject" => StepKind::Inject,
            "hit" => StepKind::ExpectHit,
            "wait" => StepKind::Wait,
            _ => StepKind::Launch,
        }
    }

    /// True when Proxzy really does the work; false when an external harness
    /// (mobile-mcp, Playwright, XCUITest) has to perform the UI action.
    pub fn is_engine(self) -> bool {
        matches!(
            self,
            StepKind::Scenario | StepKind::Inject | StepKind::ExpectHit | StepKind::Wait
        )
    }
}

#[derive(Clone, Debug)]
pub struct ScreenStep {
    pub kind: StepKind,
    /// The target of the step: a selector, a rule name, a test name or seconds.
    pub action: String,
    /// Scenario, injected value, expected count or visible text.
    pub value: String,
}

impl ScreenStep {
    pub fn new(kind: StepKind, action: &str, value: &str) -> Self {
        Self {
            kind,
            action: action.into(),
            value: value.into(),
        }
    }

    pub fn describe(&self) -> String {
        match self.kind {
            StepKind::Launch => format!("Launch {}", non_empty(&self.action, "the app under test")),
            StepKind::Tap => format!("Tap {}", non_empty(&self.action, "a control")),
            StepKind::Type => format!(
                "Type \"{}\" into {}",
                self.value,
                non_empty(&self.action, "a field")
            ),
            StepKind::ExpectVisible => {
                format!("Expect \"{}\" on screen", non_empty(&self.action, "a label"))
            }
            StepKind::Scenario => format!(
                "Switch {} to {}",
                non_empty(&self.action, "the mock"),
                non_empty(&self.value, "success")
            ),
            StepKind::Inject => format!(
                "Inject {} into {}",
                non_empty(&self.value, "a value"),
                non_empty(&self.action, "the mock")
            ),
            StepKind::ExpectHit => format!(
                "Wait until {} has {} hit(s)",
                non_empty(&self.action, "the mock"),
                non_empty(&self.value, "1")
            ),
            StepKind::Wait => format!("Wait {} ms", non_empty(&self.action, "500")),
        }
    }
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[derive(Clone, Debug)]
pub struct ScreenPlay {
    pub id: u64,
    pub name: String,
    pub goal: String,
    pub steps: Vec<ScreenStep>,
    pub last: Vec<StepOutcome>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepStatus {
    Passed,
    Simulated,
    Failed,
}

impl StepStatus {
    pub fn label(self) -> &'static str {
        match self {
            StepStatus::Passed => "done",
            StepStatus::Simulated => "simulated",
            StepStatus::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StepOutcome {
    pub step: String,
    pub status: StepStatus,
    pub detail: String,
}

pub fn sample_screen_plays() -> Vec<ScreenPlay> {
    vec![
        ScreenPlay {
            id: 1,
            name: "Wallet error recovery".into(),
            goal: "Show the error state, check the retry screen, then recover to a successful balance."
                .into(),
            steps: vec![
                ScreenStep::new(StepKind::Launch, "com.example.wallet", "iOS or Android UI test"),
                ScreenStep::new(StepKind::Scenario, "Wallet balance", "error"),
                ScreenStep::new(StepKind::Tap, "#wallet-tab", ""),
                ScreenStep::new(StepKind::ExpectHit, "Wallet balance", "1"),
                ScreenStep::new(StepKind::ExpectVisible, "Could not load your balance", ""),
                ScreenStep::new(StepKind::Scenario, "Wallet balance", "success"),
                ScreenStep::new(StepKind::Tap, "#retry-button", ""),
                ScreenStep::new(StepKind::ExpectHit, "Wallet balance", "2"),
                ScreenStep::new(StepKind::ExpectVisible, "128.50", ""),
                ScreenStep::new(StepKind::Wait, "400", ""),
            ],
            last: Vec::new(),
        },
        ScreenPlay {
            id: 2,
            name: "Login with injected user".into(),
            goal: "Inject a test phone and OTP, then check that the auth screen accepts them."
                .into(),
            steps: vec![
                ScreenStep::new(StepKind::Launch, "com.example.wallet", "iOS or Android UI test"),
                ScreenStep::new(StepKind::Inject, "Auth session", "phone=0812345678"),
                ScreenStep::new(StepKind::Inject, "Auth session", "otp=111111"),
                ScreenStep::new(StepKind::Tap, "#phone-field", ""),
                ScreenStep::new(StepKind::Type, "#phone-field", "0812345678"),
                ScreenStep::new(StepKind::Tap, "#send-otp", ""),
                ScreenStep::new(StepKind::ExpectHit, "Auth session", "1"),
                ScreenStep::new(StepKind::Tap, "#otp-field", ""),
                ScreenStep::new(StepKind::Type, "#otp-field", "111111"),
                ScreenStep::new(StepKind::ExpectVisible, "Welcome back", ""),
            ],
            last: Vec::new(),
        },
        ScreenPlay {
            id: 3,
            name: "Empty catalog state".into(),
            goal: "Switch the products mock to the empty scenario and check the empty-list screen."
                .into(),
            steps: vec![
                ScreenStep::new(StepKind::Launch, "com.example.wallet", "iOS or Android UI test"),
                ScreenStep::new(StepKind::Scenario, "GraphQL products fixture", "empty"),
                ScreenStep::new(StepKind::Tap, "#catalog-tab", ""),
                ScreenStep::new(StepKind::ExpectHit, "GraphQL products fixture", "1"),
                ScreenStep::new(StepKind::ExpectVisible, "No products yet", ""),
            ],
            last: Vec::new(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

/// Find the rule a screen-play step or agent command refers to. Names win over
/// patterns; a URL/path target falls back to the normal wildcard matcher.
pub fn rule_index_for_target(rules: &[MapRule], target: &str) -> Option<usize> {
    let target = target.trim();
    if target.is_empty() {
        return rules.first().map(|_| 0);
    }
    if let Some(index) = rules.iter().position(|r| {
        r.name.eq_ignore_ascii_case(target) || r.pattern.eq_ignore_ascii_case(target)
    }) {
        return Some(index);
    }
    rules
        .iter()
        .position(|r| pattern_matches_url(&r.pattern, target))
}

/// Parse `key=value` from an inject step.
pub fn parse_inject(value: &str) -> Option<(String, String)> {
    let (key, value) = value.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    Some((key.to_string(), value.trim().to_string()))
}

/// The environment a UI-test launch passes to the app under test.
#[cfg(test)]
pub fn env_block(port: u16, play_name: &str) -> String {
    env_block_for("127.0.0.1", port, play_name, None)
}

pub fn env_block_for(
    host: &str,
    port: u16,
    play_name: &str,
    control_token: Option<&str>,
) -> String {
    let token_line = control_token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| format!("\nPROXZY_CONTROL_TOKEN={}", t))
        .unwrap_or_default();
    format!(
        "PROXZY_PROXY_URL=http://{}:{}\nPROXZY_HOST={}\nPROXZY_PORT={}\nPROXZY_SESSION={}\nPROXZY_SCENARIO=success{}",
        host, port, host, port, play_name, token_line
    )
}

#[cfg(test)]
pub fn xctest_snippet(port: u16, play_name: &str) -> String {
    xctest_snippet_for("127.0.0.1", port, play_name, None)
}

pub fn xctest_snippet_for(
    host: &str,
    port: u16,
    play_name: &str,
    control_token: Option<&str>,
) -> String {
    let token_line = control_token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| format!("\napp.launchEnvironment[\"PROXZY_CONTROL_TOKEN\"] = \"{}\"", t))
        .unwrap_or_default();
    format!(
        "// XCUITest — pass the proxy config into the app (device on Wi-Fi: http://{host}:{port}/ca.cer)\nlet app = XCUIApplication()\napp.launchEnvironment[\"PROXZY_PROXY_URL\"] = \"http://{host}:{port}\"\napp.launchEnvironment[\"PROXZY_PORT\"] = \"{port}\"\napp.launchEnvironment[\"PROXZY_SESSION\"] = \"{play_name}\"\napp.launchEnvironment[\"PROXZY_SCENARIO\"] = \"success\"{token_line}\napp.launch()"
    )
}

#[cfg(test)]
pub fn espresso_snippet(port: u16, play_name: &str) -> String {
    espresso_snippet_for("127.0.0.1", port, play_name, None)
}

pub fn espresso_snippet_for(
    host: &str,
    port: u16,
    play_name: &str,
    control_token: Option<&str>,
) -> String {
    let token_extra = control_token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| format!("\n  putExtra(\"PROXZY_CONTROL_TOKEN\", \"{}\")", t))
        .unwrap_or_default();
    let bridge_hint = if host == "127.0.0.1" {
        format!("// adb reverse tcp:{port} tcp:{port}\n")
    } else {
        format!("// Wi-Fi LAN proxy: http://{host}:{port} (install /ca.cer on device)\n")
    };
    format!(
        "// Android UI test — pass proxy config into the test intent\n{bridge_hint}val intent = Intent(context, MainActivity::class.java).apply {{\n  putExtra(\"PROXZY_PROXY_URL\", \"http://{host}:{port}\")\n  putExtra(\"PROXZY_PORT\", \"{port}\")\n  putExtra(\"PROXZY_SESSION\", \"{play_name}\"){token_extra}\n}}\n// adb shell settings put global http_proxy {host}:{port}"
    )
}

#[cfg(test)]
pub fn playwright_snippet(port: u16, play_name: &str) -> String {
    playwright_snippet_for("127.0.0.1", port, play_name)
}

pub fn playwright_snippet_for(host: &str, port: u16, play_name: &str) -> String {
    format!(
        "// playwright.config.ts — send browser traffic through Proxzy\nexport default {{\n  use: {{\n    proxy: {{ server: 'http://{host}:{port}' }},\n    ignoreHTTPSErrors: true,\n    extraHTTPHeaders: {{ 'X-Proxzy-Session': '{play_name}' }},\n  }},\n}};"
    )
}

#[cfg(test)]
pub fn mcp_snippet(port: u16, play_name: &str) -> String {
    mcp_snippet_for("127.0.0.1", port, play_name, None)
}

pub fn mcp_snippet_for(
    host: &str,
    port: u16,
    play_name: &str,
    control_token: Option<&str>,
) -> String {
    let token_entry = control_token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| format!(",\n        \"PROXZY_CONTROL_TOKEN\": \"{}\"", escape_json(t)))
        .unwrap_or_default();
    format!(
        "{{\n  \"mcpServers\": {{\n    \"mobilewright\": {{\n      \"command\": \"<your mobilewright MCP command>\",\n      \"env\": {{\n        \"PROXZY_PROXY_URL\": \"http://{host}:{port}\",\n        \"PROXZY_PORT\": \"{port}\",\n        \"PROXZY_SESSION\": \"{play_name}\",\n        \"PROXZY_SCENARIO\": \"success\"{token_entry}\n      }}\n    }}\n  }}\n}}"
    )
}

fn token_header_arg(control_token: Option<&str>) -> String {
    control_token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| format!(" \\\n  -H 'X-Proxzy-Control: {}'", t))
        .unwrap_or_default()
}

#[cfg(test)]
pub fn inject_curl(port: u16, target: &str, key: &str, value: &str) -> String {
    inject_curl_for("127.0.0.1", port, target, key, value, None)
}

pub fn inject_curl_for(
    host: &str,
    port: u16,
    target: &str,
    key: &str,
    value: &str,
    control_token: Option<&str>,
) -> String {
    format!(
        "curl -s -X POST http://{}:{}/__proxzy/inject{} \\\n  -H 'content-type: application/json' \\\n  -d '{{\"pattern\":\"{}\",\"inject\":{{\"{}\":\"{}\"}}}}'",
        host,
        port,
        token_header_arg(control_token),
        escape_json(target),
        escape_json(key),
        escape_json(value)
    )
}

#[cfg(test)]
pub fn scenario_curl(port: u16, target: &str, scenario: &str) -> String {
    scenario_curl_for("127.0.0.1", port, target, scenario, None)
}

pub fn scenario_curl_for(
    host: &str,
    port: u16,
    target: &str,
    scenario: &str,
    control_token: Option<&str>,
) -> String {
    format!(
        "curl -s -X POST http://{}:{}/__proxzy/scenario{} \\\n  -H 'content-type: application/json' \\\n  -d '{{\"pattern\":\"{}\",\"scenario\":\"{}\"}}'",
        host,
        port,
        token_header_arg(control_token),
        escape_json(target),
        escape_json(scenario)
    )
}

#[cfg(test)]
pub fn health_curl(port: u16) -> String {
    health_curl_for("127.0.0.1", port, None)
}

pub fn health_curl_for(host: &str, port: u16, control_token: Option<&str>) -> String {
    match control_token.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => format!("curl -s -H 'X-Proxzy-Control: {}' http://{}:{}/__proxzy/health", t, host, port),
        None => format!("curl -s http://{}:{}/__proxzy/health", host, port),
    }
}

// ---------------------------------------------------------------------------
// WireMock-style export
// ---------------------------------------------------------------------------

/// Export the Map Local rules as a WireMock mappings document. This is a
/// one-way bridge for learning: the file can be read by a WireMock-style
/// engine, but importing mappings back is not part of this POC.
pub fn wiremock_mappings(rules: &[MapRule]) -> String {
    let rows: Vec<String> = rules
        .iter()
        .map(|rule| {
            let method = if rule.method.label() == "ANY" {
                String::new()
            } else {
                format!("\n      \"method\": \"{}\",", rule.method.label())
            };
            let delay = if rule.delay_ms > 0 {
                format!(
                    ",\n      \"fixedDelayMilliseconds\": {}",
                    rule.delay_ms
                )
            } else {
                String::new()
            };
            let json_body = if Json::parse(&rule.body).is_ok() {
                rule.body.trim().to_string()
            } else {
                format!("\"{}\"", escape_json(&rule.body))
            };
            format!(
                "    {{\n      \"name\": \"{}\",\n      \"request\": {{{}\n        \"urlPattern\": \"{}\"\n      }},\n      \"response\": {{\n        \"status\": {},\n        \"headers\": {{ \"Content-Type\": \"{}\" }},{}\n        \"jsonBody\": {}\n      }}\n    }}",
                escape_json(&rule.name),
                method,
                wildcard_regex(&rule.pattern),
                rule.status,
                escape_json(&rule.content_type),
                delay,
                json_body
            )
        })
        .collect();
    format!("{{\n  \"mappings\": [\n{}\n  ]\n}}", rows.join(",\n"))
}

/// Turn a Map Local wildcard pattern into a regex for WireMock's `urlPattern`.
pub fn wildcard_regex(pattern: &str) -> String {
    let mut out = String::new();
    for ch in pattern.trim().chars() {
        match ch {
            '*' => out.push_str(".*"),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    if out.starts_with(".*") {
        out
    } else {
        format!(".*{}", out)
    }
}

// ---------------------------------------------------------------------------
// JSON: import / export for the studio files
// ---------------------------------------------------------------------------

pub fn tests_to_json(tests: &[ApiTest]) -> String {
    let rows: Vec<String> = tests
        .iter()
        .map(|test| {
            let headers = test
                .headers
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{{\"key\":\"{}\",\"value\":\"{}\"}}",
                        escape_json(k),
                        escape_json(v)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let injects = test
                .injects
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{{\"key\":\"{}\",\"value\":\"{}\"}}",
                        escape_json(k),
                        escape_json(v)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let scenario = test
                .scenario
                .map(|s| format!("\"{}\"", s.slug()))
                .unwrap_or_else(|| "null".into());
            format!(
                "  {{\n    \"name\": \"{}\",\n    \"suite\": \"{}\",\n    \"method\": \"{}\",\n    \"url\": \"{}\",\n    \"headers\": [{}],\n    \"body\": \"{}\",\n    \"scenario\": {},\n    \"scenario_target\": \"{}\",\n    \"injects\": [{}],\n    \"expect\": {{ \"status\": \"{}\", \"body_contains\": \"{}\", \"json_key\": \"{}\", \"header\": \"{}\", \"header_value\": \"{}\", \"max_ms\": \"{}\", \"source\": \"{}\" }}\n  }}",
                escape_json(&test.name),
                escape_json(&test.suite),
                escape_json(&test.method),
                escape_json(&test.url),
                headers,
                escape_json(&test.body),
                scenario,
                escape_json(&test.scenario_target),
                injects,
                escape_json(&test.expect_status),
                escape_json(&test.expect_body),
                escape_json(&test.expect_json_key),
                escape_json(&test.expect_header_name),
                escape_json(&test.expect_header_value),
                escape_json(&test.expect_max_ms),
                test.expect_source.slug()
            )
        })
        .collect();
    format!("[\n{}\n]", rows.join(",\n"))
}

pub fn tests_from_json(text: &str) -> Result<Vec<ApiTest>, String> {
    let root = Json::parse(text)?;
    let array = root.as_array().ok_or("Expected a JSON array of tests")?;
    let mut tests = Vec::new();
    for (index, item) in array.iter().enumerate() {
        let obj = item.as_object().ok_or("Each test must be an object")?;
        let name = obj
            .key("name")
            .and_then(Json::as_str)
            .unwrap_or("Imported test");
        let mut test = ApiTest::new(
            (index + 1) as u64,
            name,
            obj.key("suite").and_then(Json::as_str).unwrap_or("Imported"),
            obj.key("method").and_then(Json::as_str).unwrap_or("GET"),
            obj.key("url").and_then(Json::as_str).unwrap_or(""),
        );
        test.body = obj
            .key("body")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        test.scenario_target = obj
            .key("scenario_target")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        test.scenario = obj
            .key("scenario")
            .and_then(Json::as_str)
            .and_then(MockScenario::from_slug);
        if let Some(headers) = obj.key("headers").and_then(Json::as_array) {
            for header in headers {
                if let Some(header) = header.as_object() {
                    if let (Some(k), Some(v)) = (
                        header.key("key").and_then(Json::as_str),
                        header.key("value").and_then(Json::as_str),
                    ) {
                        test.headers.push((k.to_string(), v.to_string()));
                    }
                }
            }
        }
        if let Some(injects) = obj.key("injects").and_then(Json::as_array) {
            for inject in injects {
                if let Some(inject) = inject.as_object() {
                    if let (Some(k), Some(v)) = (
                        inject.key("key").and_then(Json::as_str),
                        inject.key("value").and_then(Json::as_str),
                    ) {
                        test.injects.push((k.to_string(), v.to_string()));
                    }
                }
            }
        }
        if let Some(expect) = obj.key("expect").and_then(Json::as_object) {
            test.expect_status = expect
                .key("status")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            test.expect_body = expect
                .key("body_contains")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            test.expect_json_key = expect
                .key("json_key")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            test.expect_header_name = expect
                .key("header")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            test.expect_header_value = expect
                .key("header_value")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            test.expect_max_ms = expect
                .key("max_ms")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            test.expect_source = ExpectSource::from_slug(
                expect.key("source").and_then(Json::as_str).unwrap_or("any"),
            );
        }
        tests.push(test);
    }
    Ok(tests)
}

pub fn plays_to_json(plays: &[ScreenPlay]) -> String {
    let rows: Vec<String> = plays
        .iter()
        .map(|play| {
            let steps = play
                .steps
                .iter()
                .map(|step| {
                    format!(
                        "{{\"kind\":\"{}\",\"action\":\"{}\",\"value\":\"{}\"}}",
                        step.kind.slug(),
                        escape_json(&step.action),
                        escape_json(&step.value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "  {{\n    \"name\": \"{}\",\n    \"goal\": \"{}\",\n    \"steps\": [{}]\n  }}",
                escape_json(&play.name),
                escape_json(&play.goal),
                steps
            )
        })
        .collect();
    format!("[\n{}\n]", rows.join(",\n"))
}

pub fn plays_from_json(text: &str) -> Result<Vec<ScreenPlay>, String> {
    let root = Json::parse(text)?;
    let array = root.as_array().ok_or("Expected a JSON array of screen plays")?;
    let mut plays = Vec::new();
    for (index, item) in array.iter().enumerate() {
        let obj = item.as_object().ok_or("Each play must be an object")?;
        let name = obj
            .key("name")
            .and_then(Json::as_str)
            .unwrap_or("Imported play");
        let mut play = ScreenPlay {
            id: (index + 1) as u64,
            name: name.to_string(),
            goal: obj
                .key("goal")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
            steps: Vec::new(),
            last: Vec::new(),
        };
        if let Some(steps) = obj.key("steps").and_then(Json::as_array) {
            for step in steps {
                if let Some(step) = step.as_object() {
                    play.steps.push(ScreenStep::new(
                        StepKind::from_slug(
                            step.key("kind").and_then(Json::as_str).unwrap_or("launch"),
                        ),
                        step.key("action").and_then(Json::as_str).unwrap_or(""),
                        step.key("value").and_then(Json::as_str).unwrap_or(""),
                    ));
                }
            }
        }
        plays.push(play);
    }
    Ok(plays)
}

pub fn escape_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// True when the JSON body contains the key anywhere. A dotted key such as
/// `data.balance` walks the path; a plain key searches the whole document.
pub fn json_key_present(body: &str, key: &str) -> Result<bool, String> {
    let parsed = Json::parse(body).map_err(|err| format!("Body is not JSON: {}", err))?;
    if key.contains('.') {
        let mut node = &parsed;
        for part in key.split('.') {
            let Some(next) = node.get(part) else {
                return Ok(false);
            };
            node = next;
        }
        Ok(true)
    } else {
        Ok(json_contains_key(&parsed, key))
    }
}

fn json_contains_key(value: &Json, key: &str) -> bool {
    match value {
        Json::Obj(entries) => entries
            .iter()
            .any(|(k, v)| k == key || json_contains_key(v, key)),
        Json::Arr(items) => items.iter().any(|v| json_contains_key(v, key)),
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// Look up a key on a parsed JSON object, kept separate from `Vec::get` so the
/// importers read like the document they parse.
trait JsonEntries {
    fn key(&self, name: &str) -> Option<&Json>;
}

impl JsonEntries for [(String, Json)] {
    fn key(&self, name: &str) -> Option<&Json> {
        self.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut parser = JsonParser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        parser.skip();
        let value = parser.value()?;
        parser.skip();
        if parser.pos != parser.bytes.len() {
            return Err("Unexpected text after the JSON value".into());
        }
        Ok(value)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, Json)>> {
        match self {
            Json::Obj(entries) => Some(entries),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn skip(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => Err("Expected a JSON value".into()),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(format!("Expected {}", word))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || matches!(ch, b'-' | b'+' | b'.' | b'e' | b'E') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "Bad number".to_string())?;
        raw.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| format!("Bad number: {}", raw))
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err("Expected a string".into());
        }
        self.pos += 1;
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            self.pos += 1;
            match ch {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(escaped) = self.peek() else {
                        return Err("Unfinished escape".into());
                    };
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let end = (self.pos + 4).min(self.bytes.len());
                            let hex = std::str::from_utf8(&self.bytes[self.pos..end])
                                .unwrap_or("0000");
                            let code = u32::from_str_radix(hex, 16).unwrap_or(0xFFFD);
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                            self.pos = end;
                        }
                        other => out.push(other as char),
                    }
                }
                other if other < 0x80 => out.push(other as char),
                _ => {
                    // Decode a multi-byte UTF-8 sequence.
                    let start = self.pos - 1;
                    let width = if ch & 0b1110_0000 == 0b1100_0000 {
                        2
                    } else if ch & 0b1111_0000 == 0b1110_0000 {
                        3
                    } else {
                        4
                    };
                    let end = (start + width).min(self.bytes.len());
                    let text = std::str::from_utf8(&self.bytes[start..end]).unwrap_or("");
                    out.push_str(text);
                    self.pos = end;
                }
            }
        }
        Err("Unterminated string".into())
    }

    fn array(&mut self) -> Result<Json, String> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err("Expected , or ] in an array".into()),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.pos += 1;
        let mut entries = Vec::new();
        self.skip();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(entries));
        }
        loop {
            self.skip();
            let key = self.string()?;
            self.skip();
            if self.peek() != Some(b':') {
                return Err("Expected : after an object key".into());
            }
            self.pos += 1;
            let value = self.value()?;
            entries.push((key, value));
            self.skip();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(entries));
                }
                _ => return Err("Expected , or } in an object".into()),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, pattern: &str, status: u16, body: &str) -> MapRule {
        MapRule {
            id: 1,
            name: name.into(),
            pattern: pattern.into(),
            status,
            body: body.into(),
            ..MapRule::default()
        }
    }

    #[test]
    fn fills_host_and_port_from_the_config() {
        assert_eq!(
            resolve_url("http://{host}:{port}/v1/wallet", 9090),
            "http://127.0.0.1:9090/v1/wallet"
        );
    }

    #[test]
    fn platform_bridge_is_only_needed_for_android() {
        assert_eq!(TargetPlatform::Mac.bridge_command(9090), None);
        assert_eq!(TargetPlatform::IosSimulator.bridge_command(9090), None);
        assert_eq!(TargetPlatform::IosDevice.bridge_command(9090), None);
        assert_eq!(
            TargetPlatform::AndroidEmulator.bridge_command(9090).as_deref(),
            Some("adb reverse tcp:9090 tcp:9090")
        );
    }

    #[test]
    fn physical_devices_choose_usb_or_lan_addresses() {
        assert_eq!(
            host_for_target(
                TargetPlatform::IosDevice,
                "192.168.1.20",
                AndroidTransport::UsbReverse
            ),
            "192.168.1.20"
        );
        assert_eq!(
            host_for_target(
                TargetPlatform::AndroidDevice,
                "192.168.1.20",
                AndroidTransport::UsbReverse
            ),
            "127.0.0.1"
        );
        assert_eq!(
            host_for_target(
                TargetPlatform::AndroidDevice,
                "192.168.1.20",
                AndroidTransport::WifiLan
            ),
            "192.168.1.20"
        );
        assert_eq!(
            bridge_command_for(
                TargetPlatform::AndroidDevice,
                9090,
                AndroidTransport::WifiLan
            ),
            None
        );
        assert_eq!(
            bridge_command_for(
                TargetPlatform::AndroidDevice,
                9090,
                AndroidTransport::UsbReverse
            )
            .as_deref(),
            Some("adb reverse tcp:9090 tcp:9090")
        );
    }

    #[test]
    fn checks_pass_and_fail_against_a_response() {
        let mut test = ApiTest::new(1, "wallet", "Wallet", "GET", "http://{host}:{port}/v1/wallet");
        test.expect_status = "200".into();
        test.expect_body = "\"balance\"".into();
        test.expect_json_key = "balance".into();
        test.expect_source = ExpectSource::Mock;
        let ok = TestResponse {
            status: 200,
            latency_ms: 12,
            headers: vec![("x-proxzy-rule".into(), "Wallet balance".into())],
            body: r#"{"balance":128.5}"#.into(),
            error: None,
        };
        let run = test.evaluate(&ok);
        assert!(run.passed);
        assert_eq!(run.checks.len(), 4);

        let bad = TestResponse {
            status: 500,
            latency_ms: 900,
            headers: Vec::new(),
            body: r#"{"error":"down"}"#.into(),
            error: None,
        };
        let run = test.evaluate(&bad);
        assert!(!run.passed);
        assert_eq!(run.checks.iter().filter(|c| !c.passed).count(), 4);
    }

    #[test]
    fn json_keys_can_be_nested_or_dotted() {
        let body = r#"{"data":{"wallet":{"balance":12,"items":[{"id":1}]}}}"#;
        assert!(json_key_present(body, "balance").unwrap());
        assert!(json_key_present(body, "data.wallet.balance").unwrap());
        assert!(json_key_present(body, "id").unwrap());
        assert!(!json_key_present(body, "missing").unwrap());
        assert!(json_key_present("not json", "x").is_err());
    }

    #[test]
    fn finds_the_rule_a_step_names() {
        let rules = vec![
            rule("Wallet balance", "/v1/wallet*", 200, "{}"),
            rule("Auth session", "/v1/auth", 200, "{}"),
        ];
        assert_eq!(rule_index_for_target(&rules, "Wallet balance"), Some(0));
        assert_eq!(rule_index_for_target(&rules, "Auth session"), Some(1));
        assert_eq!(
            rule_index_for_target(&rules, "https://api.test/v1/auth/login"),
            Some(1)
        );
        assert_eq!(rule_index_for_target(&rules, "unknown"), None);
    }

    #[test]
    fn inject_steps_parse_key_and_value() {
        assert_eq!(
            parse_inject("phone=0812345678"),
            Some(("phone".into(), "0812345678".into()))
        );
        assert_eq!(parse_inject("broken"), None);
    }

    #[test]
    fn test_suites_round_trip_through_json() {
        let tests = sample_tests();
        let json = tests_to_json(&tests);
        let parsed = tests_from_json(&json).unwrap();
        assert_eq!(parsed.len(), tests.len());
        assert_eq!(parsed[0].name, tests[0].name);
        assert_eq!(parsed[0].expect_json_key, "balance");
        assert_eq!(parsed[0].expect_header_name, "content-type");
        assert_eq!(parsed[3].expect_header_value, "");
        assert_eq!(parsed[2].body, tests[2].body);
        assert_eq!(parsed[3].injects[0].0, "USERNAME");
        assert_eq!(parsed[4].expect_source, ExpectSource::Live);
    }

    #[test]
    fn screen_plays_round_trip_through_json() {
        let plays = sample_screen_plays();
        let json = plays_to_json(&plays);
        let parsed = plays_from_json(&json).unwrap();
        assert_eq!(parsed.len(), plays.len());
        assert_eq!(parsed[0].name, plays[0].name);
        assert_eq!(parsed[0].steps[1].kind, StepKind::Scenario);
        assert_eq!(parsed[1].steps[1].value, "phone=0812345678");
    }

    #[test]
    fn wiremock_export_has_patterns_status_and_bodies() {
        let rules = vec![
            rule("Wallet balance", "/v1/wallet*", 200, r#"{"balance":1}"#),
            rule("Auth", "https://api.test/v1/auth", 401, "not json"),
        ];
        let out = wiremock_mappings(&rules);
        assert!(out.contains("\"mappings\""));
        assert!(out.contains(".*/v1/wallet.*"));
        assert!(out.contains("\"status\": 200"));
        assert!(out.contains("\"status\": 401"));
        assert!(out.contains("\"jsonBody\": {\"balance\":1}"));
        assert!(out.contains("\"jsonBody\": \"not json\""));
    }

    #[test]
    fn wildcard_regex_escapes_regex_characters() {
        assert_eq!(wildcard_regex("/v1/orders/*"), ".*/v1/orders/.*");
        assert_eq!(wildcard_regex("/v1/a.b"), ".*/v1/a\\.b");
    }

    #[test]
    fn env_and_snippets_use_the_configured_port() {
        let env = env_block(9092, "Wallet error recovery");
        assert!(env.contains("PROXZY_PORT=9092"));
        assert!(env.contains("PROXZY_SESSION=Wallet error recovery"));
        assert!(playwright_snippet(9092, "Wallet").contains("127.0.0.1:9092"));
        assert!(mcp_snippet(9092, "Wallet").contains("\"PROXZY_PORT\": \"9092\""));
        assert!(xctest_snippet(9092, "Wallet").contains("127.0.0.1:9092"));
        assert!(espresso_snippet(9092, "Wallet").contains("adb reverse tcp:9092"));
        assert!(inject_curl(9092, "Wallet balance", "phone", "081")
            .contains("127.0.0.1:9092/__proxzy/inject"));
        assert!(scenario_curl(9092, "Wallet balance", "error").contains("\"scenario\":\"error\""));
        assert!(health_curl(9092).contains("127.0.0.1:9092/__proxzy/health"));
        let lan_env = env_block_for("192.168.1.20", 9090, "Wallet", Some("proxzy_tok"));
        assert!(lan_env.contains("PROXZY_PROXY_URL=http://192.168.1.20:9090"));
        assert!(lan_env.contains("PROXZY_CONTROL_TOKEN=proxzy_tok"));
        assert!(health_curl_for("192.168.1.20", 9090, Some("proxzy_tok"))
            .contains("-H 'X-Proxzy-Control: proxzy_tok'"));
        assert!(xctest_snippet_for("192.168.1.20", 9090, "Wallet", Some("proxzy_tok"))
            .contains("http://192.168.1.20:9090"));
        assert!(espresso_snippet_for("192.168.1.20", 9090, "Wallet", None)
            .contains("http://192.168.1.20:9090"));
    }

    #[test]
    fn json_parser_handles_escapes_arrays_and_numbers() {
        let parsed = Json::parse(r#"{"a":[1,2,{"b":"x\"y"}],"ok":true}"#).unwrap();
        assert_eq!(parsed.get("ok"), Some(&Json::Bool(true)));
        let array = parsed.get("a").and_then(Json::as_array).unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(
            array[2].get("b").and_then(Json::as_str),
            Some("x\"y")
        );
        assert!(Json::parse("{oops}").is_err());
    }
}
