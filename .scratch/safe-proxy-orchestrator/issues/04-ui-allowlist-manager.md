# Ticket 04: UI Allowlist Manager & Safe Mode Controls

**Status:** Resolved  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** None  
**Target Files:** `app/src/rules_ui.rs`, `app/src/views.rs`, `app/src/app.rs`

---

## 1. Goal
Provide an egui interface for users to inspect, add, toggle, and delete SSL Proxying domains, and prevent accidental global network proxy activations.

## 2. Acceptance Criteria
- [x] Add an "SSL Proxying" section in the Live Capture / Tools sidebar or Settings modal (`live_ssl_allowlist_card` in `views.rs`).
- [x] Display the list of active SSL rules with delete/toggle controls (e.g. `*.shopwave.dev`, `api.dev.local:8443`).
- [x] Provide an "Add Domain Rule" form:
  - Input for domain pattern (`new_ssl_domain`).
  - Add button + quick sample preset button (`Sample API`).
- [x] Add a prominent "Safe Mode: Active" badge when the allowlist is empty or restricted, informing the user:
  *"Unlisted domains pass through securely without TLS interception. Browsers will never encounter certificate warnings."*
- [x] Revise the "Set Mac proxy" action in `views.rs` to display a confirmation dialog modal (`mac_proxy_confirm_modal`) recommending targeted testing (e.g. `curl -x` or Simulator) over global system proxying.

