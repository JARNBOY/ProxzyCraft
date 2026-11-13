# Proxzy — a network-inspector learning POC

Proxzy is a small desktop app that shows how a Proxyman-style network tool fits
together: a real local HTTP/HTTPS proxy, a live traffic list, a request
inspector, copy/share, repeat, a Composer, Map Local response mocking,
Breakpoints, and a log dashboard. Nothing is behind an account, trial or paid
tier.

## Run it

```bash
cargo run
```

Build a release copy:

```bash
cargo build --release
```

Take an automatic picture of the window (the same check the project harness
uses), then exit:

```bash
cargo run -- --screenshot shots/now.png
```

Start with full HTTPS catch already on (every host is decrypted except the
protected system services; trust the CA on the device first):

```bash
cargo run -- --full-catch
```

Optional start page for demos:

```bash
PROXZY_VIEW=tutorial cargo run
PROXZY_VIEW=guide cargo run
PROXZY_VIEW=map cargo run
PROXZY_VIEW=breakpoints cargo run
PROXZY_VIEW=logs cargo run
PROXZY_VIEW=composer cargo run
PROXZY_VIEW=acp cargo run
```

Open the tutorial or a specific tour stop immediately:

```bash
PROXZY_DEMO=tour cargo run              # 13-stop tour from the start
PROXZY_DEMO=tour-agent cargo run        # jump to Agent Control
PROXZY_DEMO=tour-agent-hook cargo run   # jump to the IDE hook card
PROXZY_DEMO=tour-acp cargo run          # jump to ACP Bridge
PROXZY_DEMO=tour-6 cargo run            # any stop, 0-12
```

The ACP screen also has three demo states:

```bash
PROXZY_DEMO=acp-replay cargo run        # auto-play the concept session
PROXZY_DEMO=acp-full cargo run          # allowed run, stopped at the end
PROXZY_DEMO=acp-deny cargo run          # rejected permission, stopped at refusal
```

Open a demo editor or dialog immediately (handy for screenshots):

```bash
PROXZY_DEMO=map-edit cargo run
PROXZY_DEMO=bp-edit cargo run
PROXZY_DEMO=pause cargo run
PROXZY_DEMO=share cargo run
PROXZY_DEMO=connect cargo run
PROXZY_DEMO=stale cargo run
PROXZY_DEMO=tunnels cargo run
PROXZY_DEMO=protected cargo run
PROXZY_DEMO=full-catch cargo run
PROXZY_DEMO=check cargo run
PROXZY_DEMO=doctor cargo run
```

`PROXZY_DEMO=connect` opens the "Connect device" confirmation as soon as a
ready simulator or emulator is found.

Combine the two to capture a page with its modal, for example
`PROXZY_VIEW=map PROXZY_DEMO=map-edit cargo run`.

## What each part does

| Area | What it does | How to try it |
| --- | --- | --- |
| Live Capture | Real proxy on `127.0.0.1:9090` (falls back to 9091–9093) with HTTP and HTTPS interception | Open Live Capture, press Start proxy, then point a device at it. **Run check** sends one real request through the gate and reads which Mac route a simulator would inherit |
| Traffic | Captured calls with method, status, type, time and size; live rows are badged `LIVE` | Click a row to pin it; use the sidebar filters or the search box. `Tunnels` shows raw and protected connections. `Follow live` jumps back to the newest call |
| Inspector | Overview, Request, Response, Headers and cURL | Select a call, then switch tabs on the right. Response bodies switch between JSON (default), Raw, XML and HTML |
| Copy & share | URL, full cURL, a local share link and Markdown | Buttons under the URL, or the cURL tab |
| Repeat | Replays the selected call into a new capture (live HTTPS replays trust the local CA) | Select a call and press Repeat |
| Composer | Writes and sends GraphQL or REST requests | Pick a preset or edit method, URL, headers and body, then Send. Its response panel shares the same JSON (default), Raw, XML and HTML switch |
| Map Local | Answers matching live or Composer requests with local JSON, status and delay; a rule can also link a fixture file on disk and read it on every request | Add a rule, switch it on, optionally browse to or paste a local file path, use Send test or the Composer |
| Breakpoints | Pauses a real live request and lets you edit, continue or abort it | Arm a rule, send traffic through the proxy, edit, Continue or Abort |
| Log Dashboard | One timeline for captures, mocks, breakpoints and system events | Open it from Monitor in the rail, the Logs chip in the top bar, or All features; filter by Info / Warn / Error and copy the session summary |
| Agent Control | A loopback control plane an external agent or IDE can drive, plus Map Local scenario switches and body injection | Open Agent Control, copy a brief under **Hook an IDE**, then let the agent call `/__proxzy/health` and `/__proxzy/scenario` |
| ACP Bridge | A concept lab for the editor ↔ agent side of an AI coding setup: ACP v1 handshake, streamed updates and a permission gate | Open ACP Bridge, step through the session, then choose Allow once, Always allow or Reject |
| API Test Bench | Runs saved requests through the mock engine and pins the expected verdict | Open API Test Bench, pick a target, then Run selected or Run all |
| Screen Play & MCP | A scripted tour of the app for recording or demoing | Open Screen Play and start a play |
| Guide & Stack | Built-in teaching page | Open Guide & Stack in the sidebar, or the ? button |
| Tutorial | A 13-stop guided tour that switches to each page, dims the background and explains the main control | Open Tutorial under **Start Here**, or press the top-bar **Tutorial** chip. Use Back / Next stop, arrow keys, or Skip to Agent. Esc exits |

