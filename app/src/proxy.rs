//! A small, real HTTP/1.1 proxy with optional HTTPS interception.
//!
//! This is the part that turns the POC from a demo feed into a tool that can
//! actually see a simulator's API calls. It listens on 127.0.0.1, accepts both
//! plain proxy requests and CONNECT tunnels, and (when the client trusts the
//! local CA) terminates TLS so the request and response can be inspected,
//! mocked or paused.
//!
//! Deliberately small: one request per connection, HTTP/1.1 only. That keeps
//! the learning POC readable while still working with iOS Simulator, Android
//! emulators, curl and normal macOS apps.

use crate::model::{
    auto_derive_empty_scenario, detect_kind, extract_domain_from_pattern, graphql_info,
    make_environment_wildcard_pattern, now_stamp, sanitize_mock_payload, ApiKind, BreakpointRule,
    Capture, MapRule, MockScenario, RuleMethod, SessionCallAudit, TestSession,
};
use flate2::read::GzDecoder;
use rcgen::{BasicConstraints, Certificate, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose};
use rustls::pki_types::{
    CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName,
};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, Semaphore};
use tokio_rustls::{TlsAcceptor, TlsConnector};

const MAX_HEAD: usize = 128 * 1024;
const MAX_BODY: usize = 12 * 1024 * 1024;
const HEAD_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONNECTIONS: usize = 128;

/// The hostname the in-app practice session calls. It never leaves the machine:
/// the proxy answers it itself, after Breakpoints and Map Local have run, so the
/// learning demo walks the same code path as a real device request.
pub const PRACTICE_HOST: &str = "practice.proxzy.gateway";

/// How the listener is allowed to be reached.
///
/// The safe default is loopback only: nothing on the network can see the proxy
/// or its certificate. A LAN listener is only created with an explicit opt-in
/// and always carries an ephemeral control token, so a phone on the same Wi-Fi
/// can use the proxy but cannot drive `/__proxzy/*` without the token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindPolicy {
    LoopbackOnly,
    Lan {
        advertised_ip: String,
        control_token: String,
    },
}

impl BindPolicy {
    pub fn is_lan(&self) -> bool {
        matches!(self, BindPolicy::Lan { .. })
    }

    pub fn control_token(&self) -> Option<&str> {
        match self {
            BindPolicy::Lan { control_token, .. } => Some(control_token),
            BindPolicy::LoopbackOnly => None,
        }
    }

    pub fn advertised_ip(&self) -> Option<&str> {
        match self {
            BindPolicy::Lan { advertised_ip, .. } => Some(advertised_ip),
            BindPolicy::LoopbackOnly => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ListenerBinding {
    pub bind_ip: IpAddr,
    pub port: u16,
    pub policy: BindPolicy,
}

impl ListenerBinding {
    pub fn display_address(&self) -> String {
        match self.policy.advertised_ip() {
            Some(ip) if self.bind_ip.is_unspecified() => format!("{}:{}", ip, self.port),
            _ => format!("{}:{}", self.bind_ip, self.port),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProxyStartOptions {
    pub port: u16,
    /// Opt-in LAN listening. The UI and the CLI both leave this off by default.
    pub lan_enabled: bool,
    /// Optional explicit bind address; `--lan` uses 0.0.0.0 when omitted.
    pub bind_ip: Option<IpAddr>,
    /// Optional fixed control token, useful for automated test runs.
    pub control_token: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CaInfo {
    pub dir: PathBuf,
    pub pem_path: PathBuf,
    pub cer_path: PathBuf,
    pub fingerprint: String,
}

#[derive(Clone, Debug)]
pub struct LiveBreakpoint {
    pub id: u64,
    pub rule_name: String,
    pub method: String,
    pub url: String,
    pub headers: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub enum BpDecision {
    Continue {
        method: String,
        url: String,
        headers: String,
        body: String,
    },
    Abort,
}

pub enum ProxyEvent {
    Started { port: u16, ca: CaInfo },
    Stopped,
    Capture(Box<Capture>),
    Log { level: String, message: String },
    Breakpoint(Box<LiveBreakpoint>),
    RemoteConfigUpdated(Box<ProxyConfig>),
    /// A call to the in-band control plane (`/__proxzy/*`), surfaced in the
    /// Agent Control feed so the person can watch an agent drive the engine.
    Control { method: String, path: String, ok: bool },
    Error(String),
}

#[derive(Clone)]
pub struct ProxyConfig {
    pub recording: bool,
    pub map_enabled: bool,
    pub map_rules: Vec<MapRule>,
    pub catch_enabled: bool,
    pub breakpoints: Vec<BreakpointRule>,
    pub ssl_proxying_enabled: bool,
    /// Explicit opt-in "full catch": decrypt every HTTPS host, still keeping
    /// protected Apple system services on a raw tunnel. Off by default so a
    /// host browser is never surprised by an untrusted certificate.
    pub intercept_all: bool,
    pub ssl_allowlist: Vec<String>,
    pub auto_record: bool,
    pub auto_record_domains: Vec<String>,
    pub active_session: Option<TestSession>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            recording: true,
            map_enabled: false,
            map_rules: Vec::new(),
            catch_enabled: false,
            breakpoints: Vec::new(),
            ssl_proxying_enabled: true,
            intercept_all: false,
            ssl_allowlist: Vec::new(),
            auto_record: false,
            auto_record_domains: Vec::new(),
            active_session: None,
        }
    }
}

impl ProxyConfig {
    pub fn should_intercept(&self, host: &str) -> bool {
        if !self.ssl_proxying_enabled {
            return false;
        }
        // Full catch is a deliberate switch in the connection card. It still
        // leaves Apple's protected services on the raw-tunnel path (checked by
        // the caller), so system updates and push keep working.
        if self.intercept_all {
            return true;
        }
        if self.ssl_allowlist.is_empty() {
            return false; // Safe default: blind tunnel everything unless explicitly whitelisted
        }
        self.ssl_allowlist
            .iter()
            .any(|pat| pat == "*" || crate::model::domain_matches(pat, host))
    }
}

struct Ca {
    key: KeyPair,
    cert: Certificate,
    cert_pem: String,
    info: CaInfo,
}

pub struct ProxyHandle {
    pub port: u16,
    pub config: Arc<RwLock<ProxyConfig>>,
    pub ca: CaInfo,
    pub binding: Arc<ListenerBinding>,
    shutdown: Arc<AtomicBool>,
    decisions: Arc<Mutex<HashMap<u64, BpDecision>>>,
    notify: Arc<Notify>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl ProxyHandle {
    pub fn update(&self, cfg: ProxyConfig) {
        if let Ok(mut guard) = self.config.write() {
            *guard = cfg;
        }
    }

    pub fn resolve(&self, id: u64, decision: BpDecision) {
        if let Ok(mut guard) = self.decisions.lock() {
            guard.insert(id, decision);
        }
        self.notify.notify_waiters();
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for ProxyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn app_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    home.join("Library/Application Support/Proxzy")
}

/// Create the local certificate authority once, then reuse it across launches.
pub fn ensure_ca() -> Result<CaInfo, String> {
    let dir = app_data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
    let pem_path = dir.join("proxzy-ca.pem");
    let cer_path = dir.join("proxzy-ca.cer");
    let key_path = dir.join("proxzy-ca-key.pem");

    let (_key, cert_pem) = if pem_path.exists() && key_path.exists() {
        let key_text = std::fs::read_to_string(&key_path)
            .map_err(|e| format!("Cannot read CA key: {}", e))?;
        restrict_permissions(&key_path);
        let cert_text = std::fs::read_to_string(&pem_path)
            .map_err(|e| format!("Cannot read CA certificate: {}", e))?;
        let key = KeyPair::from_pem(&key_text).map_err(|e| format!("Bad CA key: {}", e))?;
        (key, cert_text)
    } else {
        let key = KeyPair::generate().map_err(|e| format!("Cannot generate CA key: {}", e))?;
        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("Cannot build CA: {}", e))?;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "Proxzy Local CA");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Proxzy POC");
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let cert = params
            .self_signed(&key)
            .map_err(|e| format!("Cannot sign CA: {}", e))?;
        let cert_text = cert.pem();
        std::fs::write(&pem_path, &cert_text)
            .map_err(|e| format!("Cannot save CA certificate: {}", e))?;
        write_secret_file(&key_path, key.serialize_pem().as_bytes())
            .map_err(|e| format!("Cannot save CA key: {}", e))?;
        (key, cert_text)
    };

    // A DER copy is what iOS and some Android installers expect.
    if let Ok(der) = pem_to_der(&cert_pem) {
        let _ = std::fs::write(&cer_path, &der);
    }
    let b64 = cert_pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<String>();
    let fingerprint = base64_decode(&b64)
        .map(|der| fingerprint_of(&der))
        .unwrap_or_else(|| "unknown".to_string());

    Ok(CaInfo {
        dir,
        pem_path,
        cer_path,
        fingerprint,
    })
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut reader = Cursor::new(pem.as_bytes());
    for item in rustls_pemfile::certs(&mut reader) {
        let cert = item.map_err(|e| format!("Bad PEM: {}", e))?;
        out.extend_from_slice(cert.as_ref());
    }
    Ok(out)
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for b in text.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' => continue,
            _ => return None,
        };
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

fn fingerprint_of(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    hex::encode_upper(digest)
        .as_bytes()
        .chunks(2)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect::<Vec<_>>()
        .join(":")
}

fn load_ca() -> Result<Ca, String> {
    let info = ensure_ca()?;
    let key_text = std::fs::read_to_string(info.dir.join("proxzy-ca-key.pem"))
        .map_err(|e| format!("Cannot read CA key: {}", e))?;
    let cert_pem = std::fs::read_to_string(&info.pem_path)
        .map_err(|e| format!("Cannot read CA certificate: {}", e))?;
    let key = KeyPair::from_pem(&key_text).map_err(|e| format!("Bad CA key: {}", e))?;
    // Rebuild the issuer parameters from the same identity. The stored PEM is
    // what clients trust; this object only supplies the issuer name/key id.
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("Cannot build CA params: {}", e))?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "Proxzy Local CA");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Proxzy POC");
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let cert = params
        .self_signed(&key)
        .map_err(|e| format!("Cannot rebuild CA signer: {}", e))?;
    Ok(Ca {
        key,
        cert,
        cert_pem,
        info,
    })
}

pub fn start(port: u16, tx: Sender<ProxyEvent>) -> Result<ProxyHandle, String> {
    start_with_options(
        ProxyStartOptions {
            port,
            ..ProxyStartOptions::default()
        },
        tx,
    )
}

pub fn start_with_options(
    options: ProxyStartOptions,
    tx: Sender<ProxyEvent>,
) -> Result<ProxyHandle, String> {
    let ca = load_ca()?;
    let ca_info = ca.info.clone();
    let ca_info_thread = ca_info.clone();

    let bind_ip = options.bind_ip.unwrap_or(if options.lan_enabled {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    });
    let lan_enabled = options.lan_enabled || !bind_ip.is_loopback();
    let policy = if lan_enabled {
        let token = options
            .control_token
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(generate_control_token);
        let token_path = app_data_dir().join("control.token");
        write_control_token(&token_path, &token)?;
        let advertised_ip = crate::devices::host_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        BindPolicy::Lan {
            advertised_ip,
            control_token: token,
        }
    } else {
        BindPolicy::LoopbackOnly
    };
    let binding = Arc::new(ListenerBinding {
        bind_ip,
        port: options.port,
        policy,
    });

    let config = Arc::new(RwLock::new(ProxyConfig::default()));
    let shutdown = Arc::new(AtomicBool::new(false));
    let decisions: Arc<Mutex<HashMap<u64, BpDecision>>> = Arc::new(Mutex::new(HashMap::new()));
    let paused: Arc<Mutex<Vec<LiveBreakpoint>>> = Arc::new(Mutex::new(Vec::new()));
    let notify = Arc::new(Notify::new());
    let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u16, String>>();

    let cfg_thread = config.clone();
    let shutdown_thread = shutdown.clone();
    let decisions_thread = decisions.clone();
    let paused_thread = paused.clone();
    let notify_thread = notify.clone();
    let tx_thread = tx.clone();
    let ca_thread = Arc::new(ca);
    let binding_thread = binding.clone();
    let semaphore_thread = semaphore.clone();

    let join = std::thread::Builder::new()
        .name("proxzy-proxy".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Cannot start proxy runtime: {}", e)));
                    return;
                }
            };
            runtime.block_on(async move {
                let listener = match TcpListener::bind((binding_thread.bind_ip, binding_thread.port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!(
                            "Port {} is already in use: {}",
                            binding_thread.port, e
                        )));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(binding_thread.port));
                let _ = tx_thread.send(ProxyEvent::Started {
                    port: binding_thread.port,
                    ca: ca_info_thread.clone(),
                });
                let _ = tx_thread.send(ProxyEvent::Log {
                    level: "Info".into(),
                    message: if binding_thread.policy.is_lan() {
                        format!(
                            "Proxy listening on {} (LAN opt-in; control token required)",
                            binding_thread.display_address()
                        )
                    } else {
                        format!(
                            "Proxy listening on {} (HTTP + HTTPS, local only)",
                            binding_thread.display_address()
                        )
                    },
                });
                run_listener(
                    listener,
                    cfg_thread,
                    shutdown_thread,
                    decisions_thread,
                    paused_thread,
                    notify_thread,
                    tx_thread,
                    ca_thread,
                    binding_thread,
                    semaphore_thread,
                )
                .await;
            });
        })
        .map_err(|e| format!("Cannot start proxy thread: {}", e))?;

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(bound)) => Ok(ProxyHandle {
            port: bound,
            config,
            ca: ca_info,
            binding,
            shutdown,
            decisions,
            notify,
            join: Some(join),
        }),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(e)
        }
        Err(_) => {
            shutdown.store(true, Ordering::SeqCst);
            Err("Proxy did not start in time".to_string())
        }
    }
}

