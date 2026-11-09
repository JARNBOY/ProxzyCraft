# Brief

**Question:** Rebuild a Proxyman-style network debugging desktop app (traffic list, inspector, cURL/share, repeat, Map Local, Breakpoints, log dashboard) as a Rust + egui POC from the person's description, with no access to the original.

**Purpose:** Learning POC for the person: see how capture, mocking, breakpoint editing and logging fit together.

**Targets:**
- /Applications/Proxyman.app

**Constraints:**
- Clean room: never open, list, read, run or decompile the target
- Build only inside app/
- Rust + eframe/egui per instructions

**Done when:** cargo build succeeds, --screenshot produces a non-blank window PNG, and all requested feature areas are present and usable.
