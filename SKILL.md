---
name: proxzycraft
description: Operate, test, and extend the ProxzyCraft HTTP/HTTPS MITM proxy engine (127.0.0.1:9090), WireMock/Testful control hooks (/__proxzy/*), 3-scenario Map Local mocking (Success/Error/Empty), Direct URL/cURL import, Selective SSL Proxying allowlist, and MobileWright/mobile-mcp UI test workflows. Use when automating mobile UI tests against mocked or live APIs, injecting or switching mock scenarios, inspecting captured traffic, or modifying the ProxzyCraft Rust/egui/Tokio codebase.
---

# ProxzyCraft Agent Skill & Architecture Guide

ProxzyCraft is a native Rust (`eframe`/`egui` + `tokio` + `rustls` + `rcgen`) HTTP/1.1 & HTTPS MITM debugging proxy and WireMock/Testful mobile UI test engine listening on `127.0.0.1:9090` (with automatic fallback to `9091..=9093`).

---

## 1. Codebase Knowledge Graph (`/understand`)

```mermaid
flowchart TB
    subgraph Entry["CLI & Runtime Entry"]
        MAIN["app/src/main.rs<br/>CLI flags, screenshot mode, eframe::run_native"]
    end

    subgraph UI["Cupertino Desktop UI & State Machine"]
        APP["app/src/app.rs<br/>App struct, poll_proxy(), run_curl(), ensure_ssl_domain_for_pattern()"]
        VIEWS["app/src/views.rs<br/>(included in app.rs)<br/>Traffic table, Inspector, Composer, SSL Allowlist, Live Setup"]
        RULES_UI["app/src/rules_ui.rs<br/>(included in app.rs)<br/>Map Local v2, Direct URL/cURL bar, 3-Scenario pills, Breakpoints v2"]
        THEME["app/src/theme.rs<br/>Cupertino dark tokens, badges, vector icons"]
    end

    subgraph Domain["Domain Model & Pure Helpers"]
        MODEL["app/src/model.rs<br/>Capture, MapRule, MockScenario, BreakpointRule,<br/>parse_curl(), extract_domain_from_pattern(), mock_fixture_path()"]
        DEVICES["app/src/devices.rs<br/>xcrun simctl (iOS) & adb (Android) scanner + LAN IP"]
    end

    subgraph Engine["Async MITM & WireMock/Testful Engine"]
        PROXY["app/src/proxy.rs<br/>127.0.0.1:9090 listener, Dynamic Leaf Cert (rcgen),<br/>handle_proxzy_control (/__proxzy/*), Breakpoints, Map Local"]
    end

    MAIN --> APP
    APP -.->|"include!(\"views.rs\")"| VIEWS
    APP -.->|"include!(\"rules_ui.rs\")"| RULES_UI
    APP --> MODEL
    APP --> DEVICES
    APP --> THEME
    APP <-->|"ProxyConfig / mpsc::Receiver<ProxyEvent>"| PROXY
    PROXY --> MODEL
```

### File-to-Responsibility Map

| File | Primary Responsibility | Key Symbols |
| :--- | :--- | :--- |
| `app/src/main.rs` | Binary entry point, headless `--screenshot` / `PROXZY_VIEW` / `PROXZY_DEMO` flags, native window setup | `main` |
| `app/src/app.rs` | Central `App` state, proxy event loop sync, Composer `curl` worker, SSL auto-registration, rule modal state | `App`, `MapEditor`, `BpEditor`, `import_curl_to_composer`, `save_composer_as_mock`, `add_quick_direct_mock`, `ensure_ssl_domain_for_pattern` |
| `app/src/views.rs` | Included directly inside `app.rs` (`include!("views.rs")`). Traffic table, Inspector, Composer, Live Capture, SSL Allowlist, Logs, Guide | `traffic_page`, `composer_page`, `composer_result`, `ssl_proxying_page`, `live_page` |
| `app/src/rules_ui.rs` | Included directly inside `app.rs` (`include!("rules_ui.rs")`). Map Local v2, Direct URL/cURL Hook bar, 3-Scenario switcher pills, Breakpoints v2 | `map_local_page_v2`, `map_editor_modal_v2`, `breakpoints_page_v2`, `bp_editor_modal_v2` |
| `app/src/model.rs` | Pure data structures, 3-scenario state machine, session audit tracking, contract drift analyzer, cURL/URL parser, wildcard URL & domain matching, canonical fixture paths | `Capture`, `MapRule`, `MockScenario`, `TestSession`, `SessionCallAudit`, `ContractDriftReport`, `analyze_contract_drift`, `mock_versioned_fixture_path`, `ParsedCurl`, `parse_curl`, `pattern_matches_url`, `domain_matches` |
| `app/src/proxy.rs` | Async Tokio TCP/TLS listener on `127.0.0.1:9090`, on-the-fly `rcgen` leaf cert signer, `/__proxzy/*` HTTP control plane, session audit lifecycle, Map Local & Breakpoint execution | `ProxyHandle`, `ProxyConfig`, `ProxyEvent`, `handle_proxzy_control`, `matching_map_rule`, `handle_https_connect` |
| `app/src/devices.rs` | Background scanner for iOS Simulators (`xcrun simctl list devices -j`) and Android devices (`adb devices -l`) | `DeviceManager`, `MobileDevice`, `DevicePlatform` |
| `app/src/theme.rs` | Color palette, method/status pills, macOS traffic lights, custom painted icons | `C`, `pill`, `method_badge`, `status_badge`, `toggle` |

---

## 2. Core Engine Rules & Invariants

Follow these 5 invariants whenever operating or modifying ProxzyCraft:

1. **Selective SSL Proxying Safe Default (`ssl_allowlist`)**:
   - `ProxyConfig::should_intercept(host)` returns `false` when `ssl_proxying_enabled == false` **or** when `ssl_allowlist` is empty.
   - Non-matching hosts use blind TCP passthrough (`tokio::io::copy_bidirectional`) so host browsers, OAuth providers, and Apple/Google system services never suffer TLS errors.
2. **Automatic SSL Allowlist Sync on Mock Creation**:
   - Whenever a Map Local rule is created from a URL or pattern, extract its host with `extract_domain_from_pattern(pattern)` and register it in `ssl_allowlist` (handled automatically in UI via `App::ensure_ssl_domain_for_pattern` and in `/__proxzy/mock` via `handle_proxzy_control`).
3. **3-Scenario Mock State Machine (`Success` / `Error` / `Empty` / `Passthrough`)**:
   - Every `MapRule` stores three independent slots:
     - `MockScenario::Success` (`success_status` default `200`, `success_body`)
     - `MockScenario::Error` (`error_status` default `500`, `error_body`)
     - `MockScenario::Empty` (`empty_status` default `200`, `empty_body`)
   - Calling `rule.switch_scenario(target)` persists edits from the current `status`/`body` into the outgoing scenario slot before loading `target` into `rule.status` and `rule.body`.
   - Setting `scenario` to `"passthrough"` via `/__proxzy/scenario` disables the mock rule (`rule.enabled = false`) while keeping the domain in `ssl_allowlist`, allowing real upstream production traffic to flow and be captured.
4. **Canonical Fixture Path Convention & Static Security Redaction**:
   - Always compute fixture paths via `mock_versioned_fixture_path(env, version, method, pattern, scenario)`:
     ```text
     mocks/<env>/v<version>/<domain>/<method>_<path_slug>/<scenario>.json
     ```
     Example: `env="preprod"`, `version="1.1.0"`, `POST https://api.sample.com/v2/auth/login*` $\to$ `mocks/preprod/v1.1.0/api.sample.com/post_v2_auth_login/success.json`.
     *(Fallback: If `env` and `version` are empty, defaults to unversioned `mocks/<domain>/<method>_<path_slug>/<scenario>.json` for 100% backward compatibility).*
   - **Static Security Audit Guard (`sanitize_mock_payload`)**:
     Whenever real network traffic is saved into a mock rule or ingested via `/__proxzy/mock`, `sanitize_mock_payload` automatically redacts Authorization headers, session tokens, passwords, 10-digit phone numbers, and citizen IDs into parameterized placeholders (`{{TOKEN}}`, `{{USERNAME}}`, `{{OTP}}`). Mocks are 100% PII-safe to commit and share.
   - **Versioned API Contract Drift Classification (`analyze_contract_drift`)**:
     When running regression tests against real upstream backends (`passthrough` mode), ProxzyCraft compares live response schema with baseline mock schema:
     - `BREAKING:` Live API omitted or renamed an expected mock field &rarr; potential mobile app crash.
     - `ADDITIVE:` Live API returned new fields not yet present in baseline mock &rarr; backward-compatible enhancement.
     Session audits track `breaking_drifts` and `additive_drifts` with version stamps (`app_version`, `baseline_version`, `env`).
5. **Parameterized Mocking & Dynamic Property Injection (`inject`)**:
   - Every `MapRule` supports `inject_props` (`HashMap<String, String>`).
   - On every request intercept, `rule.render_injected_body()` dynamically replaces `{{KEY}}` placeholders and JSON key values (e.g. `phone`, `username`, `otp`, `token`, `tier`) on-the-fly, allowing E2E tests to inject dynamic user credentials (e.g. from `TestUsers.ts`) without mutating base fixtures.
6. **Modular Folders & Batch Scenario Controls**:
   - Mocks are grouped into collapsible module folders (`Auth`, `Wallet`, `Home`, `Watch`, `Profile`, `General`).
   - Batch controls allow switching all rules within a folder to `Success`, `Error`, `Empty`, or `Passthrough` in 1 click or via `POST /__proxzy/scenario {"folder": "..."}`.
7. **Loopback & Host Security Guard**:
   - `handle_proxzy_control` enforces loopback host validation (`127.0.0.1`, `localhost`, `[::1]`) and blocks untrusted cross-origin requests with HTTP 403 Forbidden.
8. **Compilation & Host Safety Guardrail**:
   - `views.rs` and `rules_ui.rs` are textually included at the bottom of `app/src/app.rs` via `include!("views.rs");` and `include!("rules_ui.rs");`. Keep them as included files rather than adding `mod views;` or `mod rules_ui;` in `main.rs`.
   - Verify code changes exclusively with `cargo test --manifest-path app/Cargo.toml`. Never launch the blocking desktop GUI (`cargo run`) or run `networksetup` to mutate host macOS Wi-Fi proxy settings during automated agent runs.

---

## 3. Agent Workflows

### Workflow 1: Autonomous Mobile UI Test Mocking & Verification (`mobile-mcp` / MobileWright)

Use the in-band HTTP control plane on `http://127.0.0.1:9090/__proxzy/*` or the zero-dependency `@proxzy/client` SDK to seed mocks, switch scenarios (`success` / `error` / `empty` / `passthrough`), auto-catch live 200 responses, and verify request hit counts during automated mobile UI tests.

1. **Auto-Catch Live 200 OK Responses (Solving Cold Start for Any Project)**:
   - When running a test against staging/preprod without existing mock fixtures:
   ```bash
   curl -s -X POST http://127.0.0.1:9090/__proxzy/record \
     -H "Content-Type: application/json" \
     -d '{"enabled": true, "domains": ["*.example.com", "api.sample.com"]}'
   ```
   Or via CLI flag when starting daemon: `proxzy --headless --auto-record="*.example.com,api.sample.com"`
   - ProxzyCraft intercepts live 200..=299 responses, runs `sanitize_mock_payload` (redacting tokens, passwords, phones, citizen IDs), generates an empty state via `auto_derive_empty_scenario`, creates an environment-agnostic wildcard pattern, and saves the mock rule. Subsequent test runs hit the mock offline with zero network latency.

2. **Inspect Engine Health & Hit Counters**:
   ```bash
   curl -s http://127.0.0.1:9090/__proxzy/health
   ```
   Returns JSON containing `status`, `auto_record`, `auto_record_domains`, `map_enabled`, `ssl_proxying_enabled`, `ssl_allowlist`, and `rules` (including live `hits` and `active_scenario` per rule).

3. **Upsert a Multi-Scenario Mock Rule (`POST /__proxzy/mock`)**:
   - Send either a `pattern` (Direct URL / wildcard) **or** a `curl` string (raw cURL command).
   - Provide `scenario` (`"success"`, `"error"`, or `"empty"`), `status`, `body`, and optional slot overrides (`success_body`, `error_body`, `empty_body`).
   - ProxzyCraft automatically enables `map_enabled`, adds the extracted domain to `ssl_allowlist`, and enables `ssl_proxying_enabled`.
   ```bash
   curl -s -X POST http://127.0.0.1:9090/__proxzy/mock \
     -H "Content-Type: application/json" \
     -d '{
       "name": "Wallet Balance API",
       "method": "GET",
       "pattern": "https://api.example.com/v1/wallet/balance*",
       "scenario": "success",
       "status": 200,
       "body": "{\"balance\": 9950.00, \"currency\": \"THB\"}",
       "error_status": 500,
       "error_body": "{\"error\": \"upstream_timeout\", \"message\": \"Service unavailable\"}",
       "empty_status": 200,
       "empty_body": "{\"balance\": 0, \"items\": []}"
     }'
   ```
4. **Switch Scenario Dynamically Between UI Test Assertions (`POST /__proxzy/scenario`)**:
   - Switch any matching rule to `"success"`, `"error"`, `"empty"`, or `"passthrough"` (real upstream network):
   ```bash
   curl -s -X POST http://127.0.0.1:9090/__proxzy/scenario \
     -H "Content-Type: application/json" \
     -d '{
       "pattern": "https://api.example.com/v1/wallet/balance*",
       "scenario": "error"
     }'
   ```
5. **Trigger Mobile Screen Action & Verify Expectation**:
   - Drive the mobile screen via `mobile-mcp` / MobileWright (tap, swipe, pull-to-refresh).
   - Query `GET http://127.0.0.1:9090/__proxzy/health` and assert that the target rule's `hits` incremented and the mobile UI rendered the expected state.
6. **Reset Counters or Clear Rules After Test Suite (`POST /__proxzy/reset`)**:
   ```bash
   # Reset hit counters only (keep rules):
   curl -s -X POST http://127.0.0.1:9090/__proxzy/reset \
     -H "Content-Type: application/json" \
     -d '{"clear_rules": false}'
   ```

**Completion criterion**: `GET /__proxzy/health` reports the expected `active_scenario` and incremented `hits` count matching the mobile UI test flow.

---

### Workflow 2: Direct URL / Domain Rule Hook (No cURL Available)

When setting up a mock for an endpoint before any request is captured and no cURL command is available:

1. **In the UI (Map Local Page)**:
   - Paste the direct URL or wildcard pattern (e.g., `https://api.example.com/v1/profile*` or `api.example.com/v1/orders`) into the **DIRECT URL / cURL HOOK** bar at the top of **Map Local**.
   - Select the target initial scenario pill (`Success`, `Error`, or `Empty`).
   - Click **`+ Quick Mock`** to create the rule immediately (auto-registering `api.example.com` into `ssl_allowlist`) or **`Edit Mock...`** to customize the response body across all 3 scenario tabs (`Success 200`, `Error 500`, `Empty 200`).
2. **Programmatically via Hook**:
   - Call `POST http://127.0.0.1:9090/__proxzy/mock` with `"pattern": "https://api.example.com/v1/profile*"` and `"scenario": "success"`.

**Completion criterion**: The rule appears in `map_rules` with `enabled: true`, its domain is present in `ssl_allowlist`, and `pattern_matches_url` matches incoming device requests.

---

### Workflow 3: Paste cURL Import & Live Upstream Capture-to-Mock

When importing a request from Postman/Chrome/logs, testing the real upstream response, and saving it as a mock fixture:

1. **Open Composer (`Tab::Composer`)**:
   - Paste a raw `curl ...` command (multiline `\`, `-X`, `-H`, `-d`, `--data-raw`, `--json`) or a direct URL into the **PASTE cURL / DIRECT URL** bar.
2. **Execute Upstream Request (`Import & Send`)**:
   - Click **`Import & Send`** to populate Method, URL, Headers, and Body and execute the real request via `run_curl`.
3. **Save Live Response as Mock (`Save Response as Mock`)**:
   - In the **Response** pane header, choose the target scenario slot (`Success`, `Error`, or `Empty`) and click **`Save Response as Mock`**.
   - ProxzyCraft creates or updates the corresponding `MapRule` with the real upstream HTTP status and payload, auto-adds the domain to `ssl_allowlist`, and displays the canonical fixture path (`mocks/<domain>/<method>_<path>/<scenario>.json`).

**Completion criterion**: `save_composer_as_mock` creates or updates the `MapRule` with the live response status/body and registers the domain in `ssl_allowlist`.

---

### Workflow 4: Modifying & Verifying the ProxzyCraft Codebase (`/implement-spec`)

When adding features or fixing bugs in the Rust codebase:

1. **Locate the Target Seam**:
   - Pure parsing, URL/domain matching, or scenario state logic $\to$ `app/src/model.rs`.
   - Proxy interception, TLS MITM, or `/__proxzy/*` control endpoints $\to$ `app/src/proxy.rs`.
   - Application actions & state transitions $\to$ `app/src/app.rs`.
   - Traffic/Composer/Allowlist views $\to$ `app/src/views.rs`.
   - Map Local / Breakpoint cards & modals $\to$ `app/src/rules_ui.rs`.
2. **Write Unit Tests alongside Implementation**:
   - Add unit tests in the `#[cfg(test)] mod tests` block of `app/src/model.rs` or `app/src/proxy.rs`.
3. **Run the Automated Test Suite**:
   ```bash
   cargo test --manifest-path app/Cargo.toml
   ```

**Completion criterion**: `cargo test --manifest-path app/Cargo.toml` compiles cleanly with zero errors and passes 100% of unit and integration tests.

---

### Workflow 5: Mobile-Wright E2E Testing with ProxzyCraft (`mobile-app-e2e`)

When integrating Mobile-Wright automated UI tests with ProxzyCraft in any test repository:

1. **Import Proxzy Fixture**:
   In any spec file in `tests/<feature>/<name>.spec.ts`:
   ```ts
   import { test, expect } from '@proxzy/client/fixture'
   import { getTestUser } from '../fixtures/users'
   ```
2. **Inject Dynamic User & Switch Scenarios**:
   ```ts
   test('Verifies Wallet error and recovery states', async ({ proxzy }) => {
     // 1. Inject TestUser (phone 0812345678, OTP 111111) dynamically:
     const user = getTestUser('basic')
     await proxzy.injectUser({ folder: 'Auth' }, user)

     // 2. Set Wallet module to Error scenario:
     await proxzy.setFolderScenario('Wallet', 'error')

     // 3. Execute mobile UI steps & assert error view:
     // ...
     await proxzy.expectHits('wallet/balance', 1)

     // 4. Recover to Success scenario in 1 call:
     await proxzy.setFolderScenario('Wallet', 'success')
     // ...
   })
   ```
3. **Verify Quality Gates**:
   ```bash
   npm run typecheck
   npm run lint
   npm test
   ```

**Completion criterion**: All quality gates pass with 0 warnings, zero PII or credentials are hardcoded into Git or mock files, and Mobile-Wright assertions pass against mock/upstream responses.

---

### Workflow 6: Universal Integration for ANY MobileWright, Playwright, or AI Agent Project (`@proxzy/client`)

To integrate ProxzyCraft into ANY mobile automation project (iOS/Android simulators or real devices):

1. **Install or Link Client**:
   ```bash
   npm install @proxzy/client
   # Or link locally from monorepo/submodule:
   # npm link ../ProxzyCraft/packages/proxzy-client
   ```
2. **Start ProxzyCraft Daemon**:
   ```bash
   # In CI or local test runner:
   cargo run --release --manifest-path path/to/ProxzyCraft/app/Cargo.toml -- --headless --auto-record="*.domain.com"
   ```
3. **Configure Simulator / Device Wi-Fi Proxy**:
   Point HTTP & HTTPS proxy to `127.0.0.1:9090` (or host LAN IP `192.168.x.x:9090` for physical devices).
4. **Drive Test Suite via TypeScript**:
   ```ts
   import { ProxzyClient } from '@proxzy/client';
   const proxzy = new ProxzyClient();

   // Seed mock or switch scenario:
   await proxzy.setScenario('v1/feed', 'error');

   // Tap in app:
   await mobile.tap('#refresh');

   // Verify network interception:
   await proxzy.expectHits('v1/feed', 1);
   ```