fn generate_control_token() -> String {
    let mut bytes = [0u8; 32];
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut bytes))
        .is_ok();
    if !filled {
        // A KeyPair is a CSPRNG-backed fallback when /dev/urandom is unavailable.
        if let Ok(key) = KeyPair::generate() {
            let digest = Sha256::digest(key.serialize_der());
            bytes.copy_from_slice(&digest[..32]);
        }
    }
    format!("proxzy_{}", hex::encode(bytes))
}

fn write_control_token(path: &Path, token: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {}", parent.display(), e))?;
    }
    write_secret_file(path, token.as_bytes())
        .map_err(|e| format!("Cannot save control token: {}", e))
}

fn write_secret_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(data)?;
    restrict_permissions(path);
    Ok(())
}

fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let a = left.as_bytes();
    let b = right.as_bytes();
    let mut diff = a.len() ^ b.len();
    let len = a.len().max(b.len());
    for index in 0..len {
        let av = a.get(index).copied().unwrap_or(0);
        let bv = b.get(index).copied().unwrap_or(0);
        diff |= usize::from(av ^ bv);
    }
    diff == 0
}

#[allow(clippy::too_many_arguments)]
async fn run_listener(
    listener: TcpListener,
    config: Arc<RwLock<ProxyConfig>>,
    shutdown: Arc<AtomicBool>,
    decisions: Arc<Mutex<HashMap<u64, BpDecision>>>,
    paused: Arc<Mutex<Vec<LiveBreakpoint>>>,
    notify: Arc<Notify>,
    tx: Sender<ProxyEvent>,
    ca: Arc<Ca>,
    binding: Arc<ListenerBinding>,
    semaphore: Arc<Semaphore>,
) {
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let accepted = tokio::time::timeout(
            Duration::from_millis(250),
            listener.accept(),
        )
        .await;
        match accepted {
            Ok(Ok((stream, peer))) => {
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => break,
                };
                let shutdown_task = shutdown.clone();
                let ctx = ConnCtx {
                    config: config.clone(),
                    decisions: decisions.clone(),
                    paused: paused.clone(),
                    notify: notify.clone(),
                    tx: tx.clone(),
                    ca: ca.clone(),
                    shutdown: shutdown_task,
                    binding: binding.clone(),
                };
                let task_tx = tx.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(e) = handle_connection(stream, peer, ctx).await {
                        // Connection resets are normal when an app is closed.
                        if !e.contains("closed") && !e.contains("reset") {
                            let _ = task_tx.send(ProxyEvent::Log {
                                level: "Warn".into(),
                                message: format!("Proxy connection: {}", e),
                            });
                        }
                    }
                });
            }
            Ok(Err(e)) => {
                let _ = tx.send(ProxyEvent::Error(format!("Proxy listener: {}", e)));
                continue;
            }
            Err(_) => continue,
        }
    }
    let _ = tx.send(ProxyEvent::Stopped);
}

#[derive(Clone)]
struct ConnCtx {
    config: Arc<RwLock<ProxyConfig>>,
    decisions: Arc<Mutex<HashMap<u64, BpDecision>>>,
    paused: Arc<Mutex<Vec<LiveBreakpoint>>>,
    notify: Arc<Notify>,
    tx: Sender<ProxyEvent>,
    ca: Arc<Ca>,
    shutdown: Arc<AtomicBool>,
    binding: Arc<ListenerBinding>,
}

async fn handle_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    ctx: ConnCtx,
) -> Result<(), String> {
    let head = match read_head(&mut stream).await.map_err(|e| e.to_string())? {
        Some(h) => h,
        None => return Ok(()),
    };
    let (head_bytes, _leftover) = split_head(&head);
    let first = first_line(head_bytes);
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_host_port(&target, 443);
        // OPEN-SOURCE SAFETY POLICY:
        // Protected system domains (Apple APNS, OS Updates, OCSP/CRLs) are ALWAYS bound to raw TCP data
        // to prevent certificate errors or device network breakage.
        let is_protected = crate::model::is_builtin_protected_domain(&host);
        let intercept = !is_protected
            && ctx
                .config
                .read()
                .map(|c| c.should_intercept(&host))
                .unwrap_or(false);

        if !intercept {
            return passthrough_tunnel(stream, &host, port, peer, is_protected, ctx).await;
        }

        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: Proxzy\r\n\r\n")
            .await
            .map_err(|e| e.to_string())?;
        let acceptor = tls_acceptor_for(&host, &ctx.ca)?;
        let tls = tokio::time::timeout(Duration::from_secs(15), acceptor.accept(stream))
            .await
            .map_err(|_| "TLS handshake timed out".to_string())?
            .map_err(|e| format!("TLS handshake: {}", e))?;
        let mut tls = tls;
        let request = read_request(&mut tls).await?;
        let Some(request) = request else { return Ok(()) };
        let scheme = "https";
        let url = build_url(scheme, &host, port, &request.target);
        process_request(
            &mut tls,
            request,
            url,
            scheme.to_string(),
            host,
            port,
            peer,
            ctx,
        )
        .await
    } else {
        let (scheme, host, port, url) = absolute_target(&target, head_bytes)?;
        let request = read_request_with_head(&mut stream, head).await?;
        let Some(request) = request else { return Ok(()) };
        process_request(
            &mut stream,
            request,
            url,
            scheme,
            host,
            port,
            peer,
            ctx,
        )
        .await
    }
}

