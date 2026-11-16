# Security Policy & Threat Model

## Architecture Summary

ProxzyCraft is a **local-first debugging proxy and E2E network oracle** intended strictly for development, QA, and test automation on devices and domains you own or are authorized to inspect. See [`PRIVACY.md`](PRIVACY.md) for local-only data handling and Root CA removal instructions.

## Default Trust & Isolation Boundaries

1. **Loopback-Only Bind by Default (`BindPolicy::LoopbackOnly`)**:
   - Without `--lan` or the in-app **Allow LAN devices** toggle, the proxy binds strictly to `127.0.0.1:<port>`.
   - Remote machines on your Wi-Fi or corporate network cannot reach the proxy or control API.

2. **Isolated Browser Session Boundary (`app/src/browser.rs`)**:
   - Desktop browser HTTPS debugging defaults to launching an **Isolated Browser Session** with explicit least-privilege flags:
     - `--proxy-server=http://127.0.0.1:<port>` (routes only that browser instance through ProxzyCraft; never mutates `networksetup` or host browsers)
     - `--user-data-dir=<temp-profile>` (isolates cookies, storage, and flags from your personal browser profile)
     - `--ignore-certificate-errors-spki-list=<BASE64_SHA256_SPKI>` (pins trust strictly to the local CA's Subject Public Key Info hash instead of trusting a Root CA in the global OS keychain or disabling TLS verification with blanket `--ignore-certificate-errors`)

3. **Empty SSL Allowlist Safe Default (`should_intercept == false`)**:
   - Unless domains are explicitly registered in the SSL allowlist (or via Map Local / `/__proxzy/record`), all `CONNECT` tunnels use **Raw TCP Data Binding** (`passthrough_tunnel`) without decrypting TLS.

4. **Built-in Protected System Domains (`BUILTIN_PROTECTED_DOMAINS`)**:
   - System push, OS updates, certificate revocation (OCSP/CRLs), and connectivity checks (e.g., `*.push.apple.com`, `swscan.apple.com`, `ocsp.*`, `play.googleapis.com`) are permanently excluded from TLS MITM decryption to prevent host OS instability.

5. **Control Plane Authentication & L7/TLS Isolation (`/__proxzy/*`)**:
   - `/__proxzy/*` routes are rejected inside TLS tunnels (`!is_tls`), preventing malicious websites from smuggling control-plane calls over HTTPS MITM.
   - `authorize_control` enforces:
     - Absence of browser `Origin` headers (zero wildcard CORS, blocking DNS rebinding / CSRF).
     - Strict `Host` verification (`127.0.0.1`, `localhost`, `::1`, `proxzy.local`, `proxzy.internal`, or active `bind_ip` / `advertised_ip`).
     - Kernel peer verification (`peer.ip().to_canonical().is_loopback()`) OR constant-time `X-Proxzy-Control` / `Authorization: Bearer` CSPRNG token (`proxzy_<64 hex chars>`) across both `GET` and `POST` in LAN mode.

6. **Targeted Anti-SSRF / Loopback-Bounce Guard (`resolve_upstream`)**:
   - Resolves upstream hostnames once before connecting (`tokio::net::lookup_host`).
   - Blocks self-listener bounces, link-local / metadata ranges (`169.254.0.0/16`, `fe80::/10`), unspecified addresses (`0.0.0.0/8`), and loopback destinations (`127.0.0.0/8`, `::1`) from non-loopback LAN callers — while preserving local `127.0.0.1` server debugging from the same host.

7. **Read-Only Onboarding Portal (`/`, `/ca.cer`, `/ca.pem`, `http://proxzy.internal/`, `http://proxzy.local/`)**:
   - Both the ICANN-reserved private-use `.internal` hostname (`http://proxzy.internal/`) and legacy `http://proxzy.local/` (alongside `127.0.0.1` / `localhost` / active LAN IP) serve the read-only onboarding portal.
   - Exposes only the public root CA certificate (no private key, no rules, no logs, no control token) with `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`.

8. **Private Key & Secret Storage (`0o600`)**:
   - `proxzy-ca-key.pem` and `control.token` under `~/Library/Application Support/Proxzy/` are created with Unix mode `0o600` (owner read/write only) and excluded by `.gitignore`.
   - Captured mock fixtures are scrubbed via `sanitize_mock_payload` to redact bearer tokens, passwords, OTPs, phones, and IDs.

## Unsigned Open-Source Distribution & Gatekeeper Transparency

- **Zero-cost open-source distribution:** ProxzyCraft v1.0.0 is distributed at **$0** under the MIT license via GitHub source builds (`cargo build --release --manifest-path app/Cargo.toml`) and direct GitHub Releases binaries with published SHA-256 checksums.
- **No Apple Notarization / Mac App Store:** Prebuilt macOS binaries attached to GitHub Releases are **unsigned / non-notarized** open-source builds (ProxzyCraft does not use a paid Apple Developer ID certificate and is not distributed through the Mac App Store because mandatory App Sandbox restrictions conflict with developer tooling such as `xcrun simctl`, `adb`, and opt-in `networksetup` proxy switching).
- **Verification & Gatekeeper guidance:**
  - Verify release archive SHA-256 checksums against the release `SHA256SUMS.txt` before running prebuilt binaries, or compile directly from source (`cargo build --release --manifest-path app/Cargo.toml`).
  - On macOS, if Gatekeeper warns about an unsigned downloaded binary, either compile from source or explicitly allow it via *System Settings → Privacy & Security → Open Anyway* only after verifying the release checksum.

## Reporting a Vulnerability

Please **do not** open public GitHub issues for sensitive security vulnerabilities. Report privately to the repository maintainer via GitHub Security Advisories or private repository contact with reproduction details and impact scope.
