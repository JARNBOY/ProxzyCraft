# Brief

**Question:** Extend the Proxzy POC with the automation layer the person asked for: an Agent Control page, a WireMock-style API Test Bench, a Screen Play & MCP page with per-platform port config and response injection, and a Map Local connector pipeline view.

**Purpose:** Learning POC for the person: see how Map Local, agent-driven mocks, full API tests and mobile UI-test sync fit together.

**Targets:**
- /Applications/Proxyman.app

**Constraints:**
- Clean room: never open, list, read, run or decompile the target
- Build only inside app/
- Rust + eframe/egui; build must succeed and --screenshot must produce a non-blank PNG

**Done when:** cargo build and tests succeed, --screenshot works for the new pages, and rebuild_check reports it would pass.
