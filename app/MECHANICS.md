# Proxyman mechanics found in the original app

This sheet records what the original at `/Applications/Proxyman.app` actually
does, where it does it, and which numbers this POC copies. Each row names the
place in the original that settles it: either a function in the Mach-O core
(`ProxymanCore.framework`, opened with the deep-mode decompiler) or one of the
English UI tables inside the app bundle. Values that could not be settled are
marked **estimated**.

## Where the evidence comes from

| Short name | File in the original |
| --- | --- |
| Map Local table | `Contents/Resources/en.lproj/MapLocalViewController.strings` |
| Map Local editor | `Contents/Resources/en.lproj/EditMapLocalViewController.strings` |
| Map Local help | `Contents/Resources/en.lproj/HelpMapLocalViewController.strings` |
| Matching rule | `Contents/Resources/en.lproj/MatchingRuleView.strings` |
| Breakpoint window | `Contents/Resources/en.lproj/BreakpointController.strings` |
| Breakpoint list | `Contents/Resources/en.lproj/BreakpointListController.strings` |
| Compose | `Contents/Resources/en.lproj/Compose.strings` |
| Code generator | `Contents/Resources/en.lproj/CodeGeneratorViewController.strings` |
| Main menu | `Contents/Resources/en.lproj/Main.strings` |
| Filters | `Contents/Resources/en.lproj/Filters.strings` |
| Defaults initialiser | function `0xf34bc` in `ProxymanCore` (writes every user default once) |
| Delay default | function `0x40c9c` in `ProxymanCore` (registers `mapLocalResponseDelay = 10`) |
| Delay presets | `DelayTimePreset` in the `ProxymanCore` Swift reflection string table at `0x10aa192` |
| Per-key getters | `0x42858`, `0x42f8c`, `0x42960`, `0x43050`, `0x42e68`, `0x42e04`, `0x421cc`, `0x40a20` in `ProxymanCore` |
| cURL builder | function `0x2cd188` in `ProxymanCore` (writes the `curl` command line) |
| Label builder | function `0x28fe9c` in `ProxymanCore` (request-row title and query names) |
| Large-file warning | function `0x1bb608` in `ProxymanCore` (reads the 20 MB string at `0xfb01b0`) |

## Functions and tables read this pass

Each row is a function or reflection table opened in the deep-mode decompiler.
The address is the image offset inside `ProxymanCore` (sha256
`817924b660c749797148a3fb9609ec6c08084f5902b9153ae61f88992f2d9b0c`).

| Address | What the function does | Value it settles |
| --- | --- | --- |
| `0xf34bc` | one-shot defaults initialiser: writes each key only if absent | `CurrentProxyPort` = `0x2382` (9090) at label `f5a38`; `composeTimeoutConfig` = `0x1e` (30 s) at table offset `f00`; `IncludeHTTPProxyCURL` = 1 at `0x78`; `monacoEditorTabSize` = 2; `isComposeFollowRedirects` = 1; `isGistAutoRedactSensitiveData` = 1; `isAutoCleanUpMemory` = 1 |
| `0x40c9c` | registers the Map Local delay default | `mapLocalResponseDelay` = **10 ms** |
| `0x42858` | getter for the breakpoint abort mode | raw value **0** → Drop Connection (`dropConnection`, then `return503`, `return429`) |
| `0x42f8c` | getter for the Compose timeout | returns **30** (seconds) |
| `0x42960` | getter for the embedded editor indentation | `monacoEditorTabSize` = **2** |
| `0x43050` | getter for the Compose redirect switch | `isComposeFollowRedirects` = **true** |
| `0x42e68` | getter for Gist redaction | `isGistAutoRedactSensitiveData` = **true** |
| `0x42e04` | getter for the memory clean-up switch | `isAutoCleanUpMemory` = **true** |
| `0x421cc` | getter for Map Local file auto-save | `isAutoSaveOnMapLocalFile` = **true** |
| `0x40a20` | getter for the last Map Local folder | nil / empty until the user picks one |
| `0x1bb608` | builds the size warning for a fixture | compares against **20 MB** and uses the string at `0xfb01b0`: "File size is bigger than 20Mb could impact the performance!" |
| `0x2cd188` | serialises a request as a cURL command | always appends `--proxy http://localhost:<port>`; body as `--data-binary @-` (or `--data-binary "@<path>"` for a file); multipart adds `-H 'Content-Type: multipart/form-data'`; Content-Length is recomputed |
| `0x28fe9c` | builds the request-row label and GraphQL operation name | falls back to the literal `<Missing Query Name>` |
| `0x4bdd4` | reads the `graphQLQueryName` key from a parsed body | key name `graphQLQueryName` |
| `0x10aa192` | `DelayTimePreset` reflection table | cases and payloads: `noDelay` `0x10aa1c2`, `oneSecond` `0x10aa1ca`, `twoSeconds` `0x10aa1d4`, `threeSeconds` `0x10aa1df`, `fiveSeconds` `0x10aa1ec`, `tenSeconds` `0x10aa1f8`, `thirtySeconds` `0x10aa203`, `sixtySeconds` `0x10aa211` |

