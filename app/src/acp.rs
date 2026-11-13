// ---------------------------------------------------------------------------
// Agent Client Protocol (ACP) concept model.
//
// ACP is the editor-to-agent side of an AI coding setup. The messages below
// follow the public ACP v1 flow (initialize -> session/new -> session/prompt,
// with session/update notifications and session/request_permission). The
// transcript is a teaching lab, not a live transport: Proxzy is the HTTP gate
// behind the agent, and no external ACP agent is launched.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcpActor {
    Client,
    Agent,
}

impl AcpActor {
    pub fn direction(self) -> &'static str {
        match self {
            AcpActor::Client => "C -> A",
            AcpActor::Agent => "A -> C",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcpKind {
    Request,
    Response,
    Notification,
    Permission,
}

impl AcpKind {
    pub fn label(self) -> &'static str {
        match self {
            AcpKind::Request => "REQUEST",
            AcpKind::Response => "RESULT",
            AcpKind::Notification => "NOTIFY",
            AcpKind::Permission => "PERMISSION",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AcpStep {
    pub actor: AcpActor,
    pub kind: AcpKind,
    pub method: &'static str,
    pub title: &'static str,
    pub note: &'static str,
    pub json: String,
}

/// The user's answer to the simulated `session/request_permission` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcpPermission {
    Pending,
    AllowOnce,
    AllowAlways,
    RejectOnce,
}

impl AcpPermission {
    pub fn label(self) -> &'static str {
        match self {
            AcpPermission::Pending => "PENDING",
            AcpPermission::AllowOnce => "ALLOWED ONCE",
            AcpPermission::AllowAlways => "ALWAYS ALLOWED",
            AcpPermission::RejectOnce => "REJECTED",
        }
    }

    pub fn option_id(self) -> &'static str {
        match self {
            AcpPermission::AllowOnce => "allow-once",
            AcpPermission::AllowAlways => "allow-always",
            AcpPermission::RejectOnce => "reject-once",
            AcpPermission::Pending => "",
        }
    }

    pub fn is_allowed(self) -> bool {
        matches!(
            self,
            AcpPermission::AllowOnce | AcpPermission::AllowAlways
        )
    }
}

/// The permission request always sits at this index: initialize (2 messages),
/// session/new (2), prompt (1), tool_call (1), then the gate.
pub const ACP_GATE_INDEX: usize = 6;

fn with_port(text: &str, port: u16) -> String {
    text.replace("__PORT__", &port.to_string())
}

fn step(
    actor: AcpActor,
    kind: AcpKind,
    method: &'static str,
    title: &'static str,
    note: &'static str,
    json: &str,
    port: u16,
) -> AcpStep {
    AcpStep {
        actor,
        kind,
        method,
        title,
        note,
        json: with_port(json, port),
    }
}

/// Build the visible part of the ACP conversation for the current permission
/// decision. Pending stops at the permission request; rejecting cuts the turn
/// short; allowing continues through the tool result and the final stop reason.
pub fn acp_transcript(port: u16, permission: AcpPermission) -> Vec<AcpStep> {
    let mut steps = vec![
        step(
            AcpActor::Client,
            AcpKind::Request,
            "initialize",
            "Open the connection",
            "Version 1 and the client's capabilities are offered first.",
            r#"{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "initialize",
  "params": {
    "protocolVersion": 1,
    "clientCapabilities": {
      "fs": { "readTextFile": true, "writeTextFile": true },
      "terminal": true
    },
    "clientInfo": {
      "name": "proxzy-editor",
      "title": "Proxzy ACP Lab",
      "version": "1.0.0"
    }
  }
}"#,
            port,
        ),
        step(
            AcpActor::Agent,
            AcpKind::Response,
            "initialize · result",
            "Agree on the version",
            "The agent answers with its own version, capabilities and auth methods.",
            r#"{
  "jsonrpc": "2.0",
  "id": 0,
  "result": {
    "protocolVersion": 1,
    "agentCapabilities": {
      "loadSession": true,
      "promptCapabilities": { "image": false, "audio": false },
      "mcpCapabilities": { "http": true, "sse": false }
    },
    "agentInfo": {
      "name": "demo-acp-agent",
      "title": "Demo ACP Agent",
      "version": "0.4.0"
    },
    "authMethods": []
  }
}"#,
            port,
        ),
        step(
            AcpActor::Client,
            AcpKind::Request,
            "session/new",
            "Create a workspace session",
            "The client names an absolute working folder and any MCP servers the agent may use.",
            r#"{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "session/new",
  "params": {
    "cwd": "/Users/you/Projects/wallet",
    "mcpServers": []
  }
}"#,
            port,
        ),
        step(
            AcpActor::Agent,
            AcpKind::Response,
            "session/new · result",
            "A session id comes back",
            "Every later prompt, update and cancel refers to this conversation id.",
            r#"{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "sessionId": "sess_proxzy_01"
  }
}"#,
            port,
        ),
        step(
            AcpActor::Client,
            AcpKind::Request,
            "session/prompt",
            "Ask for a real check",
            "The editor sends the user's words. The agent owns the model and the tools.",
            r#"{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session/prompt",
  "params": {
    "sessionId": "sess_proxzy_01",
    "prompt": [
      {
        "type": "text",
        "text": "Switch MAP-01 to Error 500, send the checkout request, and report the verdict."
      }
    ]
  }
}"#,
            port,
        ),
        step(
            AcpActor::Agent,
            AcpKind::Notification,
            "session/update · tool_call",
            "The agent wants to act",
            "This notice is one-way; the editor shows the tool and its status.",
            r#"{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_proxzy_01",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "call_01",
      "title": "Switch Map Local rule MAP-01 to Error 500",
      "name": "proxzy_scenario",
      "kind": "execute",
      "status": "pending",
      "rawInput": {
        "url": "http://127.0.0.1:__PORT__/__proxzy/scenario",
        "rule": "MAP-01",
        "scenario": "error"
      }
    }
  }
}"#,
            port,
        ),
        step(
            AcpActor::Agent,
            AcpKind::Permission,
            "session/request_permission",
            "The permission gate",
            "The agent asks the client before a side effect. This is the pause you see in the lab.",
            r#"{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "session/request_permission",
  "params": {
    "sessionId": "sess_proxzy_01",
    "toolCall": { "toolCallId": "call_01" },
    "options": [
      { "optionId": "allow-once", "name": "Allow once", "kind": "allow_once" },
      { "optionId": "allow-always", "name": "Always allow Proxzy mocks", "kind": "allow_always" },
      { "optionId": "reject-once", "name": "Reject", "kind": "reject_once" }
    ]
  }
}"#,
            port,
        ),
    ];

    if permission == AcpPermission::Pending {
        return steps;
    }

    let picked = permission.option_id();
    if permission == AcpPermission::RejectOnce {
        steps.push(step(
            AcpActor::Client,
            AcpKind::Response,
            "permission · result",
            "You said no",
            "The client returns the selected option; the agent must not run the tool.",
            &format!(
                r#"{{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {{
    "outcome": {{
      "outcome": "selected",
      "optionId": "{}"
    }}
  }}
}}"#,
                picked
            ),
            port,
        ));
        steps.push(step(
            AcpActor::Agent,
            AcpKind::Notification,
            "session/update · notice",
            "No traffic changed",
            "One plausible ending: the agent stops instead of trying a different side effect.",
            r#"{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_proxzy_01",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "messageId": "msg_agent_reject",
      "content": {
        "type": "text",
        "text": "Permission was rejected, so I did not change MAP-01 or send the request."
      }
    }
  }
}"#,
            port,
        ));
        steps.push(step(
            AcpActor::Agent,
            AcpKind::Response,
            "session/prompt · result",
            "The turn ends",
            "A stop reason closes the prompt. Here the agent reports refusal.",
            r#"{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "stopReason": "refusal"
  }
}"#,
            port,
        ));
        return steps;
    }

    steps.push(step(
        AcpActor::Client,
        AcpKind::Response,
        "permission · result",
        "You allowed the tool",
        "The result echoes the option id the person picked.",
        &format!(
            r#"{{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {{
    "outcome": {{
      "outcome": "selected",
      "optionId": "{}"
    }}
  }}
}}"#,
            picked
        ),
        port,
    ));
    steps.push(step(
        AcpActor::Agent,
        AcpKind::Notification,
        "session/update · tool_call_update",
        "The mock switch runs",
        "Only now does the agent call the real loopback control plane.",
        r#"{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_proxzy_01",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "call_01",
      "status": "completed",
      "content": [
        {
          "type": "content",
          "content": {
            "type": "text",
            "text": "MAP-01 is on Error. GET /demo/checkout now answers 500."
          }
        }
      ],
      "rawOutput": {
        "rule": "MAP-01",
        "scenario": "error",
        "status": 200
      }
    }
  }
}"#,
        port,
    ));
    steps.push(step(
        AcpActor::Agent,
        AcpKind::Notification,
        "session/update · agent_message_chunk",
        "The agent reports back",
        "Text streams to the editor as a notification, not as a new request.",
        r#"{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_proxzy_01",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "messageId": "msg_agent_42",
      "content": {
        "type": "text",
        "text": "MAP-01 is armed. I sent the checkout call and the gate returned 500 from Map Local."
      }
    }
  }
}"#,
        port,
    ));
    steps.push(step(
        AcpActor::Agent,
        AcpKind::Response,
        "session/prompt · result",
        "The turn ends",
        "The response closes the prompt request with a stop reason.",
        r#"{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "stopReason": "end_turn"
  }
}"#,
        port,
    ));
    steps
}

