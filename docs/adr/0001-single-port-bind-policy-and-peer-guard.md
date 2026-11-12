---
status: accepted
---

# Single-Port Bind Policy, Portal Split, and Peer Guard

Separating the local HTTP proxy, the `/__proxzy/*` control plane, and the phone certificate portal across multiple TCP ports would break standard single-port mobile proxy setup and add extra firewall/ADB rules (`adb reverse tcp:PORT tcp:PORT`). Instead, ProxzyCraft serves all three planes on one proxy port gated by [`BindPolicy`](file:///Users/paponsuapmongkonchai/MoneyWarProject/ProxzyCraft/app/src/proxy.rs#L50), [`classify_service_request`](file:///Users/paponsuapmongkonchai/MoneyWarProject/ProxzyCraft/app/src/proxy.rs#L1441), [`authorize_control`](file:///Users/paponsuapmongkonchai/MoneyWarProject/ProxzyCraft/app/src/proxy.rs#L1490), and an upstream SSRF peer guard.

The trade-off is strict in-handler routing: (1) TLS termination never exposes `/__proxzy/*`, so external HTTPS destinations cannot smuggle control calls; (2) origin-form `/ca.*` and `http://proxzy.local/` serve read-only CA certificates from anywhere able to reach the port, whereas `/__proxzy/*` rejects browser `Origin` headers, requires loopback `Host` (`127.0.0.1` / `localhost` / `[::1]`) or LAN IP when enabled, and requires `X-Proxzy-Control` from `control.token` whenever the listener is bound to LAN; and (3) non-loopback proxy peers cannot forward to loopback or link-local metadata addresses (`169.254.0.0/16`, `fc00::/7`, etc.) unless the listener is local-only or explicitly targetable via localhost.

