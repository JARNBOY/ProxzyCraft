# Ticket 05: Verification & Safety Test Suite

**Status:** Resolved  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** None  
**Target Files:** `app/src/model.rs`, `app/src/proxy.rs`

---

## 1. Goal
Provide automated test verification for domain pattern matching, passthrough streaming fidelity, and selective interception routing without starting the GUI or mutating system network settings.

## 2. Acceptance Criteria
- [x] Unit tests for `domain_matches`:
  - Exact match (`api.foo.com` matches `api.foo.com`).
  - Wildcard subdomain (`*.foo.com` matches `api.foo.com`, `auth.api.foo.com`, and `foo.com`).
  - Wildcard mismatch (`*.foo.com` does not match `notfoo.com` or `foo.org`).
  - Port handling (`api.foo.com:443` matches `*.foo.com`, `api.dev.local:8443` matches `api.dev.local:8443`).
  - Empty pattern and special characters (`*`, `*.*`, empty string).
- [x] ProxyConfig interception routing tests:
  - Default config has empty allowlist and never intercepts (`google.com`, `apple.com` pass through).
  - Selective interception only intercepts whitelisted targets.
  - Disabled ssl proxying passes all traffic without interception.
- [x] Invariant check: Run `cargo test` in a sandboxed test harness to verify all tests pass with zero regressions (19 tests passed, 0 failed).