pub struct AcpCheck {
    pub ok: bool,
    pub title: &'static str,
    pub detail: &'static str,
}

/// A small set of facts the lab can verify from what it has shown so far.
/// These are checks of the teaching transcript, not a protocol conformance run.
pub fn acp_checks(permission: AcpPermission) -> Vec<AcpCheck> {
    let settled = permission != AcpPermission::Pending;
    vec![
        AcpCheck {
            ok: true,
            title: "Version agreed first",
            detail: "initialize swaps protocol version and capabilities before a session exists.",
        },
        AcpCheck {
            ok: true,
            title: "Workspace is explicit",
            detail: "session/new carries an absolute cwd and the MCP server list.",
        },
        AcpCheck {
            ok: settled,
            title: "Side effects ask first",
            detail: if settled {
                "The MAP-01 switch waited at session/request_permission."
            } else {
                "Waiting for your decision at the permission gate."
            },
        },
        AcpCheck {
            ok: settled,
            title: "The turn has a stop reason",
            detail: if permission == AcpPermission::RejectOnce {
                "Rejected: the turn ends with refusal."
            } else if settled {
                "Allowed: the turn ends with end_turn."
            } else {
                "The prompt is still open until the permission is answered."
            },
        },
        AcpCheck {
            ok: true,
            title: "Proxzy only sees HTTP",
            detail: "ACP stays between editor and agent; the gate sees the request the agent sends.",
        },
    ]
}