async fn passthrough_tunnel(
    mut client: TcpStream,
    host: &str,
    port: u16,
    peer: SocketAddr,
    is_protected: bool,
    ctx: ConnCtx,
) -> Result<(), String> {
    let started = Instant::now();
    let addr = format!("{}:{}", host, port);
    let upstream_addr = resolve_upstream(host, port, peer, &ctx.binding).await?;
    let mut upstream = tokio::time::timeout(Duration::from_secs(15), TcpStream::connect(upstream_addr))
        .await
        .map_err(|_| format!("Connect to {} timed out", addr))?
        .map_err(|e| format!("Connect to {}: {}", addr, e))?;

    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: Proxzy\r\n\r\n")
        .await
        .map_err(|e| e.to_string())?;

    let _ = ctx.tx.send(ProxyEvent::Log {
        level: "Info".into(),
        message: format!(
            "Passthrough tunnel: {}:{} ({})",
            host,
            port,
            if is_protected {
                "System Protected Raw Data"
            } else {
                "TLS Bypassed"
            }
        ),
    });

    let copy_res = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    let (c2s, s2c) = copy_res.unwrap_or_default();
    let total_bytes = c2s + s2c;
    let latency_ms = started.elapsed().as_millis() as u64;

    let resp_body = if is_protected {
        format!(
            "🛡️ [SYSTEM PROTECTED RAW DATA TUNNEL]\nHost: {}:{}\nTransferred: {} bytes (Client: {} B, Server: {} B)\nDuration: {} ms\n\nPolicy Notice:\nThis domain matches ProxzyCraft's Open-Source Safety Policy (Apple APNS / OCSP / OS Update).\nProxzyCraft safely bound raw TCP data without TLS decryption to preserve host OS and security certificate integrity.",
            host, port, total_bytes, c2s, s2c, latency_ms
        )
    } else {
        format!(
            "🔒 [RAW TCP TUNNEL - TLS BYPASSED]\nHost: {}:{}\nTransferred: {} bytes (Client: {} B, Server: {} B)\nDuration: {} ms\n\nStatus:\nThis domain is currently passed through without decryption because it is not in your SSL Allowlist.\nClick '+ Enable TLS Decryption' in the Inspector to decrypt and inspect HTTP/JSON traffic for this domain.",
            host, port, total_bytes, c2s, s2c, latency_ms
        )
    };

    let capture = Capture {
        id: 0,
        method: "CONNECT".to_string(),
        url: format!("https://{}:{}", host, port),
        status: 200,
        latency_ms: latency_ms.max(1),
        size_bytes: total_bytes,
        kind: ApiKind::Tunnel,
        started: now_stamp(),
        remote: format!("{}:{} via {}", host, port, peer.ip()),
        protocol: "TCP Tunnel (Raw)".to_string(),
        req_headers: vec![
            ("Host".into(), host.to_string()),
            ("Port".into(), port.to_string()),
            (
                "Tunnel-Type".into(),
                if is_protected {
                    "System-Protected".into()
                } else {
                    "Raw-Passthrough".into()
                },
            ),
        ],
        req_body: String::new(),
        resp_headers: vec![
            (
                "Content-Type".into(),
                "application/octet-stream; tunnel=passthrough".into(),
            ),
            (
                "X-Proxzy-Tunnel".into(),
                if is_protected {
                    "Protected-Raw-Data"
                } else {
                    "Bypassed-Raw-Data"
                }
                .into(),
            ),
            ("X-Transferred-Bytes".into(), total_bytes.to_string()),
        ],
        resp_body,
        graphql: None,
        mock: None,
        pinned: false,
        aborted: false,
    };

    if ctx.config.read().map(|c| c.recording).unwrap_or(true) {
        let _ = ctx.tx.send(ProxyEvent::Capture(Box::new(capture)));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_request<S>(
    client: &mut S,
    mut request: HttpRequest,
    mut url: String,
    mut scheme: String,
    mut host: String,
    mut port: u16,
    peer: SocketAddr,
    ctx: ConnCtx,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let started = Instant::now();
    let is_tls = scheme == "https";
    let control_path = origin_path(&url);
    match classify_service_request(&request.target, &host, port, is_tls, &ctx.binding) {
        ServiceRoute::ControlPlane => {
            if let Err(reason) = authorize_control(peer, &request.headers, &ctx.binding) {
                let resp = control_denied(reason);
                let _ = ctx.tx.send(ProxyEvent::Control {
                    method: request.method.clone(),
                    path: control_path.split('?').next().unwrap_or(&control_path).to_string(),
                    ok: false,
                });
                write_response(client, &resp, request.method == "HEAD").await?;
                return Ok(());
            }
            let resp = handle_proxzy_control(&request, &control_path, &ctx);
            let _ = ctx.tx.send(ProxyEvent::Control {
                method: request.method.clone(),
                path: control_path.split('?').next().unwrap_or(&control_path).to_string(),
                ok: resp.status < 400,
            });
            write_response(client, &resp, request.method == "HEAD").await?;
            return Ok(());
        }
        ServiceRoute::Portal => {
            let resp = handle_portal(&request, &control_path, &ctx);
            write_response(client, &resp, request.method == "HEAD").await?;
            return Ok(());
        }
        ServiceRoute::ProxyForward => {}
    }

    // 0) A local self-test target. The connection card sends one real request
    // through the listener; the proxy answers it and records it, so "can you
    // see my traffic?" is answered with evidence instead of a guess. This is
    // deliberately local: the check works with no network and no certificate.
    if host.eq_ignore_ascii_case("proxzy.gateway") {
        let nonce = query_value(&url, "nonce").unwrap_or_default();
        let body = format!(
            "{{\"ok\":true,\"gate\":\"proxzy\",\"nonce\":\"{}\",\"note\":\"This request travelled through the proxy and was recorded.\"}}",
            escape_json(&nonce)
        );
        let headers = vec![
            ("content-type".into(), "application/json".into()),
            ("x-proxzy-selftest".into(), "capture".into()),
            ("cache-control".into(), "no-store".into()),
        ];
        let resp = make_response(200, "OK", headers.clone(), body.clone().into_bytes());
        write_response(client, &resp, request.method.eq_ignore_ascii_case("HEAD")).await?;

        let latency = started.elapsed().as_millis() as u64;
        let capture = Capture {
            id: 0,
            method: request.method.clone(),
            url: url.clone(),
            status: 200,
            latency_ms: latency.max(1),
            size_bytes: body.len() as u64,
            kind: ApiKind::Rest,
            started: now_stamp(),
            remote: "proxzy.gateway · local gate".into(),
            protocol: "HTTP/1.1".into(),
            req_headers: request.headers.clone(),
            req_body: String::from_utf8_lossy(&request.body).to_string(),
            resp_headers: headers,
            resp_body: body,
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        };
        if ctx.config.read().map(|c| c.recording).unwrap_or(true) {
            let _ = ctx.tx.send(ProxyEvent::Capture(Box::new(capture)));
        }
        let _ = ctx.tx.send(ProxyEvent::Log {
            level: "Info".into(),
            message: "Capture check: request reached the gate and was recorded".into(),
        });
        return Ok(());
    }

    let mut mock_name: Option<String> = None;
    let mut mock_response: Option<HttpResponse> = None;
    let mut aborted = false;

    // 1) A breakpoint can pause the live request and hand it back to the UI.
    if let Some(rule) = matching_breakpoint(&ctx, &request, &url) {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let pending = LiveBreakpoint {
            id,
            rule_name: rule.name.clone(),
            method: request.method.clone(),
            url: url.clone(),
            headers: request
                .headers
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            body: String::from_utf8_lossy(&request.body).to_string(),
        };
        if let Ok(mut p) = ctx.paused.lock() {
            p.push(pending.clone());
        }
        let _ = ctx.tx.send(ProxyEvent::Breakpoint(Box::new(pending.clone())));
        let _ = ctx.tx.send(ProxyEvent::Log {
            level: "Warn".into(),
            message: format!("Paused {} {} at \"{}\"", request.method, url, rule.name),
        });
        let decision = wait_for_decision(&ctx, id).await;
        if let Ok(mut p) = ctx.paused.lock() {
            p.retain(|x| x.id != id);
        }
        match decision {
            Some(BpDecision::Abort) => {
                let resp = make_response(
                    403,
                    "Forbidden",
                    vec![("content-type".into(), "application/json".into())],
                    br#"{"error":"Request blocked by Proxzy breakpoint"}"#.to_vec(),
                );
                write_response(client, &resp, false).await?;
                aborted = true;
            }
            Some(BpDecision::Continue {
                method,
                url: new_url,
                headers,
                body,
            }) => {
                request.method = method;
                request.headers = parse_header_text(&headers);
                request.body = body.into_bytes();
                if new_url.starts_with("http://") || new_url.starts_with("https://") {
                    if let Some((s, h, p, _)) = parse_absolute(&new_url) {
                        scheme = s;
                        host = h;
                        port = p;
                        url = new_url;
                    }
                }
            }
            None => {
                // Nobody answered in time: let the original request through.
            }
        }
    }

    // 2) Map Local answers matching live requests without touching the network.
    if !aborted {
        if let Some(rule) = matching_map_rule(&ctx, &request, &url) {
            let delay = rule.delay_ms.min(10_000);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            let mut headers = vec![
                ("content-type".to_string(), rule.content_type.clone()),
                ("x-proxzy-rule".to_string(), rule.name.clone()),
            ];
            if let Some(origin) = request
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("origin"))
            {
                headers.push(("access-control-allow-origin".into(), origin.1.clone()));
            }
            let rendered_body = rule.response_body();
            let resp = make_response(rule.status, reason(rule.status), headers, rendered_body.into_bytes());
            write_response(client, &resp, request.method == "HEAD").await?;
            mock_name = Some(rule.name.clone());
            mock_response = Some(resp);
            let latency = started.elapsed().as_millis() as u64;
            let scenario_label = match rule.active_scenario {
                MockScenario::Success => "success",
                MockScenario::Error => "error",
                MockScenario::Empty => "empty",
            };
            if let Ok(mut cfg) = ctx.config.write() {
                if let Some(ref mut sess) = cfg.active_session {
                    let req_path = url.split("://").nth(1).and_then(|s| s.find('/')).map(|idx| &url[idx..]).unwrap_or("/");
                    if sess.matches_scope(&host, req_path) {
                        sess.record_call(SessionCallAudit {
                            timestamp: now_stamp(),
                            method: request.method.clone(),
                            url: url.clone(),
                            status: rule.status,
                            latency_ms: latency,
                            source: format!("MOCK (rule: '{}', scenario: '{}')", rule.name, scenario_label),
                            app_version: sess.app_version.clone(),
                            baseline_version: sess.baseline_version.clone(),
                            env: sess.env.clone(),
                            is_drift_detected: false,
                            drift_notes: Vec::new(),
                            breaking_changes_count: 0,
                            additive_changes_count: 0,
                        });
                    }
                }
            }
            let _ = ctx.tx.send(ProxyEvent::Log {
                level: "Warn".into(),
                message: format!("Map Local answered {} with \"{}\"", url, rule.name),
            });
        }
    }

    if !aborted && mock_name.is_none() {
        // 3) Forward to the real server, then answer the client. The practice
        // host is answered by the proxy itself, after Map Local and Breakpoints
        // have had their turn, so the learning demo exercises the real path.
        let is_practice = host.eq_ignore_ascii_case(PRACTICE_HOST);
        let resp = if is_practice {
            practice_response(&request, &url).await
        } else {
            match forward_request(&request, &scheme, &host, port, &url, peer, &ctx.binding).await {
                Ok(r) => r,
                Err(e) => {
                    let body = format!("{{\"error\":\"Proxzy could not reach the server\",\"detail\":\"{}\"}}", escape_json(&e));
                    let resp = make_response(
                        502,
                        "Bad Gateway",
                        vec![("content-type".into(), "application/json".into())],
                        body.into_bytes(),
                    );
                    let _ = ctx.tx.send(ProxyEvent::Log {
                        level: "Error".into(),
                        message: format!("{} {} failed: {}", request.method, url, e),
                    });
                    resp
                }
            }
        };
        write_response(client, &resp, request.method == "HEAD").await?;

        let latency = started.elapsed().as_millis() as u64;
        let body_text = decode_body(&resp.body, header_value(&resp.headers, "content-encoding"));
        let capture = build_capture(
            &request,
            &url,
            &host,
            port,
            peer,
            resp.status,
            latency,
            &resp.headers,
            &body_text,
            mock_name.clone(),
            aborted,
            scheme == "https",
        );
        if ctx.config.read().map(|c| c.recording).unwrap_or(true) {
            let _ = ctx.tx.send(ProxyEvent::Capture(Box::new(capture)));
        }

        let mut matching_mock_body = None;
        if let Ok(cfg) = ctx.config.read() {
            if let Some(r) = cfg.map_rules.iter().find(|r| {
                r.method.matches(&request.method) && crate::model::pattern_matches_url(&r.pattern, &url)
            }) {
                matching_mock_body = Some(r.body.clone());
            }
        }
        let drift_report = if let Some(ref m_body) = matching_mock_body {
            crate::model::analyze_contract_drift(m_body, &body_text)
        } else {
            crate::model::ContractDriftReport {
                is_drift_detected: false,
                missing_in_live: Vec::new(),
                added_in_live: Vec::new(),
                notes: Vec::new(),
            }
        };
        let is_drift_detected = drift_report.is_drift_detected;
        let drift_notes = drift_report.notes;
        let breaking_changes_count = drift_report.missing_in_live.len();
        let additive_changes_count = drift_report.added_in_live.len();
        let mut was_auto_recorded = false;

        if !is_practice && resp.status >= 200 && resp.status < 300 {
            if let Ok(mut cfg) = ctx.config.write() {
                let req_path = url.split("://").nth(1).and_then(|s| s.find('/')).map(|idx| &url[idx..]).unwrap_or("/");
                let should_auto_record = cfg.auto_record
                    && (cfg.auto_record_domains.is_empty()
                        || cfg
                            .auto_record_domains
                            .iter()
                            .any(|d| crate::model::domain_matches(d, &host)))
                    && cfg.active_session.as_ref().map(|s| s.matches_scope(&host, req_path)).unwrap_or(true);
                if should_auto_record {
                    let wildcard_pattern = make_environment_wildcard_pattern(&url);
                    if !cfg.map_rules.iter().any(|r| r.pattern == wildcard_pattern) {
                        let sanitized_body = sanitize_mock_payload(&body_text);
                        let empty_body = auto_derive_empty_scenario(&body_text);
                        let folder = cfg
                            .active_session
                            .as_ref()
                            .map(|s| s.folder.clone())
                            .unwrap_or_else(|| {
                                extract_domain_from_pattern(&wildcard_pattern)
                                    .unwrap_or_else(|| "General".into())
                            });
                        let (app_ver, env_name) = cfg
                            .active_session
                            .as_ref()
                            .map(|s| (s.app_version.clone(), s.env.clone()))
                            .unwrap_or_else(|| (String::new(), String::new()));
                        let next_id = cfg.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                        let rule = MapRule {
                            id: next_id,
                            name: format!("Auto Mock ({} {})", request.method, host),
                            folder,
                            pattern: wildcard_pattern,
                            method: match request.method.to_ascii_uppercase().as_str() {
                                "GET" => RuleMethod::Get,
                                "POST" => RuleMethod::Post,
                                "PUT" => RuleMethod::Put,
                                "DELETE" => RuleMethod::Delete,
                                _ => RuleMethod::Any,
                            },
                            status: resp.status,
                            content_type: header_value(&resp.headers, "content-type")
                                .unwrap_or_else(|| "application/json".into()),
                            delay_ms: 0,
                            body: sanitized_body.clone(),
                            local_file: String::new(),
                            graphql_name: String::new(),
                            enabled: true,
                            hits: 0,
                            active_scenario: MockScenario::Success,
                            success_status: resp.status,
                            success_body: sanitized_body,
                            error_status: 500,
                            error_body: r#"{"error":"Internal Server Error","status":500}"#.into(),
                            empty_status: 200,
                            empty_body,
                            inject_props: std::collections::HashMap::new(),
                            app_version: app_ver,
                            env: env_name,
                        };
                        cfg.map_rules.push(rule);
                        cfg.map_enabled = true;
                        was_auto_recorded = true;
                        let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
                        let _ = ctx.tx.send(ProxyEvent::Log {
                            level: "Info".into(),
                            message: format!("Auto-recorded live 200 OK mock for {}", url),
                        });
                    }
                }
            }
        }

        if let Ok(mut cfg) = ctx.config.write() {
            if let Some(ref mut sess) = cfg.active_session {
                let req_path = url.split("://").nth(1).and_then(|s| s.find('/')).map(|idx| &url[idx..]).unwrap_or("/");
                if sess.matches_scope(&host, req_path) {
                    sess.record_call(SessionCallAudit {
                        timestamp: now_stamp(),
                        method: request.method.clone(),
                        url: url.clone(),
                        status: resp.status,
                        latency_ms: latency,
                        source: if was_auto_recorded {
                            "AUTO_RECORDED (200 OK -> Mock Created)".into()
                        } else {
                            format!("UPSTREAM ({} {})", resp.status, reason(resp.status))
                        },
                        app_version: sess.app_version.clone(),
                        baseline_version: sess.baseline_version.clone(),
                        env: sess.env.clone(),
                        is_drift_detected,
                        drift_notes,
                        breaking_changes_count,
                        additive_changes_count,
                    });
                }
            }
        }
        return Ok(());
    }

    // Mock / abort path: still record the real bytes that were served.
    let latency = started.elapsed().as_millis() as u64;
    let (status, resp_headers, resp_body) = if let Some(resp) = &mock_response {
        (
            resp.status,
            resp.headers.clone(),
            decode_body(&resp.body, header_value(&resp.headers, "content-encoding")),
        )
    } else {
        (
            403,
            vec![("content-type".to_string(), "application/json".to_string())],
            "{\"error\":\"Request blocked by Proxzy breakpoint\"}".to_string(),
        )
    };
    let mut capture = build_capture(
        &request,
        &url,
        &host,
        port,
        peer,
        status,
        latency,
        &resp_headers,
        &resp_body,
        mock_name.clone(),
        aborted,
        scheme == "https",
    );
    if let Some(name) = mock_name {
        capture.mock = Some(name);
    }
    if ctx.config.read().map(|c| c.recording).unwrap_or(true) {
        let _ = ctx.tx.send(ProxyEvent::Capture(Box::new(capture)));
    }
    Ok(())
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn matching_breakpoint(ctx: &ConnCtx, req: &HttpRequest, url: &str) -> Option<BreakpointRule> {
    let cfg = ctx.config.read().ok()?;
    if !cfg.catch_enabled {
        return None;
    }
    let body = String::from_utf8_lossy(&req.body);
    cfg.breakpoints
        .iter()
        .find(|r| r.enabled && r.matches(&req.method, url, &body))
        .cloned()
}

fn matching_map_rule(ctx: &ConnCtx, req: &HttpRequest, url: &str) -> Option<MapRule> {
    let mut cfg = ctx.config.write().ok()?;
    if !cfg.map_enabled {
        return None;
    }
    let body = String::from_utf8_lossy(&req.body);
    let matched = cfg
        .map_rules
        .iter_mut()
        .find(|r| r.enabled && r.matches(&req.method, url, &body))?;
    matched.hits += 1;
    let out = matched.clone();
    let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
    Some(out)
}

/// Extract a string value `"key": "value"` from a lightweight JSON payload without external crates.
fn json_str_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let idx = json.find(&needle)?;
    let rest = &json[idx + needle.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if let Some(stripped) = after.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = stripped.chars();
        while let Some(c) = chars.next() {
            if c == '"' {
                return Some(out);
            }
            if c == '\\' {
                if let Some(esc) = chars.next() {
                    match esc {
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        other => {
                            out.push('\\');
                            out.push(other);
                        }
                    }
                }
            } else {
                out.push(c);
            }
        }
        None
    } else {
        None
    }
}

/// Extract a numeric value `"key": 200` from a JSON payload.
fn json_u16_field(json: &str, key: &str) -> Option<u16> {
    let needle = format!("\"{}\"", key);
    let idx = json.find(&needle)?;
    let rest = &json[idx + needle.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let digits: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u16>().ok()
}

/// Extract key-value mapping from `"inject": { "key": "val" }` in lightweight JSON.
fn json_inject_map(json: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let needle = "\"inject\"";
    let Some(idx) = json.find(needle) else {
        return map;
    };
    let rest = &json[idx + needle.len()..];
    let Some(colon) = rest.find(':') else {
        return map;
    };
    let after_colon = rest[colon + 1..].trim_start();
    let Some(brace_start) = after_colon.find('{') else {
        return map;
    };
    let mut depth = 0;
    let mut obj_end = None;
    for (i, c) in after_colon[brace_start..].char_indices() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                obj_end = Some(brace_start + i);
                break;
            }
        }
    }
    let Some(end_idx) = obj_end else {
        return map;
    };
    let obj_str = &after_colon[brace_start + 1..end_idx];
    for pair in obj_str.split(',') {
        if let Some((k, v)) = pair.split_once(':') {
            let clean_k = k.trim().trim_matches('"').trim();
            let clean_v = v.trim().trim_matches('"').trim();
            if !clean_k.is_empty() {
                map.insert(clean_k.to_string(), clean_v.to_string());
            }
        }
    }
    map
}