## Live capture and certificates

The proxy creates a local certificate authority on first launch:

- `~/Library/Application Support/Proxzy/proxzy-ca.pem`
- `~/Library/Application Support/Proxzy/proxzy-ca.cer`
- `~/Library/Application Support/Proxzy/proxzy-ca-key.pem`

Only the `.pem`/`.cer` files are meant to leave the Mac. The private key stays
in that folder.

### Safe capture: raw tunnels and protected domains

Every TCP connection is listed, even before decryption is on. An HTTPS app
domain that is not in the SSL allowlist appears with a `RAW TUNNEL` badge; its
bytes still flow normally but their contents stay encrypted. Select that row in
All Traffic and press **Enable TLS Decryption**, or turn on **Full HTTPS catch**
on Live Capture, to decrypt later requests from that domain. Protected system
domains are still left on a raw tunnel even when full catch is on.

Apple push, OS update and certificate-check domains, plus Google Play services,
always stay raw and carry a `PROTECTED` badge. That keeps macOS and simulators
safe from certificate errors and SSL pinning crashes. The sample session
includes one PROTECTED row and one RAW TUNNEL row under the **Tunnels** filter.

### Capture check

**Run check** on Live Capture answers "is the gate actually catching traffic?" in one
step. It sends `GET http://proxzy.gateway/__proxzy/check?nonce=…` through the real
proxy, shows the status and timing, and reports the HTTPS mode next to it. The
same strip reads which web proxy the Mac currently has enabled: a simulator
inherits that route, so **Direct (off)** means app traffic never reaches Proxzy.
The check never changes a setting. To turn the proxy on, press **Set Mac proxy**
and confirm; if macOS refuses, the app shows the matching `sudo networksetup`
command to copy.

`PROXZY_DEMO=check` opens this check automatically, and `PROXZY_DEMO=doctor`
opens it and leaves the result on screen, which is how the project screenshots
are taken.

### iOS Simulator

1. Keep the proxy running on the Live Capture page.
2. Open the simulator in Xcode. The **Detected devices** card refreshes every
   few seconds and shows the booted simulator by name and a green **READY** pill
   before any certificate action is offered.
3. Press **Connect…** on that row and confirm. **Connect and trust** installs
   the CA into that simulator's UDID with
   `xcrun simctl keychain <UDID> add-root-cert`, points the Mac's web proxy at
   Proxzy, and restarts the simulator so the new trust loads. **Certificate
   only** skips the proxy and restart.
4. Run the app. HTTPS calls appear in All Traffic after the simulator trusts the
   CA. If the one-click step cannot change a network service, use the commands
   shown on the page or in the Guide's "Connect real traffic" section.

### Android emulator or phone

Start an emulator in Android Studio (or plug in a phone with USB debugging) and
wait for it to appear in **Detected devices**. Press **Connect…** and confirm.
The action targets the serial you clicked: an emulator gets
`10.0.2.2:<port>`, a physical phone uses the Mac's LAN address, and the CA is
copied to `/sdcard/Download/proxzy-ca.cer`. Finish by installing it under
Settings › Security › Encryption & credentials › Install a certificate › CA
certificate. Android 7+ apps that opt out of user certificates need a debug
build or a network security config.

### This Mac

Press **Set Mac proxy** to point every enabled network service at Proxzy, or use
the `curl -x http://127.0.0.1:<port>` command without changing system settings.
While the Mac proxy is on, the status bar shows an amber **Mac proxy on** note
with a **Restore now** button. Closing Proxzy restores the proxy automatically,
so the browser does not keep showing "No internet" after the app quits. Press
**Clear Mac proxy** on the Live Capture page to restore it by hand at any time.

## Hook an IDE

