//! Pure helpers for discovering Chromium-family browsers and building
//! Isolated Browser Session launch specifications without altering host proxy
//! settings or global certificate stores.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// A discovered desktop Chromium-family executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserCandidate {
    pub name: &'static str,
    pub executable: PathBuf,
}

/// Complete, detached launch specification for an Isolated Browser Session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserLaunchSpec {
    pub browser_name: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub profile_dir: PathBuf,
}

/// Return a dedicated temporary profile directory scoped to this proxy port
/// so the user's primary browser profile, cookies, and history are never touched.
pub fn isolated_profile_dir(port: u16) -> PathBuf {
    std::env::temp_dir().join(format!("proxzy-browser-{}", port))
}

/// Ordered list of well-known Chromium-family executable paths for `os`
/// (`"macos"`, `"linux"`, `"windows"`).
pub fn candidate_paths_for_os(
    os: &str,
    home: Option<&Path>,
    local_app_data: Option<&Path>,
    program_files: Option<&Path>,
    program_files_x86: Option<&Path>,
) -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    match os {
        "macos" => {
            let apps = [
                ("Google Chrome", "Google Chrome.app/Contents/MacOS/Google Chrome"),
                ("Chromium", "Chromium.app/Contents/MacOS/Chromium"),
                ("Brave", "Brave Browser.app/Contents/MacOS/Brave Browser"),
                ("Microsoft Edge", "Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
                (
                    "Google Chrome Canary",
                    "Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                ),
            ];
            for (name, rel) in apps {
                out.push((name, PathBuf::from("/Applications").join(rel)));
                if let Some(h) = home {
                    out.push((name, h.join("Applications").join(rel)));
                }
            }
        }
        "linux" => {
            for (name, path) in [
                ("Google Chrome", "/usr/bin/google-chrome"),
                ("Google Chrome", "/usr/bin/google-chrome-stable"),
                ("Chromium", "/usr/bin/chromium"),
                ("Chromium", "/usr/bin/chromium-browser"),
                ("Chromium", "/snap/bin/chromium"),
                ("Brave", "/usr/bin/brave-browser"),
                ("Microsoft Edge", "/usr/bin/microsoft-edge"),
                ("Microsoft Edge", "/usr/bin/microsoft-edge-stable"),
            ] {
                out.push((name, PathBuf::from(path)));
            }
        }
        "windows" => {
            if let Some(local) = local_app_data {
                out.push((
                    "Google Chrome",
                    local.join(r"Google\Chrome\Application\chrome.exe"),
                ));
                out.push(("Chromium", local.join(r"Chromium\Application\chrome.exe")));
                out.push((
                    "Brave",
                    local.join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
                ));
                out.push((
                    "Microsoft Edge",
                    local.join(r"Microsoft\Edge\Application\msedge.exe"),
                ));
            }
            for root in [program_files, program_files_x86].into_iter().flatten() {
                out.push((
                    "Google Chrome",
                    root.join(r"Google\Chrome\Application\chrome.exe"),
                ));
                out.push(("Chromium", root.join(r"Chromium\Application\chrome.exe")));
                out.push((
                    "Brave",
                    root.join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
                ));
                out.push((
                    "Microsoft Edge",
                    root.join(r"Microsoft\Edge\Application\msedge.exe"),
                ));
            }
        }
        _ => {}
    }
    out
}

/// Ordered executable names searched on `PATH` for `os`.
pub fn path_binary_names_for_os(os: &str) -> &'static [(&'static str, &'static str)] {
    match os {
        "windows" => &[
            ("Google Chrome", "chrome.exe"),
            ("Chromium", "chromium.exe"),
            ("Brave", "brave.exe"),
            ("Microsoft Edge", "msedge.exe"),
        ],
        _ => &[
            ("Google Chrome", "google-chrome"),
            ("Google Chrome", "google-chrome-stable"),
            ("Chromium", "chromium"),
            ("Chromium", "chromium-browser"),
            ("Brave", "brave-browser"),
            ("Microsoft Edge", "microsoft-edge"),
        ],
    }
}