/// Extract an array of strings from `"key": ["item1", "item2"]` in lightweight JSON.
fn json_str_array(json: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{}\"", key);
    let Some(idx) = json.find(&needle) else {
        return Vec::new();
    };
    let rest = &json[idx + needle.len()..];
    let Some(open_bracket) = rest.find('[') else {
        return Vec::new();
    };
    let after_bracket = &rest[open_bracket + 1..];
    let Some(close_bracket) = after_bracket.find(']') else {
        return Vec::new();
    };
    let array_content = &after_bracket[..close_bracket];
    let mut out = Vec::new();
    let mut inside = false;
    let mut current = String::new();
    for c in array_content.chars() {
        if c == '"' {
            if inside {
                inside = false;
                let clean = current.trim();
                if !clean.is_empty() {
                    out.push(clean.to_string());
                }
                current.clear();
            } else {
                inside = true;
            }
        } else if inside {
            current.push(c);
        }
    }
    out
}

/// Where a request that arrives at the proxy port should go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceRoute {
    /// Local JSON API used by agents and test runners (`/__proxzy/*`).
    ControlPlane,
    /// Read-only certificate and onboarding pages for phones.
    Portal,
    /// Everything else is normal proxied traffic.
    ProxyForward,
}

fn origin_path_or_target(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        origin_path(target)
    } else {
        target.to_string()
    }
}

fn host_without_port(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return &rest[..end];
        }
    }
    if let Some((name, port)) = host.rsplit_once(':') {
        if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return name;
        }
    }
    host
}

fn is_local_host(host: &str, binding: &ListenerBinding) -> bool {
    let name = host_without_port(host).to_ascii_lowercase();
    if name == "localhost" || name == "proxzy.local" || name == "127.0.0.1" || name == "::1" {
        return true;
    }
    if name == binding.bind_ip.to_string().to_ascii_lowercase() {
        return true;
    }
    match binding.policy.advertised_ip() {
        Some(ip) => name == ip.to_ascii_lowercase(),
        None => false,
    }
}

/// Classify a request without touching the network.
///
/// The control plane and the certificate portal are only reachable over a
/// direct, unencrypted connection to a local host. Traffic that was decrypted
/// from a CONNECT tunnel (`is_tls`) is always forwarded, so a site under test
/// cannot smuggle `/__proxzy/*` into the local engine.
pub fn classify_service_request(
    target: &str,
    host: &str,
    _port: u16,
    is_tls: bool,
    binding: &ListenerBinding,
) -> ServiceRoute {
    if is_tls {
        return ServiceRoute::ProxyForward;
    }
    let path = origin_path_or_target(target);
    let path_only = path.split('?').next().unwrap_or(&path);
    let absolute = target.starts_with("http://") || target.starts_with("https://");
    let local_host = is_local_host(host, binding);
    let via_portal_host = local_host || host_without_port(host).eq_ignore_ascii_case("proxzy.local");

    if path_only.starts_with("/__proxzy") && (!absolute || via_portal_host) {
        return ServiceRoute::ControlPlane;
    }
    if via_portal_host
        && matches!(
            path_only,
            "/" | "/ca.cer" | "/ca.pem" | "/ssl" | "/proxzy.local"
        )
    {
        return ServiceRoute::Portal;
    }
    ServiceRoute::ProxyForward
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        other => other,
    }
}

/// Control plane authorisation: exact local Host, no browser Origin, and either
/// a loopback peer or a matching control token. Applied to GET and POST alike.
pub fn authorize_control(
    peer: SocketAddr,
    headers: &[(String, String)],
    binding: &ListenerBinding,
) -> Result<(), &'static str> {
    if header(headers, "origin").is_some() {
        return Err("browser origin is not allowed on the control plane");
    }
    let Some(host) = header(headers, "host") else {
        return Err("missing Host header");
    };
    if !is_local_host(host, binding) {
        return Err("Host does not match the local proxy");
    }
    if canonical_ip(peer.ip()).is_loopback() {
        return Ok(());
    }
    let supplied = header(headers, "x-proxzy-control").or_else(|| {
        header(headers, "authorization").and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
    });
    match (supplied, binding.policy.control_token()) {
        (Some(given), Some(expected)) if constant_time_eq(given, expected) => Ok(()),
        (Some(_), Some(_)) => Err("control token is incorrect"),
        _ => Err("a control token is required for LAN callers"),
    }
}

fn control_denied(reason: &str) -> HttpResponse {
    let body = format!(
        "{{\"ok\":false,\"error\":\"Forbidden: {}\"}}",
        escape_json(reason)
    );
    make_response(
        403,
        "Forbidden",
        vec![
            ("content-type".into(), "application/json".into()),
            ("cache-control".into(), "no-store".into()),
            ("x-content-type-options".into(), "nosniff".into()),
        ],
        body.into_bytes(),
    )
}

fn handle_portal(req: &HttpRequest, path: &str, ctx: &ConnCtx) -> HttpResponse {
    let path_only = path.split('?').next().unwrap_or(path);
    let ca = &ctx.ca.info;
    let mut headers = vec![
        ("cache-control".to_string(), "no-store".to_string()),
        ("x-content-type-options".to_string(), "nosniff".to_string()),
        ("connection".to_string(), "close".to_string()),
    ];
    if req.method != "GET" && req.method != "HEAD" {
        return make_response(
            405,
            "Method Not Allowed",
            headers,
            br#"{"ok":false,"error":"The onboarding portal is read-only"}"#.to_vec(),
        );
    }
    match path_only {
        "/ca.cer" => {
            let der = std::fs::read(&ca.cer_path).unwrap_or_default();
            headers.push((
                "content-type".to_string(),
                "application/x-x509-ca-cert".to_string(),
            ));
            headers.push((
                "content-disposition".to_string(),
                "attachment; filename=\"proxzy-ca.cer\"".to_string(),
            ));
            make_response(200, "OK", headers, der)
        }
        "/ca.pem" => {
            let pem = std::fs::read(&ca.pem_path).unwrap_or_default();
            headers.push(("content-type".to_string(), "application/x-pem-file".to_string()));
            headers.push((
                "content-disposition".to_string(),
                "attachment; filename=\"proxzy-ca.pem\"".to_string(),
            ));
            make_response(200, "OK", headers, pem)
        }
        _ => {
            let host = ctx.binding.display_address();
            let token_hint = if ctx.binding.policy.is_lan() {
                "LAN mode is on. Keep this page on a trusted Wi-Fi network."
            } else {
                "This proxy is local only. Turn on LAN access in Proxzy to reach it from a phone."
            };
            let html = format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Proxzy certificate</title><style>body{{font-family:-apple-system,system-ui,sans-serif;background:#0f1115;color:#e8edf5;margin:0;padding:32px}}main{{max-width:620px;margin:auto}}h1{{font-size:24px}}a{{color:#6ea8ff}}code{{background:#1a1f27;padding:2px 6px;border-radius:4px}}.card{{background:#161b22;border:1px solid #2a313c;border-radius:12px;padding:20px;margin:18px 0}}</style></head><body><main><h1>Proxzy local certificate</h1><p>Install this certificate only on devices you own. It lets Proxzy read HTTPS traffic from the apps you are testing.</p><div class=\"card\"><p><strong>iPhone / iPad</strong><br>Open <a href=\"/ca.cer\">/ca.cer</a>, allow the profile, then trust it in Settings &rsaquo; General &rsaquo; About &rsaquo; Certificate Trust Settings.</p><p><strong>Android</strong><br>Download <a href=\"/ca.cer\">/ca.cer</a>, then install it under Security &rsaquo; Encryption &amp; credentials &rsaquo; Install a certificate &rsaquo; CA certificate.</p><p><strong>Command line</strong><br><a href=\"/ca.pem\">/ca.pem</a></p></div><div class=\"card\"><p>Proxy address: <code>{}</code></p><p>{}</p><p>SHA-256: <code>{}</code></p></div></main></body></html>",
                host, token_hint, ca.fingerprint
            );
            headers.push(("content-type".to_string(), "text/html; charset=utf-8".to_string()));
            make_response(200, "OK", headers, html.into_bytes())
        }
    }
}

/// Resolve a host once and return only an address that is safe to connect to.
///
/// This blocks the proxy from being used as a bounce into its own listener, into
/// loopback from a LAN caller, or into link-local/unspecified addresses, while
/// still allowing a developer on the same Mac to reach a local test server.
pub async fn resolve_upstream(
    host: &str,
    port: u16,
    peer: SocketAddr,
    binding: &ListenerBinding,
) -> Result<SocketAddr, String> {
    let mut addresses = Vec::new();
    if let Ok(ip) = host.parse::<IpAddr>() {
        addresses.push(SocketAddr::new(ip, port));
    } else {
        let resolved = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| format!("DNS lookup for {} timed out", host))?
        .map_err(|e| format!("DNS lookup for {}: {}", host, e))?;
        addresses.extend(resolved);
    }
    for addr in addresses {
        if !is_blocked_destination(addr, peer, binding) {
            return Ok(addr);
        }
    }
    Err(format!(
        "Blocked upstream destination {}:{} (loopback, link-local or the proxy itself)",
        host, port
    ))
}

