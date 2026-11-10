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

Optional start page for demos:

```bash
PROXZY_VIEW=guide cargo run
PROXZY_VIEW=map cargo run
PROXZY_VIEW=breakpoints cargo run
PROXZY_VIEW=logs cargo run
PROXZY_VIEW=composer cargo run
```

Open a demo editor or dialog immediately (handy for screenshots):

```bash
PROXZY_DEMO=map-edit cargo run
PROXZY_DEMO=bp-edit cargo run
PROXZY_DEMO=pause cargo run
PROXZY_DEMO=share cargo run
PROXZY_DEMO=connect cargo run
PROXZY_DEMO=stale cargo run
```

`PROXZY_DEMO=connect` opens the "Connect device" confirmation as soon as a
ready simulator or emulator is found.

Combine the two to capture a page with its modal, for example
`PROXZY_VIEW=map PROXZY_DEMO=map-edit cargo run`.

## What each part does

| Area | What it does | How to try it |
| --- | --- | --- |
| Live Capture | Real proxy on `127.0.0.1:9090` (falls back to 9091–9093) with HTTP and HTTPS interception | Open Live Capture, press Start proxy, then point a device at it |
| Traffic | Captured calls with method, status, type, time and size; live rows are badged `LIVE` | Click a row to pin it; use the sidebar filters or the search box. `Follow live` jumps back to the newest call |
| Inspector | Overview, Request, Response, Headers and cURL | Select a call, then switch tabs on the right. Response bodies switch between JSON (default), Raw, XML and HTML |
| Copy & share | URL, full cURL, a local share link and Markdown | Buttons under the URL, or the cURL tab |
| Repeat | Replays the selected call into a new capture (live HTTPS replays trust the local CA) | Select a call and press Repeat |
| Composer | Writes and sends GraphQL or REST requests | Pick a preset or edit method, URL, headers and body, then Send. Its response panel shares the same JSON (default), Raw, XML and HTML switch |
| Map Local | Answers matching live or Composer requests with local JSON, status and delay | Add a rule, switch it on, use Send test or the Composer |
| Breakpoints | Pauses a real live request and lets you edit, continue or abort it | Arm a rule, send traffic through the proxy, edit, Continue or Abort |
| Log Dashboard | One timeline for captures, mocks, breakpoints and system events | Filter by Info / Warn / Error; copy the session summary |
| Guide & Stack | Built-in teaching page | Open Guide & Stack in the sidebar, or the ? button |

## Live capture and certificates

The proxy creates a local certificate authority on first launch:

- `~/Library/Application Support/Proxzy/proxzy-ca.pem`
- `~/Library/Application Support/Proxzy/proxzy-ca.cer`
- `~/Library/Application Support/Proxzy/proxzy-ca-key.pem`

Only the `.pem`/`.cer` files are meant to leave the Mac. The private key stays
in that folder.

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

## What is real and what is simulated

- **Real:** the local proxy, TLS interception, the CA and its certificate
  installation helpers, live Map Local answers, live request breakpoints,
  Composer sends through the system `curl`, repeat, clipboard actions, and every
  dashboard number.
- **Simulated:** the seeded opening traffic rows (badged `SAMPLE`), the
  pre-filled demo rules, and the response-stage breakpoint editor. HTTPS
  interception is HTTP/1.1 only; HTTP/2 and gRPC are not advertised through the
  proxy.
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

