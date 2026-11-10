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
    detect_kind, extract_domain_from_pattern, graphql_info, now_stamp, ApiKind, BreakpointRule,
    Capture, MapRule, MockScenario, RuleMethod,
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
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio_rustls::{TlsAcceptor, TlsConnector};

const MAX_HEAD: usize = 128 * 1024;
const MAX_BODY: usize = 12 * 1024 * 1024;

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
    Breakpoint(LiveBreakpoint),
    RemoteConfigUpdated(ProxyConfig),
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
    pub ssl_allowlist: Vec<String>,
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
            ssl_allowlist: Vec::new(),
        }
    }
}

impl ProxyConfig {
    pub fn should_intercept(&self, host: &str) -> bool {
        if !self.ssl_proxying_enabled {
            return false;
        }
        if self.ssl_allowlist.is_empty() {
            return false; // Safe default: blind tunnel everything unless explicitly whitelisted
        }
        self.ssl_allowlist
            .iter()
            .any(|pat| crate::model::domain_matches(pat, host))
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
    shutdown: Arc<AtomicBool>,
    decisions: Arc<Mutex<HashMap<u64, BpDecision>>>,
    paused: Arc<Mutex<Vec<LiveBreakpoint>>>,
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

    pub fn pending(&self) -> Vec<LiveBreakpoint> {
        self.paused
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default()
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
        std::fs::write(&key_path, key.serialize_pem())
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
    let ca = load_ca()?;
    let ca_info = ca.info.clone();
    let ca_info_thread = ca_info.clone();
    let config = Arc::new(RwLock::new(ProxyConfig::default()));
    let shutdown = Arc::new(AtomicBool::new(false));
    let decisions: Arc<Mutex<HashMap<u64, BpDecision>>> = Arc::new(Mutex::new(HashMap::new()));
    let paused: Arc<Mutex<Vec<LiveBreakpoint>>> = Arc::new(Mutex::new(Vec::new()));
    let notify = Arc::new(Notify::new());
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u16, String>>();

    let cfg_thread = config.clone();
    let shutdown_thread = shutdown.clone();
    let decisions_thread = decisions.clone();
    let paused_thread = paused.clone();
    let notify_thread = notify.clone();
    let tx_thread = tx.clone();
    let ca_thread = Arc::new(ca);

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
                let listener = match TcpListener::bind(("127.0.0.1", port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!(
                            "Port {} is already in use: {}",
                            port, e
                        )));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(port));
                let _ = tx_thread.send(ProxyEvent::Started {
                    port,
                    ca: ca_info_thread.clone(),
                });
                let _ = tx_thread.send(ProxyEvent::Log {
                    level: "Info".into(),
                    message: format!("Proxy listening on 127.0.0.1:{} (HTTP + HTTPS)", port),
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
            shutdown,
            decisions,
            paused,
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
                let shutdown_task = shutdown.clone();
                let ctx = ConnCtx {
                    config: config.clone(),
                    decisions: decisions.clone(),
                    paused: paused.clone(),
                    notify: notify.clone(),
                    tx: tx.clone(),
                    ca: ca.clone(),
                    shutdown: shutdown_task,
                };
                let task_tx = tx.clone();
                tokio::spawn(async move {
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
            Ok(Err(_)) => continue,
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
        let intercept = ctx
            .config
            .read()
            .map(|c| c.should_intercept(&host))
            .unwrap_or(false);

        if !intercept {
            return passthrough_tunnel(stream, &host, port, ctx).await;
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
    ctx: ConnCtx,
) -> Result<(), String> {
    let addr = format!("{}:{}", host, port);
    let mut upstream = tokio::time::timeout(Duration::from_secs(15), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("Connect to {} timed out", addr))?
        .map_err(|e| format!("Connect to {}: {}", addr, e))?;

    client
        .write_all(b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: Proxzy\r\n\r\n")
        .await
        .map_err(|e| e.to_string())?;

    let _ = ctx.tx.send(ProxyEvent::Log {
        level: "Info".into(),
        message: format!("Passthrough tunnel: {}:{} (TLS bypassed)", host, port),
    });

    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
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
    let control_path = origin_path(&url);
    if control_path.starts_with("/__proxzy")
        || request.target.starts_with("/__proxzy")
        || host.eq_ignore_ascii_case("proxzy.local")
    {
        let resp = handle_proxzy_control(&request, &control_path, &ctx);
        write_response(client, &resp, request.method == "HEAD").await?;
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
        let _ = ctx.tx.send(ProxyEvent::Breakpoint(pending.clone()));
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
            let resp = make_response(rule.status, reason(rule.status), headers, rule.body.as_bytes().to_vec());
            write_response(client, &resp, request.method == "HEAD").await?;
            mock_name = Some(rule.name.clone());
            mock_response = Some(resp);
            let _ = ctx.tx.send(ProxyEvent::Log {
                level: "Warn".into(),
                message: format!("Map Local answered {} with \"{}\"", url, rule.name),
            });
        }
    }

    if !aborted && mock_name.is_none() {
        // 3) Forward to the real server, then answer the client.
        let resp = match forward_request(&request, &scheme, &host, port, &url).await {
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
    let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(cfg.clone()));
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

/// Handle Testful / WireMock-like HTTP control requests sent to `/__proxzy/*`.
/// Allows MobileWright, `mobile-mcp`, XCTest, Espresso, or `curl` to inspect proxy state,
/// dynamically inject or switch mock scenarios (`success` / `error` / `empty` / `passthrough`),
/// auto-whitelist HTTPS domains, and verify hit counts on the same port (`127.0.0.1:9090`).
fn handle_proxzy_control(req: &HttpRequest, control_path: &str, ctx: &ConnCtx) -> HttpResponse {
    let path = control_path.split('?').next().unwrap_or(control_path);
    let body_text = String::from_utf8_lossy(&req.body).to_string();
    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("access-control-allow-origin".to_string(), "*".to_string()),
    ];

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
                    "{{\"id\":{},\"name\":\"{}\",\"pattern\":\"{}\",\"method\":\"{}\",\"scenario\":\"{}\",\"status\":{},\"enabled\":{},\"hits\":{},\"fixture\":\"{}\"}}",
                    r.id,
                    escape_json(&r.name),
                    escape_json(&r.pattern),
                    r.method.label(),
                    r.active_scenario.slug(),
                    r.status,
                    r.enabled,
                    r.hits,
                    escape_json(&r.fixture_path())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            "{{\"ok\":true,\"engine\":\"ProxzyCraft\",\"recording\":{},\"map_enabled\":{},\"ssl_proxying_enabled\":{},\"ssl_allowlist\":[{}],\"rules_count\":{},\"rules\":[{}]}}",
            cfg.recording,
            cfg.map_enabled,
            cfg.ssl_proxying_enabled,
            allowlist_json,
            cfg.map_rules.len(),
            rules_json
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
        let scenario = json_str_field(&body_text, "scenario")
            .and_then(|s| MockScenario::from_slug(&s))
            .unwrap_or(MockScenario::Success);
        let status = json_u16_field(&body_text, "status").unwrap_or_else(|| scenario.default_status());
        let resp_body = json_str_field(&body_text, "body").unwrap_or_else(|| scenario.default_body());
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

        if let Ok(mut cfg) = ctx.config.write() {
            cfg.map_enabled = true;
            if let Some(domain) = extract_domain_from_pattern(&pattern) {
                if !cfg.ssl_allowlist.iter().any(|d| d.eq_ignore_ascii_case(&domain)) {
                    cfg.ssl_allowlist.push(domain);
                }
            }
            if let Some(existing) = cfg.map_rules.iter_mut().find(|r| r.pattern == pattern) {
                existing.switch_scenario(scenario);
                existing.status = status;
                existing.body = resp_body.clone();
                existing.method = method;
                existing.enabled = true;
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
            } else {
                let next_id = cfg.map_rules.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                let mut rule = MapRule {
                    id: next_id,
                    name: name.clone(),
                    pattern: pattern.clone(),
                    method,
                    status,
                    body: resp_body.clone(),
                    enabled: true,
                    active_scenario: scenario,
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
                cfg.map_rules.insert(0, rule);
            }
            let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(cfg.clone()));
            let _ = ctx.tx.send(ProxyEvent::Log {
                level: "Info".into(),
                message: format!(
                    "Control Hook upserted mock '{}' ({} {})",
                    pattern,
                    scenario.label(),
                    status
                ),
            });
        }

        let json = format!(
            "{{\"ok\":true,\"action\":\"mock_upserted\",\"pattern\":\"{}\",\"scenario\":\"{}\",\"status\":{}}}",
            escape_json(&pattern),
            scenario.slug(),
            status
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/scenario" {
        let Some(pattern) = json_str_field(&body_text, "pattern")
            .or_else(|| json_str_field(&body_text, "url"))
        else {
            return make_response(
                400,
                "Bad Request",
                headers,
                br#"{"ok":false,"error":"Missing 'pattern' field"}"#.to_vec(),
            );
        };
        let raw_scenario = json_str_field(&body_text, "scenario").unwrap_or_else(|| "success".into());
        let is_passthrough = matches!(
            raw_scenario.trim().to_ascii_lowercase().as_str(),
            "passthrough" | "real" | "upstream" | "off"
        );
        let mut found = false;
        let mut new_status = 200u16;
        let mut new_enabled = true;

        if let Ok(mut cfg) = ctx.config.write() {
            if let Some(rule) = cfg
                .map_rules
                .iter_mut()
                .find(|r| r.pattern == pattern || r.name == pattern)
            {
                found = true;
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
            if found && new_enabled {
                cfg.map_enabled = true;
            }
            if found {
                let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(cfg.clone()));
                let _ = ctx.tx.send(ProxyEvent::Log {
                    level: "Info".into(),
                    message: format!(
                        "Control Hook switched '{}' to '{}'",
                        pattern, raw_scenario
                    ),
                });
            }
        }

        if !found {
            return make_response(
                404,
                "Not Found",
                headers,
                br#"{"ok":false,"error":"No matching rule found for pattern"}"#.to_vec(),
            );
        }
        let json = format!(
            "{{\"ok\":true,\"action\":\"scenario_switched\",\"pattern\":\"{}\",\"scenario\":\"{}\",\"status\":{},\"enabled\":{}}}",
            escape_json(&pattern),
            escape_json(&raw_scenario),
            new_status,
            new_enabled
        );
        return make_response(200, "OK", headers, json.into_bytes());
    }

    if req.method == "POST" && path == "/__proxzy/reset" {
        if let Ok(mut cfg) = ctx.config.write() {
            for r in &mut cfg.map_rules {
                r.hits = 0;
            }
            let _ = ctx.tx.send(ProxyEvent::RemoteConfigUpdated(cfg.clone()));
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

async fn forward_request(
    req: &HttpRequest,
    scheme: &str,
    host: &str,
    port: u16,
    url: &str,
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
    let tcp = tokio::time::timeout(Duration::from_secs(20), TcpStream::connect(&addr))
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
        let n = r.read(&mut tmp).await?;
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
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let (host, port) = split_host_port(&host_header, 80);
        let url = build_url("http", &host, port, target);
        Ok(("http".to_string(), host, port, url))
    }
}

fn parse_absolute(url: &str) -> Option<(String, String, u16, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority_end = rest
        .find(|c| c == '/' || c == '?' || c == '#')
        .unwrap_or(rest.len());
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
        cfg.ssl_allowlist = vec!["*".into()];
        assert!(!cfg.should_intercept("anything.com"));
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
        };

        // 1) Upsert mock via POST /__proxzy/mock
        let mock_req = HttpRequest {
            method: "POST".into(),
            target: "/__proxzy/mock".into(),
            headers: Vec::new(),
            body: br#"{"pattern":"https://api.shopwave.dev/v1/orders*","method":"GET","scenario":"success","status":200,"body":"{\"orders\":[1]}"}"#.to_vec(),
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
    }
}
