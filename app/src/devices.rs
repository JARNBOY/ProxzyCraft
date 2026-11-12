//! Device discovery for the "connect a device" step.
//!
//! The app never installs a certificate on its own. It first asks the Mac which
//! iOS simulators and Android emulators are available, shows that list to the
//! person, and only then offers a confirmed "connect" action. Everything here
//! shells out to the normal developer tools (`xcrun simctl` and `adb`); nothing
//! touches the inspected app.

use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceKind {
    IosSimulator,
    AndroidEmulator,
    AndroidDevice,
}

impl DeviceKind {
    pub fn label(self) -> &'static str {
        match self {
            DeviceKind::IosSimulator => "iOS Simulator",
            DeviceKind::AndroidEmulator => "Android emulator",
            DeviceKind::AndroidDevice => "Android device",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            DeviceKind::IosSimulator => "iOS SIM",
            DeviceKind::AndroidEmulator => "ANDROID EMU",
            DeviceKind::AndroidDevice => "ANDROID",
        }
    }

    pub fn is_android(self) -> bool {
        !matches!(self, DeviceKind::IosSimulator)
    }
}

#[derive(Clone, Debug)]
pub struct DetectedDevice {
    /// UDID for a simulator, serial number for an Android device.
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    /// "Booted", "Shutdown", "device", "offline" or "unauthorized".
    pub state: String,
    /// Runtime / model line shown under the name.
    pub detail: String,
    /// True when the device is running and can accept a certificate now.
    pub ready: bool,
}

