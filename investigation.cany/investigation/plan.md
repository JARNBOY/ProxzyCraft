# Plan

1. [done] Read the original's menus, tables and defaults — ProxymanCore defaults initialiser 0xf34bc and the English .strings tables fix the port, compose timeout, abort menu, copy formats and rule order.
2. [done] Write the mechanics sheet and JSON — app/MECHANICS.md and app/mechanics.json now record 29 values, every one with a source.
3. [done] Add the Breakpoint Abort-by modes — Drop / 503 / 429, with Drop as the original's default; the proxy drops the socket or answers a JSON error.
4. [done] Add Compose timeout, share formats and Map Local delay presets — 15/30/60/Off timeout passed to curl, HTTPie/Postman/HAR in the share sheet, the original's eight delay presets plus a 20 MB fixture warning.
5. [done] Decompile the per-key getters and the cURL/label builders — 0x42858, 0x42f8c, 0x42960, 0x43050, 0x42e68, 0x42e04, 0x421cc and 0x40a20 settle the defaults; 0x2cd188 and 0x28fe9c settle cURL and GraphQL labels.
6. [done] Apply the decompiled values in the Rust POC — New model constants, live proxy in cURL, -L follow-redirects default, 2-space indentation, 10 ms delay default, 60 s ceiling, 20 MB threshold.
7. [done] Build, test and photograph the new controls — Clean build, 92 tests pass, and verify-mechanics.png confirms the Follow chip and inspector draw.
8. [done] Check the build against the original and update the handoff — rebuild_check reports build, screenshot and mechanics all passed (29 sourced values), and says this would pass.