fn is_blocked_destination(addr: SocketAddr, peer: SocketAddr, binding: &ListenerBinding) -> bool {
    let ip = canonical_ip(addr.ip());
    let peer_loopback = canonical_ip(peer.ip()).is_loopback();
    if ip.is_unspecified() {
        return true;
    }
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_link_local() {
                return true;
            }
            if v4.is_loopback() && !peer_loopback {
                return true;
            }
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            if (first & 0xffc0) == 0xfe80 {
                return true;
            }
            if v6.is_loopback() && !peer_loopback {
                return true;
            }
        }
    }
    // Never let the proxy connect back into its own listener.
    if addr.port() == binding.port {
        let advertised = binding
            .policy
            .advertised_ip()
            .and_then(|value| value.parse::<IpAddr>().ok());
        if ip.is_loopback()
            || ip == canonical_ip(binding.bind_ip)
            || advertised.map(|a| canonical_ip(a) == ip).unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Handle Testful / WireMock-like HTTP control requests sent to `/__proxzy/*`.
/// Allows MobileWright, `mobile-mcp`, XCTest, Espresso, or `curl` to inspect proxy state,
/// dynamically inject or switch mock scenarios (`success` / `error` / `empty` / `passthrough`),
/// auto-whitelist HTTPS domains, and verify hit counts on the same port (`127.0.0.1:9090`).
fn handle_proxzy_control(req: &HttpRequest, control_path: &str, ctx: &ConnCtx) -> HttpResponse {
    let path = control_path.split('?').next().unwrap_or(control_path);
    let body_text = String::from_utf8_lossy(&req.body).to_string();
    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("cache-control".to_string(), "no-store".to_string()),
        ("x-content-type-options".to_string(), "nosniff".to_string()),
    ];

    if req.method == "OPTIONS" {
        return make_response(204, "No Content", headers, Vec::new());
    }

    if req.method == "GET" && (path == "/__proxzy/health" || path == "/__proxzy/status" || path == "/__proxzy") {
        let cfg = ctx.config.read().map(|c| c.clone()).unwrap_or_default();
        let allowlist_json = cfg
            .ssl_allowlist
            .iter()
            .map(|d| format!("\"{}\"", escape_json(d)))
            .collect::<Vec<_>>()
            .join(",");
        let rules_json = cfg
            .map_rules
            .iter()
            .map(|r| {
                format!(
                    "{{\"id\":{},\"name\":\"{}\",\"folder\":\"{}\",\"pattern\":\"{}\",\"method\":\"{}\",\"scenario\":\"{}\",\"status\":{},\"enabled\":{},\"hits\":{},\"inject_count\":{},\"fixture\":\"{}\"}}",
                    r.id,
                    escape_json(&r.name),
                    escape_json(r.folder_name()),
                    escape_json(&r.pattern),
                    r.method.label(),
                    r.active_scenario.slug(),
                    r.status,
                    r.enabled,
                    r.hits,
                    r.inject_props.len(),
                    escape_json(&r.fixture_path())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let auto_record_domains_json = cfg
            .auto_record_domains
            .iter()
            .map(|d| format!("\"{}\"", escape_json(d)))
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            "{{\"ok\":true,\"engine\":\"ProxzyCraft\",\"recording\":{},\"auto_record\":{},\"auto_record_domains\":[{}],\"map_enabled\":{},\"ssl_proxying_enabled\":{},\"ssl_allowlist\":[{}],\"rules_count\":{},\"rules\":[{}]}}",
            cfg.recording,
            cfg.auto_record,
            auto_record_domains_json,
            cfg.map_enabled,
            cfg.ssl_proxying_enabled,
            allowlist_json,
            cfg.map_rules.len(),
            rules_json
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/record" {
        let enabled = !body_text.contains("\"enabled\":false") && !body_text.contains("\"enabled\": false");
        let domains = json_str_array(&body_text, "domains");
        if let Ok(mut cfg) = ctx.config.write() {
            cfg.auto_record = enabled;
            if !domains.is_empty() {
                cfg.auto_record_domains = domains.clone();
                for d in &domains {
                    if !cfg.ssl_allowlist.iter().any(|existing| existing.eq_ignore_ascii_case(d)) {
                        cfg.ssl_allowlist.push(d.clone());
                    }
                }
            }
            let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
        }
        return make_response(200, "OK", headers, br#"{"ok":true,"auto_record_updated":true}"#.to_vec());
    }

    if req.method == "POST" && path == "/__proxzy/session/start" {
        let name = json_str_field(&body_text, "name").unwrap_or_else(|| "TestSession".into());
        let folder = json_str_field(&body_text, "folder").unwrap_or_else(|| "General".into());
        let app_version = json_str_field(&body_text, "app_version").unwrap_or_default();
        let baseline_version = json_str_field(&body_text, "baseline_version").unwrap_or_default();
        let env = json_str_field(&body_text, "env").unwrap_or_default();
        let domains = json_str_array(&body_text, "domains");
        let paths = json_str_array(&body_text, "paths");

        if let Ok(mut cfg) = ctx.config.write() {
            for d in &domains {
                if !cfg.ssl_allowlist.iter().any(|existing| existing.eq_ignore_ascii_case(d)) {
                    cfg.ssl_allowlist.push(d.clone());
                }
            }
            cfg.active_session = Some(TestSession::new(
                &name,
                &folder,
                &app_version,
                &baseline_version,
                &env,
                domains,
                paths,
            ));
            let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
            let _ = ctx.tx.send(ProxyEvent::Log {
                level: "Info".into(),
                message: format!(
                    "Started Test Session \"{}\" (v{} vs baseline v{} @ {})",
                    name, app_version, baseline_version, env
                ),
            });
        }
        let json = format!(
            "{{\"ok\":true,\"session_started\":true,\"name\":\"{}\",\"folder\":\"{}\",\"app_version\":\"{}\",\"baseline_version\":\"{}\",\"env\":\"{}\"}}",
            escape_json(&name),
            escape_json(&folder),
            escape_json(&app_version),
            escape_json(&baseline_version),
            escape_json(&env)
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "GET" && path == "/__proxzy/session/summary" {
        let Ok(cfg) = ctx.config.read() else {
            return make_response(500, "Internal Server Error", headers, br#"{"ok":false}"#.to_vec());
        };
        let Some(ref sess) = cfg.active_session else {
            return make_response(200, "OK", headers, br#"{"ok":true,"active":false,"message":"No active session"}"#.to_vec());
        };
        let calls_json = sess
            .audit_log
            .iter()
            .map(|c| {
                let drifts_json = c
                    .drift_notes
                    .iter()
                    .map(|d| format!("\"{}\"", escape_json(d)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"timestamp\":\"{}\",\"method\":\"{}\",\"url\":\"{}\",\"status\":{},\"latency_ms\":{},\"source\":\"{}\",\"app_version\":\"{}\",\"baseline_version\":\"{}\",\"env\":\"{}\",\"is_drift_detected\":{},\"breaking_changes\":{},\"additive_changes\":{},\"drift_notes\":[{}]}}",
                    escape_json(&c.timestamp),
                    escape_json(&c.method),
                    escape_json(&c.url),
                    c.status,
                    c.latency_ms,
                    escape_json(&c.source),
                    escape_json(&c.app_version),
                    escape_json(&c.baseline_version),
                    escape_json(&c.env),
                    c.is_drift_detected,
                    c.breaking_changes_count,
                    c.additive_changes_count,
                    drifts_json
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            "{{\"ok\":true,\"active\":true,\"name\":\"{}\",\"folder\":\"{}\",\"app_version\":\"{}\",\"baseline_version\":\"{}\",\"env\":\"{}\",\"started_at\":\"{}\",\"total_calls\":{},\"mock_calls\":{},\"upstream_calls\":{},\"auto_recorded_calls\":{},\"drift_warnings\":{},\"breaking_drifts\":{},\"additive_drifts\":{},\"calls\":[{}]}}",
            escape_json(&sess.name),
            escape_json(&sess.folder),
            escape_json(&sess.app_version),
            escape_json(&sess.baseline_version),
            escape_json(&sess.env),
            escape_json(&sess.started_at),
            sess.total_calls,
            sess.mock_calls,
            sess.upstream_calls,
            sess.auto_recorded_calls,
            sess.drift_warnings,
            sess.breaking_drifts,
            sess.additive_drifts,
            calls_json
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/session/end" {
        let summary_json = if let Ok(mut cfg) = ctx.config.write() {
            if let Some(sess) = cfg.active_session.take() {
                let calls_json = sess
                    .audit_log
                    .iter()
                    .map(|c| {
                        let drifts_json = c
                            .drift_notes
                            .iter()
                            .map(|d| format!("\"{}\"", escape_json(d)))
                            .collect::<Vec<_>>()
                            .join(",");
                        format!(
                            "{{\"timestamp\":\"{}\",\"method\":\"{}\",\"url\":\"{}\",\"status\":{},\"latency_ms\":{},\"source\":\"{}\",\"app_version\":\"{}\",\"baseline_version\":\"{}\",\"env\":\"{}\",\"is_drift_detected\":{},\"breaking_changes\":{},\"additive_changes\":{},\"drift_notes\":[{}]}}",
                            escape_json(&c.timestamp),
                            escape_json(&c.method),
                            escape_json(&c.url),
                            c.status,
                            c.latency_ms,
                            escape_json(&c.source),
                            escape_json(&c.app_version),
                            escape_json(&c.baseline_version),
                            escape_json(&c.env),
                            c.is_drift_detected,
                            c.breaking_changes_count,
                            c.additive_changes_count,
                            drifts_json
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let s = format!(
                    "{{\"ok\":true,\"session_ended\":true,\"name\":\"{}\",\"folder\":\"{}\",\"app_version\":\"{}\",\"baseline_version\":\"{}\",\"env\":\"{}\",\"total_calls\":{},\"mock_calls\":{},\"upstream_calls\":{},\"auto_recorded_calls\":{},\"drift_warnings\":{},\"breaking_drifts\":{},\"additive_drifts\":{},\"calls\":[{}]}}",
                    escape_json(&sess.name),
                    escape_json(&sess.folder),
                    escape_json(&sess.app_version),
                    escape_json(&sess.baseline_version),
                    escape_json(&sess.env),
                    sess.total_calls,
                    sess.mock_calls,
                    sess.upstream_calls,
                    sess.auto_recorded_calls,
                    sess.drift_warnings,
                    sess.breaking_drifts,
                    sess.additive_drifts,
                    calls_json
                );
                let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
                let _ = ctx.tx.send(ProxyEvent::Log {
                    level: "Info".into(),
                    message: format!("Ended Test Session \"{}\" ({} calls recorded)", sess.name, sess.total_calls),
                });
                s
            } else {
                r#"{"ok":true,"session_ended":false,"message":"No active session to end"}"#.into()
            }
        } else {
            r#"{"ok":false}"#.into()
        };
        return make_response(200, "OK", headers, summary_json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/inject" {
        let pattern_opt = json_str_field(&body_text, "pattern").or_else(|| json_str_field(&body_text, "url"));
        let folder_opt = json_str_field(&body_text, "folder");
        if pattern_opt.is_none() && folder_opt.is_none() {
            return make_response(
                400,
                "Bad Request",
                headers,
                br#"{"ok":false,"error":"Missing 'pattern' or 'folder' in inject request"}"#.to_vec(),
            );
        }
        let inject_map = json_inject_map(&body_text);
        let mut updated = 0usize;
        if let Ok(mut cfg) = ctx.config.write() {
            for r in cfg.map_rules.iter_mut() {
                let match_p = pattern_opt.as_ref().map(|p| r.pattern == *p || r.name == *p).unwrap_or(false);
                let match_f = folder_opt.as_ref().map(|f| r.folder_name().eq_ignore_ascii_case(f)).unwrap_or(false);
                if match_p || match_f {
                    for (k, v) in &inject_map {
                        r.inject_props.insert(k.clone(), v.clone());
                    }
                    updated += 1;
                }
            }
            if updated > 0 {
                let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
                let _ = ctx.tx.send(ProxyEvent::Log {
                    level: "Info".into(),
                    message: format!("Control Hook injected {} properties into {} rules", inject_map.len(), updated),
                });
            }
        }
        let json = format!(
            "{{\"ok\":true,\"action\":\"injected\",\"updated\":{},\"inject_keys\":{}}}",
            updated,
            inject_map.len()
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/mock" {
        let Some(pattern) = json_str_field(&body_text, "pattern")
            .or_else(|| json_str_field(&body_text, "url"))
        else {
            return make_response(
                400,
                "Bad Request",
                headers,
                br#"{"ok":false,"error":"Missing 'pattern' or 'url' field in JSON body"}"#.to_vec(),
            );
        };
        let folder = json_str_field(&body_text, "folder")
            .unwrap_or_else(|| {
                extract_domain_from_pattern(&pattern).unwrap_or_else(|| "General".into())
            });
        let inject_map = json_inject_map(&body_text);
        let scenario = json_str_field(&body_text, "scenario")
            .and_then(|s| MockScenario::from_slug(&s))
            .unwrap_or(MockScenario::Success);
        let status = json_u16_field(&body_text, "status").unwrap_or_else(|| scenario.default_status());
        let raw_body = json_str_field(&body_text, "body").unwrap_or_else(|| scenario.default_body());
        let sanitize = !body_text.contains("\"sanitize\":false") && !body_text.contains("\"sanitize\": false");
        let resp_body = if sanitize {
            sanitize_mock_payload(&raw_body)
        } else {
            raw_body
        };
        let method = json_str_field(&body_text, "method")
            .map(|m| match m.to_ascii_uppercase().as_str() {
                "GET" => RuleMethod::Get,
                "POST" => RuleMethod::Post,
                "PUT" => RuleMethod::Put,
                "DELETE" => RuleMethod::Delete,
                _ => RuleMethod::Any,
            })
            .unwrap_or(RuleMethod::Any);
        let name = json_str_field(&body_text, "name")
            .unwrap_or_else(|| format!("UI Test Mock ({})", pattern));
        let app_version = json_str_field(&body_text, "app_version").unwrap_or_default();
        let env = json_str_field(&body_text, "env").unwrap_or_default();

        let (rule_id, fixture_path) = if let Ok(mut cfg) = ctx.config.write() {
            cfg.map_enabled = true;
            if let Some(domain) = extract_domain_from_pattern(&pattern) {
                if !cfg.ssl_allowlist.iter().any(|d| d.eq_ignore_ascii_case(&domain)) {
                    cfg.ssl_allowlist.push(domain);
                }
            }
            let (id, f_path) = if let Some(existing) = cfg.map_rules.iter_mut().find(|r| r.pattern == pattern) {
                existing.switch_scenario(scenario);
                existing.status = status;
                existing.body = resp_body.clone();
                existing.method = method;
                existing.folder = folder;
                existing.enabled = true;
                if !app_version.is_empty() {
                    existing.app_version = app_version.clone();
                }
                if !env.is_empty() {
                    existing.env = env.clone();
                }
                for (k, v) in &inject_map {
                    existing.inject_props.insert(k.clone(), v.clone());
                }
                match scenario {
                    MockScenario::Success => {
                        existing.success_status = status;
                        existing.success_body = resp_body;
                    }
                    MockScenario::Error => {
                        existing.error_status = status;
                        existing.error_body = resp_body;
                    }
                    MockScenario::Empty => {
                        existing.empty_status = status;
                        existing.empty_body = resp_body;
                    }
                }
                (existing.id, existing.fixture_versioned_path())
            } else {
                let next_id = cfg.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                let mut rule = MapRule {
                    id: next_id,
                    name: name.clone(),
                    folder,
                    pattern: pattern.clone(),
                    method,
                    status,
                    body: resp_body.clone(),
                    enabled: true,
                    active_scenario: scenario,
                    inject_props: inject_map,
                    app_version: app_version.clone(),
                    env: env.clone(),
                    ..MapRule::default()
                };
                match scenario {
                    MockScenario::Success => {
                        rule.success_status = status;
                        rule.success_body = resp_body;
                    }
                    MockScenario::Error => {
                        rule.error_status = status;
                        rule.error_body = resp_body;
                    }
                    MockScenario::Empty => {
                        rule.empty_status = status;
                        rule.empty_body = resp_body;
                    }
                }
                let fix_p = rule.fixture_versioned_path();
                cfg.map_rules.insert(0, rule);
                (next_id, fix_p)
            };
            let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
            let _ = ctx.tx.send(ProxyEvent::Log {
                level: "Info".into(),
                message: format!(
                    "Control Hook upserted mock '{}' ({} {})",
                    pattern,
                    scenario.label(),
                    status
                ),
            });
            (id, f_path)
        } else {
            (0, String::new())
        };

        let json = format!(
            "{{\"ok\":true,\"action\":\"mock_upserted\",\"id\":{},\"pattern\":\"{}\",\"scenario\":\"{}\",\"status\":{},\"app_version\":\"{}\",\"env\":\"{}\",\"fixture_path\":\"{}\"}}",
            rule_id,
            escape_json(&pattern),
            scenario.slug(),
            status,
            escape_json(&app_version),
            escape_json(&env),
            escape_json(&fixture_path)
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/scenario" {
        let pattern_opt = json_str_field(&body_text, "pattern").or_else(|| json_str_field(&body_text, "url"));
        let folder_opt = json_str_field(&body_text, "folder");
        if pattern_opt.is_none() && folder_opt.is_none() {
            return make_response(
                400,
                "Bad Request",
                headers,
                br#"{"ok":false,"error":"Missing 'pattern' or 'folder' field"}"#.to_vec(),
            );
        }
        let raw_scenario = json_str_field(&body_text, "scenario").unwrap_or_else(|| "success".into());
        let is_passthrough = matches!(
            raw_scenario.trim().to_ascii_lowercase().as_str(),
            "passthrough" | "real" | "upstream" | "off"
        );
        let mut found = false;
        let mut new_status = 200u16;
        let mut new_enabled = true;
        let mut switched_count = 0usize;

        if let Ok(mut cfg) = ctx.config.write() {
            for rule in cfg.map_rules.iter_mut() {
                let match_p = pattern_opt.as_ref().map(|p| rule.pattern == *p || rule.name == *p).unwrap_or(false);
                let match_f = folder_opt.as_ref().map(|f| rule.folder_name().eq_ignore_ascii_case(f)).unwrap_or(false);
                if match_p || match_f {
                    found = true;
                    switched_count += 1;
                    if is_passthrough {
                        rule.enabled = false;
                        new_enabled = false;
                        new_status = rule.status;
                    } else if let Some(sc) = MockScenario::from_slug(&raw_scenario) {
                        rule.switch_scenario(sc);
                        rule.enabled = true;
                        new_status = rule.status;
                        new_enabled = true;
                    }
                }
            }
            if found && new_enabled {
                cfg.map_enabled = true;
            }
            if found {
                let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
                let _ = ctx.tx.send(ProxyEvent::Log {
                    level: "Info".into(),
                    message: format!(
                        "Control Hook switched {} rule(s) to '{}'",
                        switched_count, raw_scenario
                    ),
                });
            }
        }

        if !found {
            return make_response(
                404,
                "Not Found",
                headers,
                br#"{"ok":false,"error":"No matching rule found"}"#.to_vec(),
            );
        }
        let target_label = folder_opt.unwrap_or_else(|| pattern_opt.unwrap_or_default());
        let json = format!(
            "{{\"ok\":true,\"action\":\"scenario_switched\",\"target\":\"{}\",\"scenario\":\"{}\",\"enabled\":{},\"status\":{},\"count\":{}}}",
            escape_json(&target_label),
            escape_json(&raw_scenario),
            new_enabled,
            new_status,
            switched_count
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/reset" {
        if let Ok(mut cfg) = ctx.config.write() {
            for r in &mut cfg.map_rules {
                r.hits = 0;
            }
            let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(Box::new(cfg.clone())));
            let _ = ctx.tx.send(ProxyEvent::Log {
                level: "Info".into(),
                message: "Control Hook reset all mock hit counters".into(),
            });
        }
        return make_response(
            200,
            "OK",
            headers,
            br#"{"ok":true,"action":"reset"}"#.to_vec(),
        );
    }

    make_response(
        404,
        "Not Found",
        headers,
        br#"{"ok":false,"error":"Unknown /__proxzy endpoint. Use GET /__proxzy/health, POST /__proxzy/mock, POST /__proxzy/scenario, or POST /__proxzy/reset"}"#.to_vec(),
    )
}

async fn wait_for_decision(ctx: &ConnCtx, id: u64) -> Option<BpDecision> {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if let Ok(mut guard) = ctx.decisions.lock() {
            if let Some(d) = guard.remove(&id) {
                return Some(d);
            }
        }
        // Closing the app must not leave a request paused for three minutes.
        if ctx.shutdown.load(Ordering::SeqCst) {
            return None;
        }
        if Instant::now() > deadline {
            return None;
        }
        let _ = tokio::time::timeout(Duration::from_millis(200), ctx.notify.notified()).await;
    }
}

#[allow(clippy::too_many_arguments)]
fn build_capture(
    req: &HttpRequest,
    url: &str,
    host: &str,
    port: u16,
    peer: SocketAddr,
    status: u16,
    latency_ms: u64,
    resp_headers: &[(String, String)],
    resp_body: &str,
    mock: Option<String>,
    aborted: bool,
    tls: bool,
) -> Capture {
    let body_text = String::from_utf8_lossy(&req.body).to_string();
    let kind = detect_kind(&req.method, url, &body_text);
    let graphql = if kind == ApiKind::Graphql {
        graphql_info(&req.method, &body_text)
    } else {
        None
    };
    Capture {
        id: 0,
        method: req.method.clone(),
        url: url.to_string(),
        status,
        latency_ms: latency_ms.max(1),
        size_bytes: resp_body.len() as u64,
        kind,
        started: now_stamp(),
        remote: format!("{}:{} via {}", host, port, peer.ip()),
        protocol: if tls { "HTTPS (MITM)" } else { "HTTP/1.1" }.into(),
        req_headers: req.headers.clone(),
        req_body: body_text,
        resp_headers: resp_headers.to_vec(),
        resp_body: resp_body.to_string(),
        graphql,
        mock,
        pinned: false,
        aborted,
    }
}

fn tls_acceptor_for(host: &str, ca: &Ca) -> Result<TlsAcceptor, String> {
    let key = KeyPair::generate().map_err(|e| format!("Leaf key: {}", e))?;
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("Leaf cert: {}", e))?;
    // IP hosts need an IP SAN; a DNS SAN alone makes Safari and many HTTP
    // clients reject the intercepted certificate.
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress(ip));
    } else {
        params
            .subject_alt_names
            .push(rcgen::SanType::DnsName(
                host.try_into()
                    .map_err(|_| format!("Invalid host name: {}", host))?,
            ));
    }
    params.distinguished_name.push(DnType::CommonName, host);
    params.is_ca = IsCa::NoCa;
    params.use_authority_key_identifier_extension = true;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let leaf = params
        .signed_by(&key, &ca.cert, &ca.key)
        .map_err(|e| format!("Leaf signing: {}", e))?;
    let chain = vec![
        CertificateDer::from(leaf.der().to_vec()),
        CertificateDer::from(pem_to_der(&ca.cert_pem)?),
    ];
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
    let mut cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(chain, key_der)
        .map_err(|e| format!("TLS server config: {}", e))?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

fn client_config() -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(cfg)
}

/// A tiny local API used by the in-app practice session. Five paths that look
/// like a real mobile backend: a GraphQL query, a REST catalogue, an
/// unauthorised order, a checkout and a recommendations feed.
///
/// This answers only `practice.proxzy.gateway` and only after the Breakpoint
/// and Map Local stages, so the person watches the real dispatch order instead
/// of a pre-baked list of rows. The small sleeps keep the timing column honest.
async fn practice_response(req: &HttpRequest, url: &str) -> HttpResponse {
    let path = origin_path(url);
    let path_only = path.split('?').next().unwrap_or(&path);
    let (status, reason_text, delay_ms, body) = match (req.method.to_uppercase().as_str(), path_only) {
        ("POST", "/demo/graphql") => (
            200,
            "OK",
            36,
            r#"{
  "data": {
    "products": [
      { "id": "prod_101", "name": "Runner shoes", "price": 1290.00, "currency": "THB", "inStock": true },
      { "id": "prod_102", "name": "Everyday tote", "price": 690.00, "currency": "THB", "inStock": true },
      { "id": "prod_103", "name": "Trail cap", "price": 450.00, "currency": "THB", "inStock": false }
    ]
  }
}"#,
        ),
        ("GET", "/demo/products") => (
            200,
            "OK",
            24,
            r#"{
  "source": "practice-upstream",
  "collection": "running",
  "limit": 20,
  "products": [
    { "id": "prod_101", "name": "Runner shoes", "price": 1290.00, "currency": "THB" },
    { "id": "prod_102", "name": "Everyday tote", "price": 690.00, "currency": "THB" },
    { "id": "prod_103", "name": "Trail cap", "price": 450.00, "currency": "THB" }
  ]
}"#,
        ),
        ("GET", "/demo/orders/8421") => (
            401,
            "Unauthorized",
            18,
            r#"{
  "error": "unauthorized",
  "message": "This order belongs to another account.",
  "code": "ORDER_401"
}"#,
        ),
        ("POST", "/demo/checkout") => (
            200,
            "OK",
            52,
            r#"{
  "order": "ord_7712",
  "status": "confirmed",
  "totalQuantity": 3,
  "total": 2190.00,
  "currency": "THB"
}"#,
        ),
        ("GET", "/demo/recommendations") => (
            200,
            "OK",
            29,
            r#"{
  "userId": "u_8841",
  "items": [
    { "sku": "TEE-01", "reason": "bought together" },
    { "sku": "MUG-04", "reason": "popular in your area" }
  ]
}"#,
        ),
        _ => (
            200,
            "OK",
            12,
            r#"{"ok":true,"note":"Practice endpoint. Use /demo/graphql, /demo/products, /demo/orders/8421, /demo/checkout or /demo/recommendations."}"#,
        ),
    };
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    let headers = vec![
        (
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        ),
        ("x-proxzy-practice".to_string(), "local-api".to_string()),
        ("cache-control".to_string(), "no-store".to_string()),
    ];
    make_response(status, reason_text, headers, body.as_bytes().to_vec())
}

async fn forward_request(
    req: &HttpRequest,
    scheme: &str,
    host: &str,
    port: u16,
    url: &str,
    peer: SocketAddr,
    binding: &ListenerBinding,
) -> Result<HttpResponse, String> {
    let path = origin_path(url);
    let mut out = Vec::new();
    out.extend_from_slice(
        format!("{} {} HTTP/1.1\r\n", req.method, path).as_bytes(),
    );
    let mut saw_host = false;
    for (k, v) in &req.headers {
        let lower = k.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "proxy-connection"
                | "connection"
                | "keep-alive"
                | "transfer-encoding"
                | "te"
                | "trailer"
                | "upgrade"
                | "content-length"
                | "accept-encoding"
        ) {
            continue;
        }
        if lower == "host" {
            saw_host = true;
        }
        out.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
    }
    if !saw_host {
        out.extend_from_slice(format!("Host: {}\r\n", host_header(host, port, scheme)).as_bytes());
    }
    out.extend_from_slice(b"Accept-Encoding: identity\r\n");
    out.extend_from_slice(b"Connection: close\r\n");
    if !req.body.is_empty() {
        out.extend_from_slice(format!("Content-Length: {}\r\n", req.body.len()).as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&req.body);

    let addr = format!("{}:{}", host, port);
    let upstream_addr = resolve_upstream(host, port, peer, binding).await?;
    let tcp = tokio::time::timeout(Duration::from_secs(20), TcpStream::connect(upstream_addr))
        .await
        .map_err(|_| format!("connect to {} timed out", addr))?
        .map_err(|e| format!("connect to {}: {}", addr, e))?;

    if scheme == "https" {
        let connector = TlsConnector::from(client_config());
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|_| format!("invalid TLS host name: {}", host))?;
        let mut tls = tokio::time::timeout(Duration::from_secs(20), connector.connect(server_name, tcp))
            .await
            .map_err(|_| "TLS connect timed out".to_string())?
            .map_err(|e| format!("TLS connect: {}", e))?;
        tls.write_all(&out).await.map_err(|e| e.to_string())?;
        read_response(&mut tls).await
    } else {
        let mut stream = tcp;
        stream.write_all(&out).await.map_err(|e| e.to_string())?;
        read_response(&mut stream).await
    }
}

// ---------------------------------------------------------------------------
// HTTP parsing
// ---------------------------------------------------------------------------

struct HttpRequest {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: u16,
    reason: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

async fn read_request<R: AsyncRead + Unpin>(r: &mut R) -> Result<Option<HttpRequest>, String> {
    let head = match read_head(r).await.map_err(|e| e.to_string())? {
        Some(h) => h,
        None => return Ok(None),
    };
    read_request_with_head(r, head).await
}

async fn read_request_with_head<R: AsyncRead + Unpin>(
    r: &mut R,
    head: Vec<u8>,
) -> Result<Option<HttpRequest>, String> {
    let (head_bytes, leftover) = split_head(&head);
    let text = String::from_utf8_lossy(head_bytes);
    let mut lines = text.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    if method.is_empty() {
        return Ok(None);
    }
    let target = parts.next().unwrap_or("/").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let body = read_body(r, &headers, leftover.to_vec(), false)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(HttpRequest {
        method,
        target,
        headers,
        body,
    }))
}

async fn read_response<R: AsyncRead + Unpin>(r: &mut R) -> Result<HttpResponse, String> {
    let head = match read_head(r).await.map_err(|e| e.to_string())? {
        Some(h) => h,
        None => return Err("server closed without a response".to_string()),
    };
    let (head_bytes, leftover) = split_head(&head);
    let text = String::from_utf8_lossy(head_bytes);
    let mut lines = text.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.splitn(3, ' ');
    let _version = parts.next().unwrap_or("");
    let status: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let reason = parts.next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let body = if status == 204 || status == 304 {
        Vec::new()
    } else {
        read_body(r, &headers, leftover.to_vec(), true)
            .await
            .map_err(|e| e.to_string())?
    };
    Ok(HttpResponse {
        status,
        reason,
        headers,
        body,
    })
}

async fn read_head<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = tokio::time::timeout(HEAD_TIMEOUT, r.read(&mut tmp))
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "request headers timed out")
            })??;
        if n == 0 {
            return Ok(if buf.is_empty() { None } else { Some(buf) });
        }
        buf.extend_from_slice(&tmp[..n]);
        if find_subslice(&buf, b"\r\n\r\n").is_some() {
            return Ok(Some(buf));
        }
        if buf.len() > MAX_HEAD {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request headers too large",
            ));
        }
    }
}

fn split_head(buf: &[u8]) -> (&[u8], &[u8]) {
    if let Some(pos) = find_subslice(buf, b"\r\n\r\n") {
        (&buf[..pos + 2], &buf[pos + 4..])
    } else {
        (buf, &[])
    }
}

fn first_line(head: &[u8]) -> &str {
    let text = std::str::from_utf8(head).unwrap_or("");
    text.split("\r\n").next().unwrap_or("")
}

async fn read_body<R: AsyncRead + Unpin>(
    r: &mut R,
    headers: &[(String, String)],
    mut buf: Vec<u8>,
    response: bool,
) -> std::io::Result<Vec<u8>> {
    let chunked = header_value(headers, "transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    if chunked {
        let mut tmp = [0u8; 8192];
        loop {
            if let Some(decoded) = try_decode_chunked(&buf) {
                return Ok(decoded);
            }
            if buf.len() > MAX_BODY {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "body too large",
                ));
            }
            let n = r.read(&mut tmp).await?;
            if n == 0 {
                return Ok(decode_chunked_lossy(&buf));
            }
            buf.extend_from_slice(&tmp[..n]);
        }
    }
    if let Some(len) = header_value(headers, "content-length")
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        let len = len.min(MAX_BODY);
        while buf.len() < len {
            let mut tmp = vec![0u8; (len - buf.len()).min(8192)];
            let n = r.read(&mut tmp).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
        buf.truncate(len);
        return Ok(buf);
    }
    if response {
        // Connection: close was requested upstream, so EOF marks the body end.
        let mut tmp = [0u8; 8192];
        loop {
            if buf.len() > MAX_BODY {
                buf.truncate(MAX_BODY);
                return Ok(buf);
            }
            let n = r.read(&mut tmp).await?;
            if n == 0 {
                return Ok(buf);
            }
            buf.extend_from_slice(&tmp[..n]);
        }
    }
    Ok(buf)
}

