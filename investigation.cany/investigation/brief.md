# Brief

**Question:** How can a Proxyman-style desktop POC show live GraphQL and REST traffic with copy/share, repeat, Map Local (including a linked fixture file), Breakpoints and a Log Dashboard, so the person can learn how the pieces fit together?

**Purpose:** A learning POC for the person: inspect requests, copy a URL or cURL, repeat a call, answer an endpoint from Map Local, pause a real request at a breakpoint, and read one log dashboard for the session.

**Targets:**
- /Applications/Proxyman.app

**Constraints:**
- Clean room: never open, list, read, run or decompile the target
- Build only inside app/; everything else is read-only
- Rust + eframe/egui; build must succeed and --screenshot must produce a non-blank PNG
- Never change host macOS proxy settings without explicit confirmation

**Done when:** The app builds, draws a real window, captures or simulates GraphQL and REST calls, copies URL/cURL, repeats, serves Map Local from inline JSON or a linked file, pauses at request/response breakpoints, shows a log dashboard, passes its tests, and rebuild_check reports it would pass.
