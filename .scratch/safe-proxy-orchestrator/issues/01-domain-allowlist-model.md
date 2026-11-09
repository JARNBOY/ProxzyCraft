# Ticket 01: Domain Allowlist & Matching Data Model

**Status:** Open (Frontier)  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** None  
**Target Files:** `app/src/model.rs`, `app/src/proxy.rs`

---

## 1. Goal
Introduce the data structures and matching logic for SSL Proxying rules. Define how hostnames are compared against patterns including exact matches (`api.dev.com`), wildcard subdomains (`*.dev.com`), port specifications (`localhost:8080`), and wildcard catch-all (`*`).

## 2. Acceptance Criteria
- [ ] `ProxyConfig` in `app/src/proxy.rs` is updated to include:
  - `pub ssl_proxying_enabled: bool` (default: `true`)
  - `pub ssl_allowlist: Vec<SslRule>` or `Vec<String>`
- [ ] Implement `domain_matches(pattern: &str, target_host: &str) -> bool`:
  - `*` matches everything.
  - `*.example.com` matches `api.example.com`, `v2.api.example.com`, and `example.com`.
  - `api.example.com` matches only `api.example.com` (case-insensitive).
  - Handles hosts with or without port (e.g. `example.com:443`).
- [ ] Implement `ProxyConfig::should_intercept(&self, host: &str) -> bool`:
  - If `!ssl_proxying_enabled`, returns `false`.
  - If `ssl_allowlist` is empty, returns `false` (Safe Default).
  - If matched, returns `true`.
- [ ] Unit tests in `model.rs` / `proxy.rs` verify exact, wildcard, and mismatch cases.

