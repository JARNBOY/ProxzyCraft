# Contributing to ProxzyCraft

Thank you for contributing to **ProxzyCraft**! Everything in this repository is free, open-source under the [MIT License](LICENSE), and designed to run 100% locally.

Please read our [Code of Conduct](CODE_OF_CONDUCT.md) before participating.

---

## Local Development Prerequisites

- **Rust toolchain:** 1.75+ (2021 edition, `cargo`, `rustfmt`, `clippy`)
- **Node.js:** 22+ (for running `node --experimental-strip-types` smoke/E2E tests) and `npm` (for `@proxzy/client` typechecking & builds under `packages/proxzy-client`)
- **Optional device tooling:**
  - macOS with Xcode (`xcrun simctl`) for iOS Simulator integration
  - Android SDK Platform-Tools (`adb`) for Android Emulator/Device integration

---

## Codebase Architecture & Critical Guardrails

Before editing Rust source files in `app/src/`, keep these repository rules in mind (see also [`AGENTS.md`](AGENTS.md)):

1. **`include!` Module Structure in `app/src/app.rs`:**
   - `app/src/views.rs`, `app/src/rules_ui.rs`, `app/src/studio_ui.rs`, `app/src/tour_ui.rs`, `app/src/acp.rs`, and `app/src/acp_ui.rs` are included directly inside `app/src/app.rs` via `include!(...)`.
   - **Never** declare `mod views;`, `mod rules_ui;`, `mod studio_ui;`, `mod tour_ui;`, `mod acp;`, or `mod acp_ui;` in `app/src/main.rs`.
2. **Selective Interception Safe Default:**
   - Keep `ProxyConfig::should_intercept` returning `false` when `ssl_allowlist` is empty so unlisted host browsers and non-target applications pass through as raw tunnels untouched.
   - Preserve `BUILTIN_PROTECTED_DOMAINS` (APNs, OS updates, OCSP/CRLs, Play Services).
3. **No Host-Mutating Automated Tests:**
   - Never invoke `networksetup`, mutate host macOS keychains, or launch interactive GUI windows inside automated tests.
   - Prefer pure helper functions (like `app/src/browser.rs` launch spec builders) and loopback ephemeral ports (`127.0.0.1:0`) for live proxy/control-plane tests.
4. **Permissive License Hygiene:**
   - ProxzyCraft uses MIT-compatible dependencies only (see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)). Do not introduce copyleft (`GPL`, `AGPL`, `LGPL`, `MPL`) crates or runtime npm dependencies without maintainer approval.

---

## Running Tests & Verification

### 1. Rust Engine & GUI Tests
```bash
cargo test --manifest-path app/Cargo.toml
```
*(In restricted sandboxes where binding live TCP loopback sockets returns `EPERM`, you can run the unit/helper subset with `-- --skip practice_session_is_a_real_round_trip_through_the_gate --skip live_portal_and_control_security_over_tcp_socket`; run the full suite outside sandbox before submitting a PR.)*

### 2. TypeScript Client SDK (`@proxzy/client`)
```bash
npm --prefix packages/proxzy-client run typecheck
node --experimental-strip-types packages/proxzy-client/test/smoke.ts
```

### 3. Full Pre-Release Regression Gate
```bash
./scripts/regression-v1.sh
```
This runs the full Rust test suite, TypeScript checks and live headless E2E verification, clean-room export via `./scripts/export-clean-release.sh`, and secret/key hygiene verification.

---

## Pull Request & Release Hygiene

- Write focused tests first for behavioral changes and bug fixes.
- Keep user-facing terminology consistent with [`CONTEXT.md`](CONTEXT.md).
- **Never commit** private keys (`proxzy-ca-key.pem`), `control.token`, `mocks/` containing unredacted secrets, `target/`, or internal scratch files.

---

## Reporting Security Issues

Please **do not** open public GitHub issues for security vulnerabilities. Follow the private reporting instructions in [`SECURITY.md`](SECURITY.md).
