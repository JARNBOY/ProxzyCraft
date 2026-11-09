# Feature Specification: Safe Selective Proxy & System Orchestrator (ProxzyCraft)

**Status:** Ready for Implementation  
**Feature Slug:** `safe-proxy-orchestrator`  
**Target Codebase:** `app/src/` (Rust + Tokio + Rustls + egui)  
**Derived From:** `/arena` synthesis (Proxyman & mitmproxy selective interception pattern)

---

## 1. Problem Statement & Background

`ProxzyCraft` currently terminates TLS unconditionally on every incoming `HTTP CONNECT target:443` request. When macOS services, public browsers (Chrome, Safari), or apps with Certificate Pinning (Spotify, Slack, iCloud) route traffic through the proxy, they encounter a dynamically minted leaf certificate signed by `Proxzy Local CA`. Because this local CA is untrusted by host browsers and system root stores, browsers immediately block requests with `NET::ERR_CERT_AUTHORITY_INVALID` or `ERR_SSL_PROTOCOL_ERROR`.

Furthermore, blanket `networksetup` scripts force the entire Mac's network traffic through the local proxy, leading to system-wide connection failures and browser lockout.

## 2. The Core Solution (The /arena Synthesis)

We adopt the industry-proven architecture used by **Proxyman** and **mitmproxy**:

1. **Selective SSL Proxying (Default: Safe Passthrough):**
   - The proxy maintains an active **SSL Interception Allowlist** (e.g. `["*.myapi.com", "api.github.com"]`).
   - Default state on launch: **Empty Allowlist** (Safe Mode).
   - If a target domain does **NOT** match the allowlist:
     - The proxy performs a **Blind TCP Passthrough (Stream Splice)**.
     - Proxzy connects a raw TCP stream to `target:443`, sends `HTTP/1.1 200 Connection Established`, and bidirectionally copies bytes via `tokio::io::copy_bidirectional`.
     - Zero TLS termination occurs; the browser and server negotiate TLS directly. Public browsers remain 100% happy with zero certificate errors.
   - If a target domain **DOES** match the allowlist:
     - Proxzy terminates TLS with `rcgen` dynamic leaf certificate minting.
     - The request is decrypted, fed through the **Map Local** and **Breakpoint** rule pipeline, and recorded to the Live Traffic feed.

2. **Scoped Device & Target Orchestration:**
   - Keep simulator certificate installation scoped to the simulator's private keychain (`xcrun simctl keychain <UDID> add-root-cert`).
   - Disallow silent or blanket macOS `networksetup` overrides. Replace with explicit confirmation and per-process instructions (`export http_proxy=...`).

---

## 3. System Invariants

1. **Browser Protection Invariant:** Any HTTPS request to a domain not explicitly present in the SSL Allowlist MUST bypass TLS termination completely. The proxy must never present a local CA certificate to an unwhitelisted domain.
2. **Data Pipeline Invariant:** Plain HTTP traffic and whitelisted HTTPS traffic must continue to pass through the Map Local, Breakpoints, and Capture pipelines without regression.
3. **Graceful Failure Invariant:** If an upstream connection fails during passthrough, Proxzy must close the client connection cleanly without panicking Tokio worker tasks.

---

## 4. Architectural Task Graph

```
[Ticket 01: Domain Allowlist Model]
        │
        ├──────────────────────────┐
        ▼                          ▼
[Ticket 02: Blind TCP Passthrough]  [Ticket 04: UI Allowlist Manager]
        │                                  │
        ▼                                  │
[Ticket 03: Selective MITM Dispatch]       │
        │                                  │
        └─────────────────┬────────────────┘
                          ▼
[Ticket 05: Verification & Safety Test Suite]
```

---

## 5. Frontier & Dependencies

| Ticket ID | Title | Blocked By | Status |
| :--- | :--- | :--- | :--- |
| `01` | [Domain Allowlist & Matching Data Model](issues/01-domain-allowlist-model.md) | None (Initial Frontier) | Open |
| `02` | [Blind TCP Passthrough Engine](issues/02-blind-tcp-passthrough.md) | `01` | Blocked |
| `03` | [Selective MITM Dispatch Pipeline](issues/03-selective-mitm-dispatch.md) | `01`, `02` | Blocked |
| `04` | [UI Allowlist Manager & Safe Mode Controls](issues/04-ui-allowlist-manager.md) | `01` | Blocked |
| `05` | [Verification & Safety Test Suite](issues/05-verification-test-suite.md) | `03`, `04` | Blocked |