fn try_decode_chunked(buf: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let line_end = find_subslice(&buf[pos..], b"\r\n")? + pos;
        let size_text = std::str::from_utf8(&buf[pos..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        pos = line_end + 2;
        if size == 0 {
            if find_subslice(&buf[pos..], b"\r\n").is_some() {
                return Some(out);
            }
            return None;
        }
        if buf.len() < pos + size + 2 {
            return None;
        }
        out.extend_from_slice(&buf[pos..pos + size]);
        pos += size + 2;
    }
}

fn decode_chunked_lossy(buf: &[u8]) -> Vec<u8> {
    try_decode_chunked(buf).unwrap_or_default()
}

async fn write_response<S: AsyncWrite + Unpin>(
    client: &mut S,
    resp: &HttpResponse,
    head_only: bool,
) -> Result<(), String> {
    let mut out = Vec::new();
    out.extend_from_slice(
        format!(
            "HTTP/1.1 {} {}\r\n",
            resp.status,
            if resp.reason.is_empty() {
                reason(resp.status).to_string()
            } else {
                resp.reason.clone()
            }
        )
        .as_bytes(),
    );
    for (k, v) in &resp.headers {
        let lower = k.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "connection" | "keep-alive" | "transfer-encoding" | "content-length"
        ) {
            continue;
        }
        out.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
    }
    out.extend_from_slice(format!("Content-Length: {}\r\n", resp.body.len()).as_bytes());
    out.extend_from_slice(b"Connection: close\r\n\r\n");
    if !head_only {
        out.extend_from_slice(&resp.body);
    }
    client
        .write_all(&out)
        .await
        .map_err(|e| e.to_string())?;
    let _ = client.flush().await;
    Ok(())
}

