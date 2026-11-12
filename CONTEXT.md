# ProxzyCraft

ProxzyCraft is a local HTTP/HTTPS interception proxy and deterministic API mock studio for desktop browsers, mobile simulators, and physical devices.

## Language

### Proxy Engine & Security Boundary

**Bind Policy**:
The listener exposure policy (`LoopbackOnly` by default, or explicit opt-in `Lan` with an advertised host IP and control token).
_Avoid_: Expose all, public proxy, open port

**Control Plane**:
The in-band HTTP control endpoints under `/__proxzy/*` (`health`, `status`, `record`, `session/*`, `inject`, `mock`, `scenario`, `reset`) gated to loopback or a valid `X-Proxzy-Control` token and denied inside tunneled HTTPS sessions.
_Avoid_: Admin API, webhook server, backdoor

**Control Token**:
The per-instance secret saved with `0600` permissions in `control.token` that authorizes remote or LAN `/__proxzy/*` control requests when LAN listening is enabled.
_Avoid_: API key, password, JWT

**Certificate Portal**:
The read-only provisioning endpoints (`/`, `/ca.pem`, `/ca.cer`, and `http://proxzy.local/`) that serve only the local CA certificate without accepting control mutations.
_Avoid_: Admin portal, onboarding server

**Protected Domain**:
An OS telemetry, push, OCSP/CRL certificate revocation, or system update hostname that always flows as a raw opaque tunnel without TLS decryption.
_Avoid_: Blocked domain, ignored host

**Selective Interception**:
The default TLS inspection rule: only hosts matched by the explicit SSL allowlist undergo MITM decryption; all other `CONNECT` tunnels pass through untouched while still appearing as `Tunnel` rows in traffic.
_Avoid_: Global MITM, decrypt all

### Mocking & Replay

**Capture**:
An observed HTTP/1.1 or HTTPS exchange or raw `Tunnel` session recorded by the proxy with timing, headers, payload bodies, and `ApiKind` (`Rest`, `GraphQl`, `Tunnel`, `Static`).
_Avoid_: Request log, trace, packet

**Map Rule**:
A stateful local response mock identified by method, URL/path/domain wildcard pattern, optional GraphQL operation name, folder, version/environment tags, and three selectable scenarios.
_Avoid_: Stub, override, fake endpoint

**Mock Scenario**:
One of three curated response variants stored on a rule (`Success`, `Error`, or `Empty`) plus rule-level enable/disable for live upstream passthrough.
_Avoid_: Fixture mode, mock profile

**Auto-Catch**:
Live recording mode (`POST /__proxzy/record`) that scrubs sensitive tokens/credentials from live `200 OK` JSON responses into template placeholders and registers a reusable Map Rule.
_Avoid_: Auto-mock, snapshotter

### E2E Verification & Devices

**Test Session**:
An active verification window tracking calls against baseline/target app versions, environment, and schema compatibility.
_Avoid_: Run ID, test trace

**Contract Drift**:
The schema delta between a recorded Map Rule JSON payload and a live upstream response, classified as `Clean`, `Additive` (new keys), or `Breaking` (missing or type-changed keys).
_Avoid_: Schema mismatch, diff

**Target Platform**:
The execution environment under test (`Mac`, `IosSimulator`, `IosDevice`, `AndroidEmulator`, `AndroidDevice`) that determines proxy host routing, bridge commands, and harness snippet defaults.
_Avoid_: Client type, runner

**Android Transport**:
The link mode for a physical Android phone: `UsbReverse` (loopback via `adb reverse tcp:<port> tcp:<port>`) or `WifiLan` (direct LAN IP with explicit LAN opt-in).
_Avoid_: ADB mode, connection string

