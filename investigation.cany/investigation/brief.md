# Brief

**Question:** How can a Proxyman-style desktop POC show live GraphQL and REST traffic with copy/share URL and cURL, repeat, Map Local (including a linked fixture file), Breakpoints and a Log Dashboard, and what are the original's real defaults and rules?

**Purpose:** The person wants to learn how Proxyman's Map Local, Breakpoint and Log dashboard pieces fit, and build a working POC in Rust/egui.

**Targets:**
- /Applications/Proxyman.app

**Constraints:**
- Deep mode: the original may be opened and decompiled, but never run or modified
- Build only inside app/; everything else is read-only
- Rust + eframe/egui; build must succeed and --screenshot must produce a non-blank PNG
- Never change host macOS proxy settings without explicit confirmation

**Done when:** The app builds, draws a real window, captures or simulates GraphQL and REST calls, copies URL/cURL, repeats, serves Map Local from inline JSON or a linked file, pauses at breakpoints, shows a log dashboard, passes its tests, and rebuild_check reports it would pass.
