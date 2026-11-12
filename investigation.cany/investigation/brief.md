# Brief

**Question:** Why does a connected iOS Simulator show no traffic in the Proxzy POC, and how can the app prove capture works while also giving external IDE agents a real hook into the proxy engine?

**Purpose:** Learning POC for the person: see how a Proxyman-style proxy, mock engine and agent-driven verification fit together, and get simulator traffic visible without unsafe system changes.

**Targets:**
- /Applications/Proxyman.app

**Constraints:**
- Clean room: never open, list, read, run or decompile the target
- Build only inside app/; everything else is read-only
- Rust + eframe/egui; build must succeed and --screenshot must produce a non-blank PNG
- Never change host macOS proxy settings without explicit confirmation

**Done when:** A real request proves capture through the gate, the Mac route explains a silent simulator, full HTTPS catch is explicit and safe by default, IDE briefs are copyable, tests pass, screenshots are current, and rebuild_check reports it would pass.
