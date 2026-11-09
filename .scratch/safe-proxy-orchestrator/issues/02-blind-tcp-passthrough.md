# Ticket 02: Blind TCP Passthrough Engine

**Status:** Resolved  
**Parent Spec:** `.scratch/safe-proxy-orchestrator/spec.md`  
**Blocked by:** `01-domain-allowlist-model.md` (Resolved)  
**Target Files:** `app/src/proxy.rs`

---

## 1. Goal
Implement raw bidirectional streaming between the client and upstream server on `HTTP CONNECT` requests when a target is not whitelisted for TLS termination.

## 2. Acceptance Criteria
- [x] Implement `async fn passthrough_tunnel(mut client: TcpStream, host: &str, port: u16, ctx: ConnCtx) -> Result<(), String>`.
- [x] Connect upstream via `tokio::net::TcpStream::connect((host, port))`.
- [x] Send `HTTP/1.1 200 Connection Established\r\nProxy-Agent: Proxzy\r\n\r\n` to client upon successful upstream connection.
- [x] Execute `tokio::io::copy_bidirectional(&mut client, &mut upstream)`.
- [x] Handle clean disconnects (e.g. client or server closes socket) without logging spurious error alarms.
- [x] Emit a lightweight `ProxyEvent::Log` indicating passthrough tunnel establishment for observability.
- [x] Ensure no TLS termination is attempted on unwhitelisted connections.

