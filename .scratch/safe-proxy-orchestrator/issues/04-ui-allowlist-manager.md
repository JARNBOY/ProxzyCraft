# Ticket 04: UI Allowlist Manager & Safe Mode Controls

**Status:** Blocked  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** `01-domain-allowlist-model.md`  
**Target Files:** `app/src/rules_ui.rs`, `app/src/views.rs`, `app/src/app.rs`

---

## 1. Goal
Provide an egui interface for users to inspect, add, toggle, and delete SSL Proxying domains, and prevent accidental global network proxy activations.

## 2. Acceptance Criteria
- [ ] Add an "SSL Proxying" section in the Live Capture / Tools sidebar or Settings modal.
- [ ] Display the list of active SSL rules with toggle checkboxes (e.g. `*.myapi.com`, `api.github.com`).
- [ ] Provide an "Add Domain Rule" form:
  - Input for domain pattern.
  - Enabled/Disabled toggle.
- [ ] Add a prominent "Safe Mode: Active" badge when the allowlist is empty or restricted, informing the user:
  *"Unlisted domains pass through securely without TLS interception. Browsers will never encounter certificate warnings."*
- [ ] Revise the "Set Mac proxy" action in `views.rs` to display a confirmation dialog recommending targeted testing (e.g. `curl -x` or Simulator) over global system proxying.

