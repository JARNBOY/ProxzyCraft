# Ticket 03: Selective MITM Dispatch Pipeline

**Status:** Resolved  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** `01-domain-allowlist-model.md` (Resolved), `02-blind-tcp-passthrough.md` (Resolved)  
**Target Files:** `app/src/proxy.rs`

---

## 1. Goal
Integrate the domain allowlist check into `handle_connection` so that incoming `CONNECT` requests are routed to either `passthrough_tunnel` (Branch 1) or full TLS MITM termination via `tls_acceptor_for` (Branch 2).

## 2. Acceptance Criteria
- [x] In `handle_connection`:
  - Extract `(host, port)` from `target`.
  - Read `ctx.config` to evaluate `should_intercept(&host)`.
  - If `false`: Route to `passthrough_tunnel(stream, &host, port, ctx).await`.
  - If `true`: Proceed with `tls_acceptor_for(&host, &ctx.ca)`, accept TLS, and call `process_request`.
- [x] Ensure `process_request` continues to handle Map Local rules, Breakpoint rules, and Live Capture recording for whitelisted targets.
- [x] Record passthrough metrics/metadata cleanly with zero browser certificate alerts.