## Proxy and session

| Rule | Value | Source |
| --- | --- | --- |
| Local HTTP/HTTPS proxy port | **9090** (written as `0x2382`) | defaults initialiser `0xf34bc`, key `CurrentProxyPort` |
| Session traffic list | every request keeps method, status, host, type, time, size; sortable columns and status-family filters | Filters table (`1xx`…`5xx`, JSON/XML/HTML/CSS/Media/Other) |
| Filters combine | AND / OR, each row with Equal, Contains, Not Contains, Starts/Ends, Wildcard, Regex, `>=`, `<` | Filters table |
| Log export | the traffic list can be saved "as Proxyman Log" | Main table |

## Map Local

| Rule | Value | Source |
| --- | --- | --- |
| Rule order | top to bottom, **stop at the first match** | Map Local table and Breakpoint list: "Each request is checked against the rules from top to bottom, stopping when a match is found." |
| What one rule maps | one of three targets: **Local Directory**, **Local File**, or a whole **HTTP/1.x message** (status line + headers + body) | Map Local help; editor "Map To" |
| Local Directory | one folder level, optional **Include SubPath**, root or sub-directories | Map Local help; editor "Enable Local Directory" |
| Missing directory/file | falls back to the **real upstream response** instead of failing | Map Local help: "If the local directory doesn't exist, it will serve from the remote server" |
| Matching rule | URL with wildcard `*` and `?`, full-matching **regex**, or a **GraphQL Query Name** (the name is read from the `graphQLQueryName` body key; when it is absent the row shows `<Missing Query Name>`) | Matching rule table; `0x4bdd4` reads `graphQLQueryName`; `0x28fe9c` supplies the `<Missing Query Name>` fallback |
| Method filter | ANY, GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, TRACE | Matching rule table |
| Subpaths | a checkbox/"Include all subpaths of this URL" appends the wildcard to the path | Matching rule table |
| Delay | "Delay Response" with the presets **No delay, 1 s, 2 s, 3 s, 5 s, 10 s, 30 s, 60 s** plus a typed `customDelayTimeMs`; the registered default is **10** (milliseconds); the last preset **60 s** is also the ceiling | Swift enum `DelayTimePreset` in `ProxymanCore` (reflection string table, 0x10aa192); `0x40c9c` |
| Large fixture warning | shows a warning above **20 MB** ("File size is bigger than 20Mb could impact the performance!") | `0x1bb608` compares against 20 MB and loads the string at `0xfb01b0` |
| Fixture auto-save | the editor saves the linked local file automatically, and the switch is **on** by default | `0x421cc` returns true for `isAutoSaveOnMapLocalFile`; Map Local editor ("Auto-Save") |
| Empty body | an empty rule still returns the chosen status, so it works as an "Empty 200" scenario | editor fields |
| Row actions | New `⌘N`, Edit `⌘⏎`, Delete `⌘⌫`, Duplicate `⌘D`, Toggle `Space`, Enable/Disable all | Map Local table footer |

## Breakpoints