pub fn acp_editor_config(port: u16) -> String {
    with_port(
        r#"{
  "agent_servers": {
    "my-acp-agent": {
      "command": "/absolute/path/to/agent",
      "args": ["--acp"],
      "env": {
        "PROXZY_PROXY_URL": "http://127.0.0.1:__PORT__"
      }
    }
  }
}"#,
        port,
    )
}

pub fn acp_mcp_entry(port: u16) -> String {
    with_port(
        r#"{
  "name": "proxzy-tools",
  "command": "/absolute/path/to/your-mcp-wrapper",
  "args": ["--port", "__PORT__"],
  "env": [
    {
      "name": "PROXZY_PROXY_URL",
      "value": "http://127.0.0.1:__PORT__"
    }
  ]
}"#,
        port,
    )
}

pub fn acp_agent_prompt(port: u16) -> String {
    with_port(
        r#"Use the Proxzy control plane at http://127.0.0.1:__PORT__/__proxzy.

1. Call /health and show the raw response.
2. Switch Map Local rule MAP-01 to the error scenario.
3. Send the checkout request and report the status you actually received.

Do not claim success without the returned status and body."#,
        port,
    )
}

pub fn acp_summary(port: u16, permission: AcpPermission) -> String {
    let steps = acp_transcript(port, permission);
    let mut out = String::new();
    out.push_str("ACP Bridge — concept session\n");
    out.push_str("Protocol: JSON-RPC 2.0 over stdio (public ACP v1 flow)\n");
    out.push_str(&format!("Control plane: http://127.0.0.1:{}/__proxzy\n", port));
    out.push_str(&format!("Permission: {}\n\n", permission.label()));
    for (index, step) in steps.iter().enumerate() {
        out.push_str(&format!(
            "{:02} {} {} — {}\n",
            index + 1,
            step.actor.direction(),
            step.method,
            step.title
        ));
    }
    out.push_str("\nACP is editor <-> agent. MCP is agent <-> tools. Proxzy sees HTTP, not ACP.\n");
    out
}

