# AGENTS.md — ProxzyCraft Workspace Instructions

Always read `SKILL.md` before operating the proxy, configuring MobileWright / `mobile-mcp` UI test mocks, or editing Rust source files in `app/src/`.

## Context Pointers

- **Agent Skill, Architecture Graph (`/understand`), Engine Rules & Workflows** — Read `SKILL.md` and `skills/proxzy-e2e/SKILL.md` when:
  - Automating mobile UI tests (`mobile-mcp` / MobileWright) via `http://127.0.0.1:9090/__proxzy/*` (`health`, `mock`, `scenario`, `record`, `inject`, `reset`).
  - Bootstrapping ProxzyCraft in any external project that asks for E2E response mocking:
    - Check health: `curl -s http://127.0.0.1:9090/__proxzy/health`
    - Run headless: `cargo run --release --manifest-path app/Cargo.toml -- --headless --auto-record="*.domain.com"`
    - Use universal client: `@proxzy/client` (located in `packages/proxzy-client`)
  - Resolving Cold-Start (no mock data yet): Enable Auto-Catch (`POST /__proxzy/record`) so live 200 OK responses are automatically sanitized and cached for offline replay.
  - Creating or switching 3-scenario Map Local mocks (`Success 200`, `Error 500`, `Empty 200`, or `passthrough` upstream).
  - Importing cURL commands, hooking direct URLs/domains without cURL, or saving live Composer responses as mocks.
  - Modifying the Rust codebase (`app/src/model.rs`, `app/src/proxy.rs`, `app/src/app.rs`, `app/src/views.rs`, `app/src/rules_ui.rs`).

## Critical Guardrails

- **Compilation Structure**: `app/src/views.rs` and `app/src/rules_ui.rs` are included inside `app/src/app.rs` via `include!("views.rs");` and `include!("rules_ui.rs");`. Never add `mod views;` or `mod rules_ui;` to `app/src/main.rs`.
- **Selective SSL Safe Default**: Keep `ProxyConfig::should_intercept` returning `false` when `ssl_allowlist` is empty so host browsers and non-target apps pass through untouched.
- **Verification**: Always verify changes with `cargo test --manifest-path app/Cargo.toml`. Never launch the blocking GUI (`cargo run`) or run `networksetup` to modify host macOS Wi-Fi proxy settings without explicit user confirmation.