| Rule | Value | Source |
| --- | --- | --- |
| Rule order | top to bottom, **first match wins** | Breakpoint list |
| What can be edited | URL, method, **status code**, headers and body, for the request **or** the response stage | Breakpoint list: "Support URL, Method, Status Code, Headers, and Body." |
| Continue | `⌘.` — send original data, ignore edits | Breakpoint window tooltips |
| Execute | `⌘⏎` — send the **edited** request/response | Breakpoint window tooltips |
| Abort | `⌘\` — stop that connection | Breakpoint window tooltips |
| Abort by | **Drop Connection** (default, raw `0`), **503 Service Unavailable**, or **429 Too Many Requests** | Breakpoint window "Abort by" menu; `BreakpointAbortOption` cases in `ProxymanCore` (`dropConnection`, `return503`, `return429`); defaults initialiser `0xf34bc` writes `0` |
| Bulk actions | Continue All, Execute All, Abort All | Breakpoint window |
| Templates | save the edited message as a template, manage templates, reset to the original | raw editor table |
| Window focus | a new paused breakpoint can bring its window to the front | Breakpoint window checkbox; defaults key `isFocusOnWindowForNewBreakpoints` |
| Empty state | "No Breakpoints" with a Manage Rules button; the window can close when nothing is paused | Breakpoint window |
| Raw editor | a valid HTTP message view with a requests/response switch | Breakpoint raw editor |

## Compose (Postman-like request builder)

| Rule | Value | Source |
| --- | --- | --- |
| Send | SEND `⌘⏎` | Compose table |
| Import | "Import from cURL…" parses a pasted cURL into method, URL, headers, body | Compose table |
| Timeout | default **30 seconds**; presets 15 s, 30 s, 60 s and 0 (no timeout) | defaults initialiser `0xf34bc` (`composeTimeoutConfig = 0x1e`); Compose table |
| Redirects | "Automatically Follow Redirects" is a switch and is **on** by default (`isComposeFollowRedirects`) | Compose table; `0x43050` returns true; defaults initialiser `0xf34bc` writes 1 |
| Body in GET | ignored, with the hint "Please use POST" | Compose table |
| Templates | GET with query, POST with JSON, POST with form, POST with multipart, empty request | Compose table |
| Request editor | monospace editor with **2-space** indentation | `0x42960` returns 2 for `monacoEditorTabSize`; defaults initialiser `0xf34bc` |
| History | every sent request is kept in a History list | Compose table |

## Copy, share and repeat

| Rule | Value | Source |
| --- | --- | --- |
| Copy as cURL | main menu "Copy as cURL", also HTTPie text; the generated command always carries `--proxy http://localhost:<proxy port>` | Main table; `0x2cd188` appends `--proxy http://localhost:<port>` |
| Code generator | cURL, HTTPie, HAR, Postman Collection 2, Python + Request, Node + Fetch, Node + HTTP, Go, Java + HTTPClient, Dart, Swift + URLSession, Swift + Alamofire, Swift + Moya, Objective-C + NSURLSession, JavaScript + jQuery | Code generator table |
| Repeat a call | re-send from the saved request, optionally without caching | Main table "No Caching"; `noCachingOnRepeatedRequest` default key |
| Share | publish a session to a Gist, with redaction of sensitive headers as an option | Main table (`Publish to Gist`, `isGistAutoRedactSensitiveData`) |

## What this POC copies exactly

- 9090 as the default proxy port (with the same 9091–9093 fallback).
- Map Local: first matching rule wins; wildcard URLs and GraphQL operation
  names; a method filter of Any/GET/POST/PUT/DELETE; the original's eight delay
  presets plus a typed millisecond value; linked local file read on every
  request; upstream fallback when the file is missing; a warning above 20 MB;
  the three-scenario switch (Success / Error / Empty).
- Breakpoints: a live pause at the request stage with method, URL, headers and
  body editing; Continue and an **Abort by** choice of Drop Connection
  (default), 503 or 429, answered with a small JSON error body for the two
  status modes.
- Compose: cURL import, GraphQL and REST presets, redirect following **on by
  default** (the `-L` flag), a 2-space body editor, a send timeout of 15 s / 30 s
  (default) / 60 s / Off passed to `curl` as `--max-time`.
- Copy: URL, cURL (with the live `--proxy http://localhost:<port>` prefix),
  HTTPie, Postman Collection 2.1, HAR 1.2, Python `requests` and Markdown share,
  grouped under one exchange row in the share sheet.
- Log dashboard: one filterable timeline of captures, mocks, breakpoints and
  system events with a session summary.

## What is estimated or still open

- The POC does **not** implement the original's full-matching **regex** mode,
  the full method list (HEAD/OPTIONS/TRACE/PATCH), Local Directory mapping,
  gist publishing, or a live response-stage pause; the response-stage editor is
  a simulated session. Only HTTP/1.1 is proxied.
- **Session retention** and memory limits are not visible in the resources that
  were read; the POC keeps the current session only and does not claim a limit.
- Pixel-level layout, colours and fonts are not measured; the POC uses its own
  three-part dark layout built from the app's structure, not a pixel copy.