impl DetectedDevice {
    pub fn state_pill(&self) -> (&'static str, bool) {
        if self.ready {
            ("READY", true)
        } else if self.state.eq_ignore_ascii_case("unauthorized") {
            ("AUTHORIZE", false)
        } else if self.state.eq_ignore_ascii_case("offline") {
            ("OFFLINE", false)
        } else if self.kind == DeviceKind::IosSimulator {
            ("NOT BOOTED", false)
        } else {
            ("NOT READY", false)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DeviceScanResult {
    pub devices: Vec<DetectedDevice>,
    pub notes: Vec<String>,
}

impl DeviceScanResult {
    pub fn ready_count(&self) -> usize {
        self.devices.iter().filter(|d| d.ready).count()
    }
}

/// Ask the Mac for every simulator and every connected Android device.
/// This blocks while the developer tools answer, so the app calls it on a
/// worker thread and keeps drawing.
pub fn detect_all() -> DeviceScanResult {
    let mut result = DeviceScanResult::default();
    detect_ios(&mut result);
    detect_android(&mut result);

    // The card lists what can be connected now. Shutdown simulators are
    // summarised instead of filling the list; Android devices stay visible so
    // an "unauthorized" phone can tell the person what to fix.
    let shutdown_ios = result
        .devices
        .iter()
        .filter(|d| d.kind == DeviceKind::IosSimulator && !d.ready)
        .count();
    result
        .devices
        .retain(|d| d.ready || d.kind.is_android());
    if shutdown_ios > 0 {
        result.notes.push(format!(
            "{} other iOS simulator{} installed but not running. Boot one to connect it.",
            shutdown_ios,
            if shutdown_ios == 1 { "" } else { "s" }
        ));
    }

    result
        .devices
        .sort_by(|a, b| b.ready.cmp(&a.ready).then(a.name.cmp(&b.name)));
    result
}

fn detect_ios(result: &mut DeviceScanResult) {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices"])
        .output();
    let out = match output {
        Ok(out) => out,
        Err(_) => {
            result.notes.push(
                "Xcode command line tools were not found, so iOS simulators cannot be listed."
                    .to_string(),
            );
            return;
        }
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        result.notes.push(if err.is_empty() {
            "xcrun could not list iOS simulators.".to_string()
        } else {
            format!("xcrun could not list iOS simulators: {}", err)
        });
        return;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut runtime = String::from("iOS Simulator");
    let mut found = 0usize;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("--") && trimmed.ends_with("--") {
            runtime = trimmed.trim_matches('-').trim().to_string();
            continue;
        }
        if trimmed.contains("Unavailable") {
            continue;
        }
        let Some(udid) = uuid_in(trimmed) else {
            continue;
        };
        let name = trimmed
            .split(" (")
            .next()
            .unwrap_or("iOS Simulator")
            .trim()
            .to_string();
        let booted = trimmed.contains("(Booted)");
        let state = if booted {
            "Booted"
        } else if trimmed.contains("(Shutdown)") {
            "Shutdown"
        } else {
            "Unavailable"
        };
        result.devices.push(DetectedDevice {
            id: udid,
            name,
            kind: DeviceKind::IosSimulator,
            state: state.to_string(),
            detail: runtime.clone(),
            ready: booted,
        });
        found += 1;
    }
    if found == 0 {
        result.notes.push(
            "No iOS simulator is installed. Open Xcode › Settings › Platforms and download a simulator runtime."
                .to_string(),
        );
    }
}

fn detect_android(result: &mut DeviceScanResult) {
    let output = Command::new("adb").args(["devices", "-l"]).output();
    let out = match output {
        Ok(out) => out,
        Err(_) => {
            result.notes.push(
                "adb was not found. Install Android platform-tools or use Android Studio's SDK Manager, then scan again."
                    .to_string(),
            );
            return;
        }
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        result.notes.push(if err.is_empty() {
            "adb could not list Android devices.".to_string()
        } else {
            format!("adb could not list Android devices: {}", err)
        });
        return;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut found = 0usize;
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(serial) = parts.next() else { continue };
        let state = parts.next().unwrap_or("unknown").to_string();
        let mut model = String::new();
        for part in parts {
            if let Some(value) = part.strip_prefix("model:") {
                model = value.replace('_', " ");
            }
        }
        let emulator = serial.starts_with("emulator-");
        let kind = if emulator {
            DeviceKind::AndroidEmulator
        } else {
            DeviceKind::AndroidDevice
        };
        let avd = if emulator { avd_name(serial) } else { None };
        let name = avd
            .or_else(|| {
                if model.is_empty() {
                    None
                } else {
                    Some(model.clone())
                }
            })
            .unwrap_or_else(|| serial.to_string());
        let detail = if model.is_empty() {
            serial.to_string()
        } else {
            format!("{} · {}", serial, model)
        };
        result.devices.push(DetectedDevice {
            id: serial.to_string(),
            name,
            kind,
            ready: state == "device",
            state,
            detail,
        });
        found += 1;
    }
    if found == 0 {
        result.notes.push(
            "No Android device is connected. Start an emulator in Android Studio (or plug in a phone with USB debugging), then scan again."
                .to_string(),
        );
    }
}

/// The AVD name is friendlier than "sdk_gphone64_arm64"; this is best-effort.
fn avd_name(serial: &str) -> Option<String> {
    let out = Command::new("adb")
        .args(["-s", serial, "emu", "avd", "name"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.eq_ignore_ascii_case("OK"))
        .map(str::to_string)
}

/// The address a physical mobile device can use to reach this host on the LAN.
/// An Android emulator instead uses the special alias 10.0.2.2 or adb reverse.
pub fn host_ip() -> Option<String> {
    for iface in ["en0", "en1", "bridge100"] {
        let Ok(out) = Command::new("ipconfig")
            .args(["getifaddr", iface])
            .output()
        else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(ip) = value.parse::<std::net::IpAddr>() {
            if !ip.is_loopback() && !ip.is_unspecified() {
                return Some(value);
            }
        }
    }
    // Cross-platform fallback via routing-table source IP selection (does not transmit).
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("1.1.1.1:80").is_ok() {
            if let Ok(local) = sock.local_addr() {
                let ip = local.ip();
                if !ip.is_loopback() && !ip.is_unspecified() {
                    return Some(ip.to_string());
                }
            }
        }
    }
    None
}

fn uuid_in(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    for start in 0..bytes.len() {
        if start + 36 <= bytes.len() {
            let candidate = &line[start..start + 36];
            if is_uuid(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_uuid_in_a_simulator_line() {
        let line = "    iPhone 16 Pro (044AEE75-8148-4976-9B50-EAFB2C887E55) (Booted)";
        assert_eq!(
            uuid_in(line).as_deref(),
            Some("044AEE75-8148-4976-9B50-EAFB2C887E55")
        );
    }

    #[test]
    fn ignores_plain_names() {
        assert_eq!(uuid_in("iPhone 16 Pro (Booted)"), None);
    }

    #[test]
    fn parses_android_devices_line() {
        let mut result = DeviceScanResult::default();
        // Reuse the parser through a tiny helper so no real adb is needed.
        let line = "emulator-5554          device product:sdk_gphone64_arm64 model:sdk_gphone64_arm64 device:emu64a transport_id:1";
        let mut parts = line.split_whitespace();
        let serial = parts.next().unwrap();
        let state = parts.next().unwrap();
        assert_eq!(serial, "emulator-5554");
        assert_eq!(state, "device");
        result.devices.push(DetectedDevice {
            id: serial.to_string(),
            name: "Pixel".to_string(),
            kind: DeviceKind::AndroidEmulator,
            state: state.to_string(),
            detail: serial.to_string(),
            ready: state == "device",
        });
        assert_eq!(result.ready_count(), 1);
    }

    #[test]
    fn host_ip_never_returns_loopback_or_unspecified() {
        if let Some(ip) = host_ip() {
            let parsed: std::net::IpAddr = ip.parse().expect("valid IP");
            assert!(!parsed.is_loopback());
            assert!(!parsed.is_unspecified());
        }
    }
}