#[cfg(test)]
mod acp_tests {
    use super::*;

    #[test]
    fn the_handshake_starts_with_initialize_and_creates_a_session() {
        let steps = acp_transcript(9090, AcpPermission::Pending);
        assert_eq!(steps[0].method, "initialize");
        assert_eq!(steps[0].kind, AcpKind::Request);
        assert_eq!(steps[2].method, "session/new");
        assert!(steps[2].json.contains("/Users/you/Projects/wallet"));
        assert!(steps[2].json.contains("\"mcpServers\": []"));
        assert!(steps[3].json.contains("sess_proxzy_01"));
    }

    #[test]
    fn the_permission_gate_sits_before_the_tool_runs() {
        let steps = acp_transcript(9090, AcpPermission::AllowOnce);
        assert_eq!(steps[ACP_GATE_INDEX].kind, AcpKind::Permission);
        assert_eq!(
            steps[ACP_GATE_INDEX].method,
            "session/request_permission"
        );
        let completed = steps
            .iter()
            .position(|s| s.method == "session/update · tool_call_update")
            .expect("an allowed run should complete the tool");
        assert!(completed > ACP_GATE_INDEX);
        assert!(steps[completed].json.contains("\"status\": \"completed\""));
    }

    #[test]
    fn a_rejection_never_reports_a_completed_tool() {
        let steps = acp_transcript(9090, AcpPermission::RejectOnce);
        assert!(!steps
            .iter()
            .any(|s| s.json.contains("\"status\": \"completed\"")));
        assert!(steps.last().unwrap().json.contains("\"stopReason\": \"refusal\""));
        assert!(steps[ACP_GATE_INDEX + 1].json.contains("\"optionId\": \"reject-once\""));
    }

    #[test]
    fn an_allowed_turn_ends_with_end_turn() {
        let steps = acp_transcript(9090, AcpPermission::AllowAlways);
        assert!(steps.last().unwrap().json.contains("\"stopReason\": \"end_turn\""));
        assert!(steps[ACP_GATE_INDEX + 1].json.contains("allow-always"));
    }

    #[test]
    fn checks_wait_until_the_permission_is_answered() {
        let pending = acp_checks(AcpPermission::Pending);
        assert!(pending.iter().any(|c| !c.ok));
        let allowed = acp_checks(AcpPermission::AllowOnce);
        assert!(allowed.iter().all(|c| c.ok));
        let rejected = acp_checks(AcpPermission::RejectOnce);
        assert!(rejected.iter().all(|c| c.ok));
    }

    #[test]
    fn setup_text_uses_the_live_port_and_names_the_three_layers() {
        let editor = acp_editor_config(9191);
        let mcp = acp_mcp_entry(9191);
        let prompt = acp_agent_prompt(9191);
        assert!(editor.contains("http://127.0.0.1:9191"));
        assert!(editor.contains("agent_servers"));
        assert!(!mcp.contains("mcpServers"));
        assert!(mcp.contains("\"name\": \"proxzy-tools\""));
        assert!(prompt.contains("/__proxzy"));
        let summary = acp_summary(9191, AcpPermission::AllowOnce);
        assert!(summary.contains("editor <-> agent"));
        assert!(summary.contains("Proxzy sees HTTP"));
    }
}
