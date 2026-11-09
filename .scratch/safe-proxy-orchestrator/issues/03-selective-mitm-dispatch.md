# Ticket 03: Selective MITM Dispatch Pipeline

**Status:** Blocked  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** `01-domain-allowlist-model.md`, `02-blind-tcp-passthrough.md`  
**Target Files:** `app/src/proxy.rs`

---

## 1. Goal
Integrate the domain allowlist check into `handle_connection` so that incoming `CONNECT` requests are routed to either `passthrough_tunnel` (Branch 1) or full TLS MITM termination via `tls_acceptor_for` (Branch 2).

## 2. Acceptance Criteria
- [ ] In `handle_connection`:
  - Extract `(host, port)` from `target`.
  - Read `ctx.config` to evaluate `should_intercept(&host)`.
  - If `false`: Route to `passthrough_tunnel(stream, &host, port, ctx).await`.
  - If `true`: Proceed with `tls_acceptor_for(&host, &ctx.ca)`, accept TLS, and call `process_request`.
- [ ] Ensure `process_request` continues to handle Map Local rules, Breakpoint rules, and Live Capture recording for whitelisted targets.
- [ ] Record passthrough metrics/metadata as a minimal capture row (or skipped from decrypted inspect view, badged `PASSTHROUGH`).

