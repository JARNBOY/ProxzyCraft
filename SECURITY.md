# Security Policy & Threat Model

## Architecture Summary

ProxzyCraft is a **local-first debugging proxy and E2E network oracle** intended strictly for development, QA, and test automation on devices and domains you own or are authorized to inspect.

## Default Trust & Isolation Boundaries

1. **Loopback-Only Bind by Default (`BindPolicy::LoopbackOnly`)**:
   - Without `--lan` or the in-app **Allow LAN devices** toggle, the proxy binds strictly to `127.0.0.1:<port>`.
   - Remote machines on your Wi-Fi or corporate network cannot reach the proxy or control API.

2. **Empty SSL Allowlist Safe Default (`should_intercept == false`)**:
   - Unless domains are explicitly registered in the SSL allowlist (or via Map Local / `/__proxzy/record`), all `CONNECT` tunnels use **Raw TCP Data Binding** (`passthrough_tunnel`) without decrypting TLS.

3. **Built-in Protected System Domains (`BUILTIN_PROTECTED_DOMAINS`)**:
   - System push, OS updates, certificate revocation (OCSP/CRLs), and connectivity checks (e.g., `*.push.apple.com`, `swscan.apple.com`, `ocsp.*`, `play.googleapis.com`) are permanently excluded from TLS MITM decryption to prevent host OS instability.

4. **Control Plane Authentication & L7/TLS Isolation (`/__proxzy/*`)**:
   - `/__proxzy/*` routes are rejected inside TLS tunnels (`!is_tls`), preventing malicious websites from smuggling control-plane calls over HTTPS MITM.
   - `authorize_control` enforces:
     - Absence of browser `Origin` headers (zero wildcard CORS, blocking DNS rebinding / CSRF).
     - Strict `Host` verification (`127.0.0.1`, `localhost`, `::1`, `proxzy.local`, or active `advertised_ip`).
     - Kernel peer verification (`peer.ip().to_canonical().is_loopback()`) OR constant-time `X-Proxzy-Control` / `Authorization: Bearer` CSPRNG token (`proxzy_<64 hex chars>`) across both `GET` and `POST` in LAN mode.

5. **Targeted Anti-SSRF / Loopback-Bounce Guard (`resolve_upstream`)**:
   - Resolves upstream hostnames once before connecting (`tokio::net::lookup_host`).
   - Blocks self-listener bounces, link-local / metadata ranges (`169.254.0.0/16`, `fe80::/10`), unspecified addresses (`0.0.0.0/8`), and loopback destinations (`127.0.0.0/8`, `::1`) from non-loopback LAN callers — while preserving local `127.0.0.1` server debugging from the same host.

6. **Read-Only Onboarding Portal (`/`, `/ca.cer`, `/ca.pem`, `http://proxzy.local/`)**:
   - Exposes only the public root CA certificate (no private key, no rules, no logs, no control token) with `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`.

7. **Private Key & Secret Storage (`0o600`)**:
   - `proxzy-ca-key.pem` and `control.token` under `~/Library/Application Support/Proxzy/` are created with Unix mode `0o600` (owner read/write only) and excluded by `.gitignore`.
   - Captured mock fixtures are scrubbed via `sanitize_mock_payload` to redact bearer tokens, passwords, OTPs, phones, and IDs.

## Reporting a Vulnerability

Please **do not** open public GitHub issues for sensitive security vulnerabilities. Report privately to the repository maintainer via GitHub Security Advisories or private repository contact with reproduction details and impact scope.

