# Ticket 05: Verification & Safety Test Suite

**Status:** Blocked  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** `03-selective-mitm-dispatch.md`, `04-ui-allowlist-manager.md`  
**Target Files:** `app/src/proxy.rs`, `app/tests/`

---

## 1. Goal
Provide automated test verification for domain pattern matching, passthrough streaming fidelity, and selective interception routing without starting the GUI or mutating system network settings.

## 2. Acceptance Criteria
- [ ] Unit tests for `domain_matches`:
  - Exact match (`api.foo.com` matches `api.foo.com`).
  - Wildcard subdomain (`*.foo.com` matches `api.foo.com`, `auth.api.foo.com`, and `foo.com`).
  - Wildcard mismatch (`*.foo.com` does not match `notfoo.com` or `foo.org`).
  - Port handling (`api.foo.com:443` matches `*.foo.com`).
  - Empty pattern and special characters.
- [ ] Integration test with Tokio mock TCP streams:
  - Non-whitelisted CONNECT returns `200 Connection Established` and pipes bytes without TLS error.
  - Whitelisted CONNECT proceeds to TLS handshake.
- [ ] Invariant check: Run `cargo test` in a sandboxed test harness to verify all tests pass with zero regressions to Map Local and Breakpoints.

