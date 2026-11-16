# Privacy Policy & Root CA Removal Guide

ProxzyCraft is built for **100% local-first network debugging and E2E API mocking**.

---

## 1. Zero Telemetry & Zero Cloud Accounts

- **No telemetry or analytics:** ProxzyCraft contains no telemetry SDKs, crash-reporting beacons, usage trackers, or remote license/update checks.
- **No cloud accounts or remote storage:** Nothing is locked behind sign-ins or trials, and no captured traffic, mock fixtures, certificates, or logs are uploaded to any external service.
- **100% local execution:** Both the native desktop GUI and `--headless` daemon operate solely on your local machine (`127.0.0.1` by default, or your explicit opt-in LAN interface when `--lan` or **Allow LAN devices** is enabled).

---

## 2. Local Certificate Authority (CA) & Private Key Guarantees

- **Per-machine key generation:** On first run, ProxzyCraft generates a unique local Root CA (`Proxzy Local CA`) under `~/Library/Application Support/Proxzy/`:
  - Public certificates: `proxzy-ca.pem`, `proxzy-ca.cer`
  - Private key: `proxzy-ca-key.pem` (written with Unix permissions `0o600` — owner read/write only)
- **Never shipped or shared:** Pre-generated CA private keys are never included in the source repository or public release archives.
- **Never served over HTTP:** The read-only Certificate Portal (`/`, `/ca.cer`, `/ca.pem`, `http://proxzy.internal/`, `http://proxzy.local/`) exposes **only** the public CA certificate and never serves `proxzy-ca-key.pem` or `control.token`.
- **Automatic PII & secret scrubbing:** When Auto-Catch (`POST /__proxzy/record`) records live `200 OK` JSON responses, `sanitize_mock_payload` automatically redacts sensitive tokens, authorization headers, passwords, OTPs, phone numbers, and IDs into template placeholders (`{{TOKEN}}`, `{{USERNAME}}`, `{{OTP}}`) before persisting mock fixtures.

---

## 3. Authorized Debugging Scope & Responsible Use

ProxzyCraft performs TLS MITM decryption only for hosts explicitly placed in the SSL allowlist (or when opt-in `--full-catch` is enabled, while always excluding built-in OS/push/OCSP domains in `BUILTIN_PROTECTED_DOMAINS`).

**Intended use:** ProxzyCraft is strictly for debugging, security QA, and automated E2E testing against **devices, simulators, browsers, and APIs that you own or have explicit authorization to inspect**. Do not use MITM interception on networks, third-party accounts, or devices without permission.

---

## 4. Complete Root CA Removal & Cleanup Instructions

### A. Isolated Browser Session (Recommended — Zero Keychain Cleanup)

When you use **Isolated Browser Session** (`--proxy-server`, `--user-data-dir`, `--ignore-certificate-errors-spki-list=<SPKI>`), ProxzyCraft **never touches** your macOS/OS keychain or system proxy settings.

- Closing the isolated browser window stops proxying immediately.
- Optionally remove the temporary browser profile directory created in your system temp folder:
  ```bash
  rm -rf "${TMPDIR:-/tmp}/proxzy-browser-9090"
  ```
- For **Firefox** isolated profiles (`firefox -P proxzy-test -no-remote`): open *Settings → Privacy & Security → View Certificates → Authorities*, select **Proxzy Local CA**, and click **Delete or Distrust**, or delete the `proxzy-test` profile via `firefox -ProfileManager`.

### B. macOS Keychain (If You Opted Into Host Keychain Trust)

1. **Command line (fast removal from login keychain):**
   ```bash
   security delete-certificate -c "Proxzy Local CA" ~/Library/Keychains/login.keychain-db
   ```
   *(If you previously trusted a certificate by SHA-1/SHA-256 hash or in another keychain, you can also run `security remove-trusted-cert <path-to-proxzy-ca.pem>` first, then delete the certificate in Keychain Access.)*
2. **Keychain Access GUI:**
   1. Open **Keychain Access.app**.
   2. Select **login** in the left sidebar and open the **Certificates** tab.
   3. Locate **Proxzy Local CA**, right-click → **Delete "Proxzy Local CA"**, and confirm.
3. **Restore macOS proxy settings (if you enabled Set Mac proxy):**
   - Click **Clear Mac proxy** in **Live Capture** (or **Restore now** in the status bar; ProxzyCraft also restores `networksetup` settings automatically on clean exit).
   - If the process was force-killed while Mac proxy was active, reset your active interface (e.g. `Wi-Fi`) in *System Settings → Network → Wi-Fi → Details → Proxies* (toggle Web Proxy and Secure Web Proxy off).

### C. iOS Simulator

- **Reset a single simulator:**
  ```bash
  xcrun simctl shutdown <UDID>
  xcrun simctl erase <UDID>
  ```
- **Or remove inside Simulator UI:** *Settings → General → VPN & Device Management* (remove profile if present) and *Settings → General → About → Certificate Trust Settings* (turn off full trust for **Proxzy Local CA**).

### D. Physical iOS / iPadOS Device

1. *Settings → General → About → Certificate Trust Settings* → toggle **off** full trust for **Proxzy Local CA**.
2. *Settings → General → VPN & Device Management* → select the **Proxzy Local CA** profile → **Remove Profile**.
3. *Settings → Wi-Fi → (i) → Configure Proxy* → set back to **Off**.

### E. Android Emulator & Physical Android Device

1. Open Android *Settings → Security → Encryption & credentials → Trusted credentials → User* tab.
2. Tap **Proxzy Local CA** and choose **Remove** (or use **Clear credentials**).
3. Clear any ADB proxy override:
   ```bash
   adb shell settings put global http_proxy :0
   ```
   *(If you used `adb reverse`, clear it with `adb reverse --remove tcp:9090` or `adb reverse --remove-all`. On physical Wi-Fi, set Wi-Fi Proxy back to **None**.)*

### F. Linux

- **Debian / Ubuntu:**
  ```bash
  sudo rm -f /usr/local/share/ca-certificates/proxzy-ca.crt
  sudo update-ca-certificates --fresh
  ```
- **Fedora / RHEL:**
  ```bash
  sudo rm -f /etc/pki/ca-trust/source/anchors/proxzy-ca.pem
  sudo update-ca-trust
  ```
- **NSS / Chrome / Firefox databases (if manually imported with `certutil`):**
  ```bash
  certutil -D -n "Proxzy Local CA" -d sql:$HOME/.pki/nssdb
  ```

### G. Windows

- **Command line (Current User Root store):**
  ```powershell
  certutil -delstore -user Root "Proxzy Local CA"
  ```
- **GUI (`certmgr.msc`):**
  1. Press `Win+R`, run `certmgr.msc`.
  2. Navigate to **Trusted Root Certification Authorities → Certificates**.
  3. Right-click **Proxzy Local CA** and select **Delete**.

### H. Wipe All Generated Local Proxzy Data (Optional)

To remove the local CA keypair, public certificates, and `control.token`:
```bash
rm -rf "$HOME/Library/Application Support/Proxzy"
```
*(In ProxzyCraft, clicking **Open folder** in Live Capture opens this directory directly.)*