Agent Control ends with a **Hook an IDE** card. It builds a plain-text brief for
Cursor, VS Code, Claude Code or JetBrains and copies it together with the rules
file that client reads (`.cursor/rules/proxzy.mdc`,
`.github/copilot-instructions.md`, `CLAUDE.md`, `.junie/guidelines.md`). The
brief names the proxy and control-plane ports, the CA path for simulators, the
health/scenario/inject/session calls, and the rule that a check only passes when
the engine response confirms it.

Nothing is written into another project for you. Paste the brief where the agent
reads its project context, then ask it to run the copied health command. That
request appears under **Control plane feed**, and the same agent can switch a
scenario or inject a body token before it reports a result. The control plane is
loopback-only and rejects browser-origin calls, so a local harness cannot expose
it to a web page.

## ACP Bridge

Agent Control answers "how does an agent drive Proxzy?" ACP Bridge answers the
question before that: how does an editor launch and supervise an AI coding
agent at all? **ACP** (Agent Client Protocol) is the editor ↔ agent link. **MCP**
is the agent ↔ tools link. Proxzy sits behind both as the HTTP gate that sees
the request the agent finally sends.

The page is a concept lab, not a live transport. It walks the public ACP v1
flow over JSON-RPC 2.0 on stdio:

1. `initialize` agrees on the protocol version and capabilities.
2. `session/new` names an absolute working folder and any MCP servers.
3. `session/prompt` sends the user's words.
4. `session/update` streams tool calls and message chunks back.
5. `session/request_permission` pauses before a side effect; the human chooses
   Allow once, Always allow or Reject.
6. `session/prompt` closes with a `stopReason` such as `end_turn` or `refusal`.

The transcript is simulated; no ACP process or tool call runs inside the lab.
The port printed on the page is the real loopback control plane an ACP agent
would call, and the sample tool shows how that agent could switch Map Local
rule MAP-01 to Error before sending the checkout request. Rejecting the
permission shows the agent stopping without that side effect. Copy-ready tabs
provide an editor `agent_servers` entry, an MCP server entry and a short agent
prompt.

Two things the lab deliberately keeps apart: ACP is not an HTTP proxy, and MCP
is not ACP. The editor owns the agent process and the permission gate; Proxzy
only owns the HTTP request that can be mocked, inspected or replayed.

## What is real and what is simulated

- **Real:** the local proxy, TLS interception, the CA and its certificate
  installation helpers, live Map Local answers, live request breakpoints,
  Composer sends through the system `curl`, repeat, clipboard actions, and every
  dashboard number.
- **Simulated:** the seeded opening traffic rows (badged `SAMPLE`), the
  pre-filled demo rules, the response-stage breakpoint editor, and the ACP
  session transcript. HTTPS interception is HTTP/1.1 only; HTTP/2 and gRPC are
  not advertised through the proxy.
- **All access:** every control that exists is usable. Nothing is locked behind
  an account or trial.

## Stack

- **Rust** for the program logic.
- **egui / eframe** for the native window and the immediate-mode UI.
- **System `curl`** for real HTTP sends.
- **`image`** for saving the `--screenshot` PNG.
- No web server, database or account is needed; everything runs locally.

## Project layout

- `src/main.rs` — window setup and `--screenshot` option.
- `src/proxy.rs` — real HTTP/1.1 proxy, CONNECT/TLS interception, local CA,
  live Map Local and live breakpoints.
- `src/app.rs` — state, navigation, proxy lifecycle, capture/Map Local/Breakpoint
  logic, certificate and simulator helpers.
- `src/devices.rs` — background discovery of booted iOS simulators and Android
  emulators/phones through `xcrun simctl` and `adb`.
- `src/views.rs` — Live Capture, Composer, Map Local, Breakpoints, Log Dashboard
  and Guide pages.
- `src/acp.rs` — ACP concept session transcript, permission model, checks and
  copy-ready setup templates.
- `src/acp_ui.rs` — the ACP Bridge page and its flow diagram.
- `src/studio_ui.rs` — Agent Control, API Test Bench and Screen Play & MCP.
- `src/tour_ui.rs` — the Tutorial page, its sidebar sketch and the 13-stop overlay.
- `src/rules_ui.rs` — shared rule tables and editors.
- `src/model.rs` — sample traffic, rules and helper formatting.
- `src/theme.rs` — colours, typography and shared widgets.

## Status Code Helper
Close Proxy In case look browser block internet from case if open Proxzy app not close proxy:
```bash
sudo networksetup -setwebproxystate "Wi-Fi" off && \
sudo networksetup -setsecurewebproxystate "Wi-Fi" off && \
sudo networksetup -setsocksfirewallproxystate "Wi-Fi" off && \
sudo networksetup -setautoproxystatus "Wi-Fi" off
```