/// Pure discovery helper checking explicit candidate paths first, then `PATH`.
pub fn discover_chromium_in(
    candidates: &[(&'static str, PathBuf)],
    path_bins: &[(&'static str, &str)],
    path_env: Option<&OsStr>,
) -> Option<BrowserCandidate> {
    for (name, path) in candidates {
        if path.is_file() {
            return Some(BrowserCandidate {
                name,
                executable: path.clone(),
            });
        }
    }
    if let Some(raw_path) = path_env {
        let dirs: Vec<PathBuf> = std::env::split_paths(raw_path).collect();
        for (name, bin) in path_bins {
            for dir in &dirs {
                let candidate = dir.join(bin);
                if candidate.is_file() {
                    return Some(BrowserCandidate {
                        name,
                        executable: candidate,
                    });
                }
            }
        }
    }
    None
}

/// Discover a Chromium-family browser on the current host.
pub fn discover_chromium() -> Option<BrowserCandidate> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let candidates = candidate_paths_for_os(
        std::env::consts::OS,
        home.as_deref(),
        local_app_data.as_deref(),
        program_files.as_deref(),
        program_files_x86.as_deref(),
    );
    let path_bins = path_binary_names_for_os(std::env::consts::OS);
    discover_chromium_in(&candidates, path_bins, std::env::var_os("PATH").as_deref())
}

/// Build Chromium CLI flags for an Isolated Browser Session pinned strictly
/// to the local CA's SPKI Fingerprint (never blanket `--ignore-certificate-errors`).
pub fn build_chromium_args(port: u16, spki_fingerprint: &str, profile_dir: &Path) -> Vec<String> {
    let mut args = vec![
        format!("--proxy-server=http://127.0.0.1:{}", port),
        format!("--user-data-dir={}", profile_dir.display()),
    ];
    let spki = spki_fingerprint.trim();
    if !spki.is_empty() {
        args.push(format!("--ignore-certificate-errors-spki-list={}", spki));
    }
    args.push("--no-first-run".to_string());
    args.push("--no-default-browser-check".to_string());
    args
}

/// Assemble a [`BrowserLaunchSpec`] from a discovered browser candidate.
pub fn launch_spec_with(
    candidate: Option<BrowserCandidate>,
    port: u16,
    spki_fingerprint: &str,
    profile_dir: &Path,
) -> Result<BrowserLaunchSpec, String> {
    let Some(browser) = candidate else {
        return Err(
            "No Chrome, Chromium, Brave, or Microsoft Edge executable was found. Copy the CLI command instead."
                .to_string(),
        );
    };
    if spki_fingerprint.trim().is_empty() {
        return Err(
            "Local CA SPKI fingerprint is not ready yet. Start the proxy or create the certificate first."
                .to_string(),
        );
    }
    Ok(BrowserLaunchSpec {
        browser_name: browser.name.to_string(),
        executable: browser.executable,
        args: build_chromium_args(port, spki_fingerprint, profile_dir),
        profile_dir: profile_dir.to_path_buf(),
    })
}

/// Discover a host Chromium-family browser and build its [`BrowserLaunchSpec`].
pub fn launch_spec(port: u16, spki_fingerprint: &str) -> Result<BrowserLaunchSpec, String> {
    let profile_dir = isolated_profile_dir(port);
    launch_spec_with(discover_chromium(), port, spki_fingerprint, &profile_dir)
}

pub fn default_executable_placeholder(os: &str) -> &'static str {
    match os {
        "macos" => "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "windows" => "chrome.exe",
        _ => "google-chrome",
    }
}

fn quote_if_needed(value: &str) -> String {
    if value.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// Format a copyable single-line Chromium Isolated Browser Session CLI command.
pub fn chromium_cli_command(
    port: u16,
    spki_fingerprint: &str,
    profile_dir: &Path,
    executable: Option<&Path>,
) -> String {
    let exe_str = executable
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| default_executable_placeholder(std::env::consts::OS).to_string());
    let effective_spki = if spki_fingerprint.trim().is_empty() {
        "<SPKI_FINGERPRINT>"
    } else {
        spki_fingerprint.trim()
    };
    let mut parts = Vec::with_capacity(6);
    parts.push(quote_if_needed(&exe_str));
    for arg in build_chromium_args(port, effective_spki, profile_dir) {
        parts.push(quote_if_needed(&arg));
    }
    parts.join(" ")
}

/// Format copyable Firefox isolated-profile + CA import instructions.
pub fn firefox_guidance(port: u16, pem_path: &str) -> String {
    format!(
        "# Firefox (isolated profile — leaves macOS proxy & keychain untouched)\n\
         firefox -CreateProfile proxzy-test\n\
         firefox -P proxzy-test -no-remote\n\
         # Firefox Settings > Network Settings: Manual proxy 127.0.0.1 port {port} (HTTPS too)\n\
         # Firefox Settings > Privacy & Security > View Certificates > Authorities > Import:\n\
         # \"{pem_path}\"  (or open http://proxzy.internal/ inside that Firefox tab)"
    )
}

/// Format a copyable Playwright snippet using the local proxy and SPKI Fingerprint.
pub fn playwright_snippet(port: u16, spki_fingerprint: &str) -> String {
    let spki = if spki_fingerprint.trim().is_empty() {
        "<SPKI_FINGERPRINT>"
    } else {
        spki_fingerprint.trim()
    };
    format!(
        "import {{ chromium }} from 'playwright';\n\n\
         const browser = await chromium.launch({{\n\
         \x20 proxy: {{ server: 'http://127.0.0.1:{port}' }},\n\
         \x20 args: ['--ignore-certificate-errors-spki-list={spki}'],\n\
         }});"
    )
}

/// Format CA removal instructions for users who opted into keychain trust or
/// want to wipe the generated local CA directory.
pub fn ca_removal_commands(ca_dir: &str) -> String {
    format!(
        "# Isolated Browser Session never touches your OS keychain — nothing to uninstall.\n\
         # If you opted into trusting Proxzy Local CA in macOS Keychain:\n\
         security delete-certificate -c \"Proxzy Local CA\" ~/Library/Keychains/login.keychain-db\n\
         # To delete the generated local CA directory:\n\
         rm -rf \"{}\"",
        ca_dir
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "proxzy-browser-test-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builds_isolated_chromium_flags_without_blanket_ignore_errors() {
        let profile = PathBuf::from("/tmp/proxzy-browser-9090");
        let args = build_chromium_args(
            9090,
            "06IY2CP9PulGUPlnE3P6NsMfJkoYhCeKeKJ/nR+lEH4=",
            &profile,
        );
        assert!(args.contains(&"--proxy-server=http://127.0.0.1:9090".to_string()));
        assert!(args.contains(&"--user-data-dir=/tmp/proxzy-browser-9090".to_string()));
        assert!(args.contains(
            &"--ignore-certificate-errors-spki-list=06IY2CP9PulGUPlnE3P6NsMfJkoYhCeKeKJ/nR+lEH4="
                .to_string()
        ));
        assert!(
            !args.iter().any(|a| a == "--ignore-certificate-errors"),
            "must never emit blanket --ignore-certificate-errors"
        );
    }

    #[test]
    fn candidate_paths_cover_macos_linux_and_windows() {
        let mac = candidate_paths_for_os(
            "macos",
            Some(Path::new("/Users/dev")),
            None,
            None,
            None,
        );
        assert!(mac
            .iter()
            .any(|(n, p)| *n == "Google Chrome"
                && p == Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")));
        assert!(mac
            .iter()
            .any(|(n, p)| *n == "Microsoft Edge"
                && p == Path::new("/Users/dev/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge")));

        let linux = candidate_paths_for_os("linux", None, None, None, None);
        assert!(linux
            .iter()
            .any(|(n, p)| *n == "Chromium" && p == Path::new("/usr/bin/chromium")));

        let win = candidate_paths_for_os(
            "windows",
            None,
            Some(Path::new(r"C:\Users\dev\AppData\Local")),
            Some(Path::new(r"C:\Program Files")),
            Some(Path::new(r"C:\Program Files (x86)")),
        );
        assert!(win.iter().any(|(n, _)| *n == "Google Chrome"));
        assert!(win.iter().any(|(n, _)| *n == "Microsoft Edge"));
    }

    #[test]
    fn discovers_preferred_candidate_and_falls_back_to_path() {
        let root = temp_test_dir("discover");
        let chrome_path = root.join("chrome-bin");
        let edge_path = root.join("edge-bin");
        std::fs::write(&edge_path, b"stub").unwrap();

        let candidates = vec![
            ("Google Chrome", chrome_path.clone()),
            ("Microsoft Edge", edge_path.clone()),
        ];
        // Edge exists while Chrome does not -> selects Edge
        let found = discover_chromium_in(&candidates, &[("Chromium", "chromium")], None).unwrap();
        assert_eq!(found.name, "Microsoft Edge");
        assert_eq!(found.executable, edge_path);

        // Chrome exists -> prefers Chrome over Edge
        std::fs::write(&chrome_path, b"stub").unwrap();
        let preferred = discover_chromium_in(&candidates, &[], None).unwrap();
        assert_eq!(preferred.name, "Google Chrome");
        assert_eq!(preferred.executable, chrome_path);

        // Fallback to PATH when explicit candidates do not exist
        let path_dir = root.join("path_dir");
        std::fs::create_dir_all(&path_dir).unwrap();
        let path_chromium = path_dir.join("chromium");
        std::fs::write(&path_chromium, b"stub").unwrap();
        let path_env = std::env::join_paths([&path_dir]).unwrap();
        let via_path = discover_chromium_in(
            &[("Google Chrome", root.join("missing"))],
            &[("Chromium", "chromium")],
            Some(path_env.as_os_str()),
        )
        .unwrap();
        assert_eq!(via_path.name, "Chromium");
        assert_eq!(via_path.executable, path_chromium);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn launch_spec_requires_discovered_browser_and_non_empty_spki() {
        let profile = PathBuf::from("/tmp/proxzy-browser-9090");
        assert!(launch_spec_with(None, 9090, "abc=", &profile).is_err());

        let candidate = BrowserCandidate {
            name: "Google Chrome",
            executable: PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        };
        assert!(launch_spec_with(Some(candidate.clone()), 9090, "   ", &profile).is_err());

        let spec = launch_spec_with(Some(candidate), 9191, "SPKI123=", &profile).unwrap();
        assert_eq!(spec.browser_name, "Google Chrome");
        assert!(spec
            .args
            .contains(&"--proxy-server=http://127.0.0.1:9191".to_string()));
        assert!(spec
            .args
            .contains(&"--ignore-certificate-errors-spki-list=SPKI123=".to_string()));
    }

    #[test]
    fn formats_cli_firefox_playwright_and_removal_strings() {
        let exe = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        let profile = Path::new("/tmp/proxzy-browser-9090");
        let cmd = chromium_cli_command(9090, "SPKI+ABC=", profile, Some(exe));
        assert!(cmd.starts_with("\"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome\""));
        assert!(cmd.contains("--proxy-server=http://127.0.0.1:9090"));
        assert!(cmd.contains("--user-data-dir=/tmp/proxzy-browser-9090"));
        assert!(cmd.contains("--ignore-certificate-errors-spki-list=SPKI+ABC="));

        let ff = firefox_guidance(9090, "/tmp/proxzy-ca.pem");
        assert!(ff.contains("proxzy-test"));
        assert!(ff.contains("127.0.0.1 port 9090"));
        assert!(ff.contains("http://proxzy.internal/"));

        let pw = playwright_snippet(9090, "SPKI+ABC=");
        assert!(pw.contains("proxy: { server: 'http://127.0.0.1:9090' }"));
        assert!(pw.contains("--ignore-certificate-errors-spki-list=SPKI+ABC="));

        let rm = ca_removal_commands("/Users/dev/Library/Application Support/Proxzy");
        assert!(rm.contains("security delete-certificate -c \"Proxzy Local CA\""));
    }
}