fn make_response(
    status: u16,
    reason: &str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
) -> HttpResponse {
    HttpResponse {
        status,
        reason: reason.to_string(),
        headers,
        body,
    }
}

fn decode_body(body: &[u8], encoding: Option<String>) -> String {
    let enc = encoding.unwrap_or_default().to_ascii_lowercase();
    if enc.contains("gzip") {
        let mut out = String::new();
        let mut decoder = GzDecoder::new(body);
        if decoder.read_to_string(&mut out).is_ok() {
            return out;
        }
    }
    String::from_utf8_lossy(body).to_string()
}

fn header_value(headers: &[(String, String)], key: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
}

fn parse_header_text(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (k, v) = line.split_once(':')?;
            if k.trim().is_empty() {
                None
            } else {
                Some((k.trim().to_string(), v.trim().to_string()))
            }
        })
        .collect()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn split_host_port(target: &str, default_port: u16) -> (String, u16) {
    if let Some(rest) = target.strip_prefix('[') {
        if let Some((host, tail)) = rest.split_once(']') {
            let port = tail
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .unwrap_or(default_port);
            return (host.to_string(), port);
        }
    }
    match target.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => {
            (host.to_string(), port.parse().unwrap())
        }
        _ => (target.to_string(), default_port),
    }
}

fn build_url(scheme: &str, host: &str, port: u16, target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    let default = if scheme == "https" { 443 } else { 80 };
    let authority = if port == default {
        host.to_string()
    } else {
        format!("{}:{}", host, port)
    };
    let path = if target.starts_with('/') {
        target.to_string()
    } else {
        format!("/{}", target)
    };
    format!("{}://{}{}", scheme, authority, path)
}

fn absolute_target(
    target: &str,
    head: &[u8],
) -> Result<(String, String, u16, String), String> {
    if target.starts_with("http://") || target.starts_with("https://") {
        parse_absolute(target).ok_or_else(|| format!("Bad URL: {}", target))
    } else {
        let text = String::from_utf8_lossy(head);
        let host_header = text
            .split("\r\n")
            .find_map(|line| {
                let (k, v) = line.split_once(':')?;
                if k.eq_ignore_ascii_case("host") {
                    Some(v.trim().to_string())
                } else {
                    None
                }
            })
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "Bad request: missing Host header".to_string())?;
        let (host, port) = split_host_port(&host_header, 80);
        let url = build_url("http", &host, port, target);
        Ok(("http".to_string(), host, port, url))
    }
}

fn parse_absolute(url: &str) -> Option<(String, String, u16, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let path = if authority_end < rest.len() {
        let p = &rest[authority_end..];
        if p.starts_with('/') {
            p.to_string()
        } else {
            format!("/{}", p)
        }
    } else {
        "/".to_string()
    };
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let default = if scheme == "https" { 443 } else { 80 };
    let (host, port) = split_host_port(authority, default);
    Some((
        scheme.clone(),
        host,
        port,
        format!("{}://{}{}", scheme, authority, path),
    ))
}

fn origin_path(url: &str) -> String {
    if let Some(rest) = url.split_once("://").map(|(_, r)| r) {
        if let Some(pos) = rest.find('/') {
            return rest[pos..].to_string();
        }
        if let Some(pos) = rest.find('?') {
            return format!("/{}", &rest[pos..]);
        }
        return "/".to_string();
    }
    url.to_string()
}

/// Read a plain query value from an absolute URL. The capture check only puts
/// digits in `nonce`, so no percent-decoding is needed here.
fn query_value(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next().unwrap_or("");
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

fn host_header(host: &str, port: u16, scheme: &str) -> String {
    let default = if scheme == "https" { 443 } else { 80 };
    if port == default {
        host.to_string()
    } else {
        format!("{}:{}", host, port)
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

fn escape_json(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")
}

#[cfg(test)]
mod proxy_tests {
    use super::*;

    #[test]
    fn default_config_is_safe_passthrough() {
        let cfg = ProxyConfig::default();
        assert!(cfg.ssl_proxying_enabled);
        assert!(cfg.ssl_allowlist.is_empty());
        // Safe Default Invariant: Empty allowlist must NEVER intercept!
        assert!(!cfg.should_intercept("google.com"));
        assert!(!cfg.should_intercept("apple.com"));
        assert!(!cfg.should_intercept("api.github.com"));
    }

    #[test]
    fn selective_interception_only_matches_allowlist() {
        let mut cfg = ProxyConfig::default();
        cfg.ssl_allowlist = vec!["*.myapi.com".into(), "api.dev.local:8443".into()];

        // Whitelisted targets
        assert!(cfg.should_intercept("api.myapi.com"));
        assert!(cfg.should_intercept("v1.users.myapi.com"));
        assert!(cfg.should_intercept("myapi.com"));
        assert!(cfg.should_intercept("api.dev.local:8443"));

        // Unlisted targets MUST pass through (no TLS MITM)
        assert!(!cfg.should_intercept("google.com"));
        assert!(!cfg.should_intercept("safebrowsing.googleapis.com"));
        assert!(!cfg.should_intercept("notmyapi.com"));
    }

    #[test]
    fn disabled_ssl_proxying_passes_everything() {
        let mut cfg = ProxyConfig::default();
        cfg.ssl_proxying_enabled = false;
        cfg.intercept_all = true;
        cfg.ssl_allowlist = vec!["*".into()];
        assert!(!cfg.should_intercept("anything.com"));
    }

    #[test]
    fn full_catch_switch_decrypts_every_host_when_on() {
        let mut cfg = ProxyConfig::default();
        cfg.intercept_all = true;
        assert!(cfg.should_intercept("api.example.com"));
        assert!(cfg.should_intercept("127.0.0.1:3000"));
        cfg.intercept_all = false;
        assert!(!cfg.should_intercept("api.example.com"));
    }

    #[test]
    fn star_in_allowlist_means_full_catch() {
        let mut cfg = ProxyConfig::default();
        cfg.ssl_allowlist = vec!["*".into()];
        assert!(cfg.should_intercept("api.example.com"));
        assert!(cfg.should_intercept("auth.example.com:8443"));
    }

    #[test]
    fn capture_check_nonce_is_read_from_the_query() {
        assert_eq!(
            query_value("http://proxzy.gateway/__proxzy/check?nonce=123&x=1", "nonce").as_deref(),
            Some("123")
        );
        assert_eq!(
            query_value("http://proxzy.gateway/__proxzy/check", "nonce"),
            None
        );
    }

    #[test]
    fn wildcard_and_port_interception() {
        let mut cfg = ProxyConfig::default();
        cfg.ssl_allowlist = vec!["*.shopwave.dev".into(), "127.0.0.1:3000".into()];
        assert!(cfg.should_intercept("api.shopwave.dev:443"));
        assert!(cfg.should_intercept("auth.shopwave.dev"));
        assert!(cfg.should_intercept("127.0.0.1:3000"));
        assert!(!cfg.should_intercept("127.0.0.1:8080"));
        assert!(!cfg.should_intercept("google.com:443"));
    }

    #[test]
    fn control_hook_json_extractors_work() {
        let payload = r#"{"pattern":"https://api.shopwave.dev/v1/orders*","scenario":"error","status":503,"body":"{\"error\":\"down\"}"}"#;
        assert_eq!(
            json_str_field(payload, "pattern").as_deref(),
            Some("https://api.shopwave.dev/v1/orders*")
        );
        assert_eq!(json_str_field(payload, "scenario").as_deref(), Some("error"));
        assert_eq!(json_u16_field(payload, "status"), Some(503));
        assert_eq!(
            json_str_field(payload, "body").as_deref(),
            Some("{\"error\":\"down\"}")
        );
    }

    #[test]
    fn control_hook_endpoints_upsert_switch_and_reset_mocks() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let key = KeyPair::generate().unwrap();
        let params = CertificateParams::new(Vec::<String>::new()).unwrap();
        let cert = params.self_signed(&key).unwrap();
        let ca = Arc::new(Ca {
            cert_pem: cert.pem(),
            key,
            cert,
            info: CaInfo {
                dir: PathBuf::new(),
                pem_path: PathBuf::new(),
                cer_path: PathBuf::new(),
                fingerprint: String::new(),
            },
        });
        let ctx = ConnCtx {
            config: Arc::new(RwLock::new(ProxyConfig::default())),
            decisions: Arc::new(Mutex::new(HashMap::new())),
            paused: Arc::new(Mutex::new(Vec::new())),
            notify: Arc::new(Notify::new()),
            tx,
            ca,
            shutdown: Arc::new(AtomicBool::new(false)),
            binding: Arc::new(ListenerBinding {
                bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 9090,
                policy: BindPolicy::LoopbackOnly,
            }),
        };

        // 1) Upsert mock via POST /__proxzy/mock
        let mock_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/mock".into(),
            headers: Vec::new(),
            body: br#"{"folder":"Orders","pattern":"https://api.shopwave.dev/v1/orders*","method":"GET","scenario":"success","status":200,"body":"{\"orders\":[1]}"}"#.to_vec(),
        };
        let resp = handle_proxzy_control(&mock_req, "/__proxzy/mock", &ctx);
        assert_eq!(resp.status, 200);
        assert!(String::from_utf8_lossy(&resp.body).contains("\"mock_upserted\""));

        // Verify domain was auto-added to ssl_allowlist and map_enabled is true
        {
            let cfg = ctx.config.read().unwrap();
            assert!(cfg.map_enabled);
            assert!(cfg.ssl_allowlist.contains(&"api.shopwave.dev".to_string()));
            assert_eq!(cfg.map_rules.len(), 1);
            assert_eq!(cfg.map_rules[0].status, 200);
        }

        // 2) Switch scenario to error via POST /__proxzy/scenario
        let sc_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/scenario".into(),
            headers: Vec::new(),
            body: br#"{"pattern":"https://api.shopwave.dev/v1/orders*","scenario":"error"}"#.to_vec(),
        };
        let sc_resp = handle_proxzy_control(&sc_req, "/__proxzy/scenario", &ctx);
        assert_eq!(sc_resp.status, 200);
        {
            let cfg = ctx.config.read().unwrap();
            assert_eq!(cfg.map_rules[0].active_scenario, MockScenario::Error);
            assert_eq!(cfg.map_rules[0].status, 500);
        }

        // 3) Check GET /__proxzy/health
        let health_req = HttpRequest {
            method: "GET".into(),
            target: "/__proxzy/health".into(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let health_resp = handle_proxzy_control(&health_req, "/__proxzy/health", &ctx);
        assert_eq!(health_resp.status, 200);
        let health_body = String::from_utf8_lossy(&health_resp.body);
        assert!(health_body.contains("\"engine\":\"ProxzyCraft\""));
        assert!(health_body.contains("\"scenario\":\"error\""));

        // 4) Test dynamic property injection via POST /__proxzy/inject
        let inject_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/inject".into(),
            headers: vec![("host".into(), "127.0.0.1:9090".into())],
            body: br#"{"pattern":"https://api.shopwave.dev/v1/orders*","inject":{"username":"0812345678","otp":"111111"}}"#.to_vec(),
        };
        let inject_resp = handle_proxzy_control(&inject_req, "/__proxzy/inject", &ctx);
        assert_eq!(inject_resp.status, 200);
        {
            let cfg = ctx.config.read().unwrap();
            assert_eq!(cfg.map_rules[0].inject_props.get("username").map(|s| s.as_str()), Some("0812345678"));
            assert_eq!(cfg.map_rules[0].inject_props.get("otp").map(|s| s.as_str()), Some("111111"));
        }

        // 5) Test batch folder scenario switching via POST /__proxzy/scenario with "folder"
        let folder_sc_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/scenario".into(),
            headers: vec![("host".into(), "127.0.0.1:9090".into())],
            body: br#"{"folder":"Orders","scenario":"success"}"#.to_vec(),
        };
        let folder_resp = handle_proxzy_control(&folder_sc_req, "/__proxzy/scenario", &ctx);
        assert_eq!(folder_resp.status, 200);
        {
            let cfg = ctx.config.read().unwrap();
            assert_eq!(cfg.map_rules[0].active_scenario, MockScenario::Success);
            assert_eq!(cfg.map_rules[0].status, 200);
        }

        // 6) Test Security Guard: a spoofed Host is rejected before the handler
        let evil_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/mock".into(),
            headers: vec![("host".into(), "evil.com".into())],
            body: br#"{"pattern":"https://api.shopwave.dev/v1/orders*","scenario":"error"}"#.to_vec(),
        };
        assert!(authorize_control(
            SocketAddr::from(([127, 0, 0, 1], 50000)),
            &evil_req.headers,
            &ctx.binding
        )
        .is_err());

        // 7) Test Auto-Catch Record Config via POST /__proxzy/record
        let rec_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/record".into(),
            headers: vec![("host".into(), "127.0.0.1:9090".into())],
            body: br#"{"enabled":true,"domains":["*.sample.com","api.trusted.com"]}"#.to_vec(),
        };
        let rec_resp = handle_proxzy_control(&rec_req, "/__proxzy/record", &ctx);
        assert_eq!(rec_resp.status, 200);
        {
            let cfg = ctx.config.read().unwrap();
            assert!(cfg.auto_record);
            assert!(cfg.auto_record_domains.contains(&"*.sample.com".to_string()));
            assert!(cfg.ssl_allowlist.contains(&"api.trusted.com".to_string()));
        }

        // 8) Test QA Test Session Audit & Summary Endpoints with Version Stamping
        let sess_start_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/session/start".into(),
            headers: vec![("host".into(), "127.0.0.1:9090".into())],
            body: br#"{"name":"TC_CHECKOUT_01","folder":"Checkout","app_version":"1.1.0","baseline_version":"1.0.0","env":"preprod","domains":["*.sample.com"],"paths":["/v1/checkout/*"]}"#.to_vec(),
        };
        let sess_start_resp = handle_proxzy_control(&sess_start_req, "/__proxzy/session/start", &ctx);
        assert_eq!(sess_start_resp.status, 200);
        {
            let cfg = ctx.config.read().unwrap();
            let sess = cfg.active_session.as_ref().unwrap();
            assert_eq!(sess.name, "TC_CHECKOUT_01");
            assert_eq!(sess.folder, "Checkout");
            assert_eq!(sess.app_version, "1.1.0");
            assert_eq!(sess.baseline_version, "1.0.0");
            assert_eq!(sess.env, "preprod");
            assert!(sess.matches_scope("api.sample.com", "/v1/checkout/pay"));
            assert!(!sess.matches_scope("api.sample.com", "/v1/auth/login"));
        }

        let sess_summary_req = HttpRequest {
            method: "GET".into(),
            target: "/__proxzy/session/summary".into(),
            headers: vec![("host".into(), "127.0.0.1:9090".into())],
            body: Vec::new(),
        };
        let sess_summary_resp = handle_proxzy_control(&sess_summary_req, "/__proxzy/session/summary", &ctx);
        assert_eq!(sess_summary_resp.status, 200);
        let summary_text = String::from_utf8_lossy(&sess_summary_resp.body);
        assert!(summary_text.contains("\"TC_CHECKOUT_01\""));
        assert!(summary_text.contains("\"folder\":\"Checkout\""));
        assert!(summary_text.contains("\"app_version\":\"1.1.0\""));
        assert!(summary_text.contains("\"baseline_version\":\"1.0.0\""));
        assert!(summary_text.contains("\"env\":\"preprod\""));
        assert!(summary_text.contains("\"breaking_drifts\":0"));
        assert!(summary_text.contains("\"additive_drifts\":0"));

        let sess_end_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/session/end".into(),
            headers: vec![("host".into(), "127.0.0.1:9090".into())],
            body: Vec::new(),
        };
        let sess_end_resp = handle_proxzy_control(&sess_end_req, "/__proxzy/session/end", &ctx);
        assert_eq!(sess_end_resp.status, 200);
        let end_text = String::from_utf8_lossy(&sess_end_resp.body);
        assert!(end_text.contains("\"app_version\":\"1.1.0\""));
        {
            let cfg = ctx.config.read().unwrap();
            assert!(cfg.active_session.is_none());
        }
    }

    fn lan_binding(token: &str) -> ListenerBinding {
        ListenerBinding {
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: 9090,
            policy: BindPolicy::Lan {
                advertised_ip: "192.168.1.20".into(),
                control_token: token.into(),
            },
        }
    }

    #[test]
    fn control_plane_requires_local_host_and_no_origin() {
        let binding = lan_binding("proxzy_secret");
        let peer = SocketAddr::from(([192, 168, 1, 50], 50000));
        let good = vec![("host".into(), "192.168.1.20:9090".into())];
        assert!(authorize_control(peer, &good, &binding).is_err());

        let with_token = vec![
            ("host".into(), "192.168.1.20:9090".into()),
            ("x-proxzy-control".into(), "proxzy_secret".into()),
        ];
        assert!(authorize_control(peer, &with_token, &binding).is_ok());

        let with_origin = vec![
            ("host".into(), "192.168.1.20:9090".into()),
            ("origin".into(), "https://evil.example".into()),
            ("x-proxzy-control".into(), "proxzy_secret".into()),
        ];
        assert!(authorize_control(peer, &with_origin, &binding).is_err());

        let spoofed = vec![("host".into(), "localhost.attacker.com".into())];
        assert!(authorize_control(peer, &spoofed, &binding).is_err());

        let loopback = SocketAddr::from(([127, 0, 0, 1], 50000));
        assert!(authorize_control(loopback, &good, &binding).is_ok());
    }

    #[test]
    fn control_plane_never_answers_inside_a_tls_tunnel() {
        let binding = ListenerBinding {
            bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 9090,
            policy: BindPolicy::LoopbackOnly,
        };
        assert_eq!(
            classify_service_request(
                "/__proxzy/health",
                "example.com",
                443,
                true,
                &binding
            ),
            ServiceRoute::ProxyForward
        );
        assert_eq!(
            classify_service_request(
                "/__proxzy/health",
                "127.0.0.1",
                9090,
                false,
                &binding
            ),
            ServiceRoute::ControlPlane
        );
        assert_eq!(
            classify_service_request(
                "/ca.cer",
                "127.0.0.1",
                9090,
                false,
                &binding
            ),
            ServiceRoute::Portal
        );
        assert_eq!(
            classify_service_request(
                "http://example.com/",
                "example.com",
                80,
                false,
                &binding
            ),
            ServiceRoute::ProxyForward
        );
    }

    #[tokio::test]
    async fn ssrf_guard_blocks_loopback_bounce_from_lan_and_self_listener() {
        let binding = lan_binding("proxzy_secret");
        let lan_peer = SocketAddr::from(([192, 168, 1, 50], 50000));
        let local_peer = SocketAddr::from(([127, 0, 0, 1], 50000));
        assert!(resolve_upstream("127.0.0.1", 3000, lan_peer, &binding)
            .await
            .is_err());
        assert!(resolve_upstream("127.0.0.1", 3000, local_peer, &binding)
            .await
            .is_ok());
        assert!(resolve_upstream("192.168.1.20", 9090, lan_peer, &binding)
            .await
            .is_err());
        assert!(resolve_upstream("169.254.169.254", 80, local_peer, &binding)
            .await
            .is_err());
    }

    #[test]
    fn absolute_target_rejects_missing_host_header() {
        assert!(absolute_target("/__proxzy/health", b"GET /__proxzy/health HTTP/1.1\r\n\r\n").is_err());
        assert!(absolute_target("/ca.cer", b"GET /ca.cer HTTP/1.1\r\nHost:   \r\n\r\n").is_err());
        let ok = absolute_target("/ca.cer", b"GET /ca.cer HTTP/1.1\r\nHost: 127.0.0.1:9090\r\n\r\n").unwrap();
        assert_eq!(ok.1, "127.0.0.1");
        assert_eq!(ok.2, 9090);
    }

    #[tokio::test]
    async fn live_portal_and_control_security_over_tcp_socket() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let temp_dir = std::env::temp_dir().join(format!(
            "proxzy-live-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let pem_path = temp_dir.join("proxzy-ca.pem");
        let cer_path = temp_dir.join("proxzy-ca.cer");

        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "Proxzy Local CA Test");
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let cert = params.self_signed(&key).unwrap();
        let cert_pem = cert.pem();
        let der = cert.der().to_vec();
        std::fs::write(&pem_path, &cert_pem).unwrap();
        std::fs::write(&cer_path, &der).unwrap();

        let ca = Arc::new(Ca {
            key,
            cert,
            cert_pem,
            info: CaInfo {
                dir: temp_dir.clone(),
                pem_path,
                cer_path,
                fingerprint: "TEST:CA:FP".into(),
            },
        });
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let binding = Arc::new(ListenerBinding {
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: addr.port(),
            policy: BindPolicy::Lan {
                advertised_ip: "127.0.0.1".into(),
                control_token: "proxzy_live_test_token".into(),
            },
        });
        let (tx, _rx) = std::sync::mpsc::channel();
        let ctx = ConnCtx {
            config: Arc::new(RwLock::new(ProxyConfig::default())),
            decisions: Arc::new(Mutex::new(HashMap::new())),
            paused: Arc::new(Mutex::new(Vec::new())),
            notify: Arc::new(Notify::new()),
            tx,
            ca,
            shutdown: Arc::new(AtomicBool::new(false)),
            binding,
        };

        let send = |raw: String| async move {
            let mut client = TcpStream::connect(addr).await.unwrap();
            client.write_all(raw.as_bytes()).await.unwrap();
            let mut buf = Vec::new();
            client.read_to_end(&mut buf).await.unwrap();
            buf
        };

        let srv = tokio::spawn(async move {
            for _ in 0..4 {
                let (sock, peer) = listener.accept().await.unwrap();
                let c = ctx.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(sock, peer, c).await;
                });
            }
        });

        // 1. Portal /ca.cer returns DER certificate with no-store & nosniff
        let cer_bytes = send(format!(
            "GET /ca.cer HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            addr.port()
        ))
        .await;
        let cer_str = String::from_utf8_lossy(&cer_bytes);
        assert!(cer_str.starts_with("HTTP/1.1 200 OK"));
        assert!(cer_str.contains("content-type: application/x-x509-ca-cert"));
        assert!(cer_str.contains("cache-control: no-store"));
        assert!(cer_str.contains("x-content-type-options: nosniff"));

        // 2. Proxy-form request to http://proxzy.local/ serves read-only HTML portal
        let portal_bytes = send(
            "GET http://proxzy.local/ HTTP/1.1\r\nHost: proxzy.local\r\nConnection: close\r\n\r\n"
                .to_string(),
        )
        .await;
        let portal_str = String::from_utf8_lossy(&portal_bytes);
        assert!(portal_str.starts_with("HTTP/1.1 200 OK"));
        assert!(portal_str.contains("/ca.cer"));

        // 3. Browser CORS Origin header on /__proxzy/health is denied with 403
        let cors_bytes = send(format!(
            "GET /__proxzy/health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: https://evil.example\r\nConnection: close\r\n\r\n",
            addr.port()
        ))
        .await;
        let cors_str = String::from_utf8_lossy(&cors_bytes);
        assert!(cors_str.starts_with("HTTP/1.1 403 Forbidden"));

        // 4. Loopback GET /__proxzy/health without Origin succeeds
        let health_bytes = send(format!(
            "GET /__proxzy/health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            addr.port()
        ))
        .await;
        let health_str = String::from_utf8_lossy(&health_bytes);
        assert!(health_str.starts_with("HTTP/1.1 200 OK"));
        assert!(health_str.contains("\"engine\":\"ProxzyCraft\""));

        srv.await.unwrap();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
