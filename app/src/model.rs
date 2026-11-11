use egui::Color32;

pub const BG: Color32 = Color32::from_rgb(22, 22, 24);
pub const RAIL: Color32 = Color32::from_rgb(26, 26, 28);
pub const PANEL: Color32 = Color32::from_rgb(30, 30, 33);
pub const CARD: Color32 = Color32::from_rgb(37, 37, 41);
pub const INPUT: Color32 = Color32::from_rgb(24, 24, 27);
pub const HOVER: Color32 = Color32::from_rgb(47, 47, 52);
pub const BORDER: Color32 = Color32::from_rgb(55, 55, 61);
pub const TEXT: Color32 = Color32::from_rgb(236, 237, 241);
pub const MUTED: Color32 = Color32::from_rgb(154, 156, 166);
pub const FAINT: Color32 = Color32::from_rgb(112, 114, 124);
pub const ACCENT: Color32 = Color32::from_rgb(88, 140, 255);
pub const GREEN: Color32 = Color32::from_rgb(61, 214, 149);
pub const AMBER: Color32 = Color32::from_rgb(255, 181, 84);
pub const RED: Color32 = Color32::from_rgb(255, 107, 107);
pub const PURPLE: Color32 = Color32::from_rgb(184, 140, 255);
pub const CYAN: Color32 = Color32::from_rgb(77, 208, 225);

pub fn method_color(method: &str) -> Color32 {
    match method {
        "GET" => GREEN,
        "POST" => AMBER,
        "PUT" => Color32::from_rgb(96, 165, 250),
        "PATCH" => PURPLE,
        "DELETE" => RED,
        "CONNECT" => Color32::from_rgb(148, 163, 184),
        "OPTIONS" | "HEAD" => MUTED,
        _ => TEXT,
    }
}

pub fn status_color(status: u16) -> Color32 {
    match status {
        0 => RED,
        200..=299 => GREEN,
        300..=399 => CYAN,
        400..=499 => AMBER,
        _ => RED,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApiKind {
    Graphql,
    Rest,
    Media,
    Tunnel,
    Other,
}

impl ApiKind {
    pub fn label(self) -> &'static str {
        match self {
            ApiKind::Graphql => "GraphQL",
            ApiKind::Rest => "REST",
            ApiKind::Media => "Media",
            ApiKind::Tunnel => "Tunnel",
            ApiKind::Other => "Other",
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            ApiKind::Graphql => PURPLE,
            ApiKind::Rest => Color32::from_rgb(120, 180, 255),
            ApiKind::Media => CYAN,
            ApiKind::Tunnel => Color32::from_rgb(148, 163, 184),
            ApiKind::Other => MUTED,
        }
    }
}

#[derive(Clone)]
pub struct GraphqlInfo {
    pub operation: String,
    pub op_type: String,
    pub variables: String,
}

#[derive(Clone)]
pub struct Capture {
    pub id: u64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub latency_ms: u64,
    pub size_bytes: u64,
    pub kind: ApiKind,
    pub started: String,
    pub remote: String,
    pub protocol: String,
    pub req_headers: Vec<(String, String)>,
    pub req_body: String,
    pub resp_headers: Vec<(String, String)>,
    pub resp_body: String,
    pub graphql: Option<GraphqlInfo>,
    pub mock: Option<String>,
    pub pinned: bool,
    pub aborted: bool,
}

impl Capture {
    pub fn host(&self) -> String {
        self.url
            .split("://")
            .nth(1)
            .unwrap_or(&self.url)
            .split('/')
            .next()
            .unwrap_or("")
            .to_string()
    }

    pub fn path(&self) -> String {
        let rest = self.url.split("://").nth(1).unwrap_or(&self.url);
        match rest.find('/') {
            Some(i) => rest[i..].to_string(),
            None => "/".to_string(),
        }
    }

    pub fn host_path(&self) -> String {
        format!("{}{}", self.host(), self.path())
    }

    pub fn is_error(&self) -> bool {
        self.status == 0 || self.status >= 400 || self.aborted
    }

    /// True for calls that came through the local proxy (or the real Composer),
    /// as opposed to the seeded sample rows that ship with the POC.
    pub fn is_live(&self) -> bool {
        matches!(
            self.protocol.as_str(),
            "HTTP/1.1" | "HTTPS (MITM)" | "TCP Tunnel (Raw)"
        )
    }

    pub fn is_tunnel(&self) -> bool {
        self.method.eq_ignore_ascii_case("CONNECT") || self.kind == ApiKind::Tunnel
    }

    pub fn is_system_protected(&self) -> bool {
        is_builtin_protected_domain(&self.host())
    }

    pub fn started_clock(&self) -> &str {
        &self.started
    }

    pub fn size_label(&self) -> String {
        if self.size_bytes >= 1024 {
            format!("{:.1} KB", self.size_bytes as f64 / 1024.0)
        } else if self.size_bytes == 0 {
            "-".to_string()
        } else {
            format!("{} B", self.size_bytes)
        }
    }

    pub fn curl(&self) -> String {
        let mut out = format!("curl -X {} '{}'", self.method, self.url);
        for (k, v) in &self.req_headers {
            out.push_str(&format!(" \\\n  -H '{}: {}'", k, v.replace('\'', "'\\''")));
        }
        if !self.req_body.is_empty() {
            out.push_str(&format!(
                " \\\n  --data-raw '{}'",
                self.req_body.replace('\'', "'\\''")
            ));
        }
        out
    }

    pub fn share_link(&self) -> String {
        format!("proxzy://share/req_{:x}", self.id)
    }
}

// ---------------------------------------------------------------------------
// URL rules
//
// A rule is a short pattern, not a full URL:
//   /v1/products             -> contains this text (old behaviour, easy to type)
//   */v1/products?*           -> any host, this path, any query string
//   */v1/products?limit=20*   -> only when the query starts with limit=20
//   *graphql*                 -> anything whose URL contains graphql
// Asterisks are the only wildcard. `?` is kept literal because it starts the
// query string, which is exactly where people need it.
// ---------------------------------------------------------------------------

pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut retry = 0usize;
    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            pi += 1;
            retry = ti;
        } else if let Some(s) = star {
            pi = s + 1;
            retry += 1;
            ti = retry;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn without_scheme(url: &str) -> &str {
    url.split("://").nth(1).unwrap_or(url)
}

fn path_of(url: &str) -> &str {
    let rest = without_scheme(url);
    match rest.find('/') {
        Some(i) => &rest[i..],
        None => "/",
    }
}

pub fn pattern_matches_url(pattern: &str, url: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if !pattern.contains('*') {
        return url.to_lowercase().contains(&pattern.to_lowercase());
    }
    let url_lower = url.to_lowercase();
    let host_path = without_scheme(url_lower.as_str());
    let path = path_of(url_lower.as_str());
    wildcard_match(pattern, &url_lower)
        || wildcard_match(pattern, host_path)
        || wildcard_match(pattern, path)
}

/// Matches hostnames for Selective SSL Proxying.
/// Supports exact match (`api.example.com`), wildcard subdomains (`*.example.com`),
/// port stripping, and wildcard catch-all (`*`).
pub fn domain_matches(pattern: &str, target_host: &str) -> bool {
    let p = pattern.trim().to_lowercase();
    let raw_h = target_host.trim().to_lowercase();
    if p.is_empty() {
        return false;
    }
    if p == "*" || p == "*.*" {
        return true;
    }
    if raw_h == p {
        return true;
    }

    let h_host = raw_h.split(':').next().unwrap_or(&raw_h);
    let p_host = p.split(':').next().unwrap_or(&p);

    if let Some(suffix) = p_host.strip_prefix("*.") {
        if h_host == suffix || h_host.ends_with(&format!(".{}", suffix)) {
            if let Some(p_port) = p.split(':').nth(1) {
                let h_port = raw_h.split(':').nth(1).unwrap_or("443");
                return h_port == p_port;
            }
            return true;
        }
    }

    if h_host == p_host {
        if let Some(p_port) = p.split(':').nth(1) {
            let h_port = raw_h.split(':').nth(1).unwrap_or("443");
            return h_port == p_port;
        }
        return true;
    }

    false
}

/// Open-source safety list of system and certificate verification domains.
/// These domains are ALWAYS safely bound as raw TCP data without TLS decryption
/// to prevent breaking host OS push notifications, updates, and certificate validation.
pub static BUILTIN_PROTECTED_DOMAINS: &[&str] = &[
    // Apple System Services & Push Notifications
    "*.push.apple.com",
    "identity.apple.com",
    "swscan.apple.com",
    "swdist.apple.com",
    "appldnld.apple.com",
    "gateway.icloud.com",
    // Certificate Authorities & Revocation (OCSP / CRL)
    "*.digicert.com",
    "*.symcb.com",
    "*.symcd.com",
    "*.verisign.com",
    "ocsp.*",
    "crl.*",
    // Android & Google System Services
    "connectivitycheck.gstatic.com",
    "play.googleapis.com",
    "android.clients.google.com",
    // Microsoft System Update
    "*.windowsupdate.com",
];

/// Returns true if the given host matches ProxzyCraft's Open-Source Safety Policy
/// for protected system domains where TLS decoding must be bypassed.
pub fn is_builtin_protected_domain(host: &str) -> bool {
    let clean = host.split(':').next().unwrap_or(host).trim().to_lowercase();
    BUILTIN_PROTECTED_DOMAINS
        .iter()
        .any(|pat| domain_matches(pat, &clean))
}

pub fn pattern_matches_graphql(pattern: &str, body: &str) -> bool {
    let name = pattern.trim();
    if name.is_empty() {
        return true;
    }
    graphql_info("POST", body)
        .map(|g| g.operation.to_lowercase().contains(&name.to_lowercase()))
        .unwrap_or(false)
}

pub fn detect_kind(method: &str, url: &str, body: &str) -> ApiKind {
    if method.eq_ignore_ascii_case("CONNECT") {
        return ApiKind::Tunnel;
    }
    let u = url.to_lowercase();
    if u.contains("graphql") || u.contains("/gql") || body.contains("\"query\"") {
        ApiKind::Graphql
    } else if u.contains(".png")
        || u.contains(".jpg")
        || u.contains(".jpeg")
        || u.contains(".webp")
        || u.contains(".svg")
        || u.contains(".mp4")
        || u.contains(".woff")
    {
        ApiKind::Media
    } else if method == "OPTIONS" || method == "HEAD" {
        ApiKind::Other
    } else {
        ApiKind::Rest
    }
}

/// Read a JSON string field without pulling in a full JSON parser.
/// Finds `"name"`, then the following `:`, then decodes the quoted string.
fn json_string_field(body: &str, name: &str) -> Option<String> {
    let needle = format!("\"{}\"", name);
    let at = body.find(&needle)? + needle.len();
    let rest = body[at..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = rest[1..].chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(out),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{0008}'),
                Some('f') => out.push('\u{000c}'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(code) {
                            out.push(c);
                        }
                    }
                }
                Some(other) => out.push(other),
                None => break,
            },
            other => out.push(other),
        }
    }
    None
}

/// Read a `{...}` or `[...]` JSON value field and return it as text.
fn json_value_field(body: &str, name: &str) -> Option<String> {
    let needle = format!("\"{}\"", name);
    let at = body.find(&needle)? + needle.len();
    let rest = body[at..].trim_start().strip_prefix(':')?.trim_start();
    let first = rest.chars().next()?;
    if first != '{' && first != '[' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in rest.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn find_word(haystack: &str, word: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let needle = word.as_bytes();
    if bytes.len() < needle.len() {
        return None;
    }
    for i in 0..=bytes.len() - needle.len() {
        if &bytes[i..i + needle.len()] != needle {
            continue;
        }
        let before_ok = i == 0
            || !(bytes[i - 1] as char).is_ascii_alphanumeric() && bytes[i - 1] != b'_';
        let after = i + needle.len();
        let after_ok =
            after == bytes.len() || !(bytes[after] as char).is_ascii_alphanumeric() && bytes[after] != b'_';
        if before_ok && after_ok {
            return Some(i);
        }
    }
    None
}

/// Parse the operation type and name out of a GraphQL document. Handles both
/// a raw document and the common JSON envelope `{ "query": "...", "variables": ... }`.
pub fn graphql_info(_method: &str, body: &str) -> Option<GraphqlInfo> {
    let trimmed = body.trim_start();
    let (document, variables) = if trimmed.starts_with('{') && body.contains("\"query\"") {
        let document = json_string_field(body, "query")?;
        let variables = json_value_field(body, "variables")
            .map(|v| pretty_print(&v))
            .unwrap_or_else(|| "{}".to_string());
        (document, variables)
    } else {
        (body.to_string(), "{}".to_string())
    };

    let lower = document.to_lowercase();
    let (op_type, pos, len) = if let Some(p) = find_word(&lower, "subscription") {
        ("subscription", p, "subscription".len())
    } else if let Some(p) = find_word(&lower, "mutation") {
        ("mutation", p, "mutation".len())
    } else if let Some(p) = find_word(&lower, "query") {
        ("query", p, "query".len())
    } else {
        // A shorthand GraphQL document starts straight with `{ ... }` and uses
        // no JSON punctuation (no `":` pairs and no commas between fields).
        let looks_like_json = trimmed.contains("\":") || trimmed.contains("\",");
        if trimmed.starts_with('{') && !body.contains("\"query\"") && !looks_like_json {
            return Some(GraphqlInfo {
                operation: "Anonymous".into(),
                op_type: "query".into(),
                variables,
            });
        }
        return None;
    };

    let after = document[pos + len..].trim_start();
    let name = after
        .split(|c: char| c.is_whitespace() || c == '(' || c == '{' || c == '@')
        .next()
        .unwrap_or("")
        .trim();
    let operation = if name.is_empty() { "Anonymous" } else { name };
    Some(GraphqlInfo {
        operation: operation.to_string(),
        op_type: op_type.to_string(),
        variables,
    })
}

pub fn pretty_print(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 64);
    let mut indent: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    for ch in body.chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '{' | '[' => {
                out.push(ch);
                indent += 1;
                out.push('\n');
                for _ in 0..indent {
                    out.push_str("  ");
                }
            }
            '}' | ']' => {
                indent = (indent - 1).max(0);
                out.push('\n');
                for _ in 0..indent {
                    out.push_str("  ");
                }
                out.push(ch);
            }
            ',' => {
                out.push(ch);
                out.push('\n');
                for _ in 0..indent {
                    out.push_str("  ");
                }
            }
            ':' => {
                out.push(ch);
                out.push(' ');
            }
            _ => out.push(ch),
        }
    }
    out.trim().to_string()
}

/// Re-indent a JSON body without changing its values. Returns `None` when the
/// text is not a complete JSON object or array, so the caller can show the raw
/// body with a short note instead of inventing structure.
pub fn json_pretty(body: &str) -> Option<String> {
    let text = body.trim();
    if !(text.starts_with('{') || text.starts_with('[')) {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 64);
    let mut indent = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut stack: Vec<char> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '{' | '[' => {
                out.push(ch);
                stack.push(ch);
                let next = next_nonspace(&chars, i + 1);
                let empty = matches!((ch, next), ('{', Some('}')) | ('[', Some(']')));
                if !empty {
                    indent += 1;
                    push_indented_line(&mut out, indent);
                }
            }
            '}' | ']' => {
                let open = stack.pop();
                let prev = prev_nonspace(&chars, i);
                let empty = matches!(
                    (open, prev),
                    (Some('{'), Some('{')) | (Some('['), Some('['))
                );
                if !empty {
                    indent = indent.saturating_sub(1);
                    push_indented_line(&mut out, indent);
                }
                out.push(ch);
            }
            ',' => {
                out.push(ch);
                push_indented_line(&mut out, indent);
            }
            ':' => {
                out.push(':');
                out.push(' ');
            }
            c if c.is_whitespace() => {}
            _ => out.push(ch),
        }
        i += 1;
    }
    if in_string || !stack.is_empty() {
        return None;
    }
    Some(out.trim_end().to_string())
}

/// Put each XML or HTML tag on its own indented line. Text is kept as written,
/// and common void elements (`<br>`, `<img>`, …) do not open a level.
pub fn markup_pretty(body: &str) -> Option<String> {
    let text = body.trim();
    if !text.contains('<') || !(text.starts_with('<') || text.contains("</")) {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 64);
    let mut indent = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '<' {
            let mut j = i + 1;
            let mut quote: Option<char> = None;
            while j < chars.len() {
                let c = chars[j];
                if let Some(q) = quote {
                    if c == q {
                        quote = None;
                    }
                } else if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '>' {
                    break;
                }
                j += 1;
            }
            if j >= chars.len() {
                let rest: String = chars[i..].iter().collect();
                push_markup_line(&mut out, indent, &rest);
                break;
            }
            let tag: String = chars[i..=j].iter().collect();
            let lower = tag.to_lowercase();
            let closing = lower.starts_with("</");
            let comment = lower.starts_with("<!--");
            let declaration = lower.starts_with("<!");
            let processing = lower.starts_with("<?");
            let self_closing = lower.ends_with("/>")
                || comment
                || declaration
                || processing
                || is_void_tag(&lower);
            if closing {
                indent = indent.saturating_sub(1);
            }
            push_markup_line(&mut out, indent, &tag);
            if !closing && !self_closing {
                indent += 1;
            }
            i = j + 1;
        } else {
            let start = i;
            while i < chars.len() && chars[i] != '<' {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                push_markup_line(&mut out, indent, trimmed);
            }
        }
    }
    Some(out.trim_start().to_string())
}

fn next_nonspace(chars: &[char], from: usize) -> Option<char> {
    chars[from..].iter().copied().find(|c| !c.is_whitespace())
}

fn prev_nonspace(chars: &[char], before: usize) -> Option<char> {
    let mut i = before;
    while i > 0 {
        i -= 1;
        if !chars[i].is_whitespace() {
            return Some(chars[i]);
        }
    }
    None
}

fn push_indented_line(out: &mut String, indent: usize) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn push_markup_line(out: &mut String, indent: usize, line: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(line);
}

fn is_void_tag(lower: &str) -> bool {
    let name: String = lower
        .trim_start_matches('<')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    matches!(
        name.as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub fn now_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let tod = secs % 86400;
    format!(
        "{:02}:{:02}:{:02}",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

fn hdr(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

pub fn sample_captures() -> Vec<Capture> {
    let products_resp = r#"{
  "data": {
    "products": {
      "nodes": [
        {
          "id": "prd_1042",
          "name": "Aero Running Shoe",
          "price": { "amount": 128.00, "currency": "USD" },
          "inStock": true,
          "rating": 4.7,
          "image": "https://cdn.shopwave.dev/p/1042.webp"
        },
        {
          "id": "prd_1043",
          "name": "Trail Runner GTX",
          "price": { "amount": 164.50, "currency": "USD" },
          "inStock": true,
          "rating": 4.5,
          "image": "https://cdn.shopwave.dev/p/1043.webp"
        },
        {
          "id": "prd_1044",
          "name": "Cloudfoam Trainer",
          "price": { "amount": 96.00, "currency": "USD" },
          "inStock": false,
          "rating": 4.2,
          "image": "https://cdn.shopwave.dev/p/1044.webp"
        }
      ],
      "pageInfo": {
        "hasNextPage": true,
        "endCursor": "YXJyYXljb25uZWN0aW9uOjI0"
      }
    }
  },
  "extensions": { "tookMs": 31, "cache": "MISS" }
}"#;

    let products_req = r#"query GetProducts($first: Int!, $filter: ProductFilter) {
  products(first: $first, filter: $filter) {
    nodes {
      id
      name
      price { amount currency }
      inStock
      rating
      image
    }
    pageInfo { hasNextPage endCursor }
  }
}"#;

    let cart_req = r#"mutation UpdateCart($input: CartInput!) {
  updateCart(input: $input) {
    cart { id totalQuantity subtotal { amount currency } }
    userErrors { field message }
  }
}"#;

    let cart_resp = r#"{
  "data": {
    "updateCart": {
      "cart": {
        "id": "cart_9f21",
        "totalQuantity": 3,
        "subtotal": { "amount": 256.00, "currency": "USD" }
      },
      "userErrors": []
    }
  }
}"#;

    let wishlist_body = r#"{"data":{"addToWishlist":{"wishlist":{"id":"wl_221","itemCount":7},"userErrors":[]}}}"#;

    vec![
        Capture {
            id: 101,
            method: "POST".into(),
            url: "https://api.shopwave.dev/graphql".into(),
            status: 200,
            latency_ms: 32,
            size_bytes: 4296,
            kind: ApiKind::Graphql,
            started: "12:04:38.912".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
                ("x-client-version", "3.14.0"),
                ("x-request-id", "req_01HQ8T4M2P"),
                ("accept-encoding", "gzip, br"),
            ]),
            req_body: products_req.to_string(),
            resp_headers: hdr(&[
                ("content-type", "application/json; charset=utf-8"),
                ("content-encoding", "br"),
                ("cache-control", "no-store"),
                ("x-request-id", "req_01HQ8T4M2P"),
                ("server", "cloudflare"),
                ("date", "Sun, 09 Nov 2025 12:04:38 GMT"),
            ]),
            resp_body: products_resp.to_string(),
            graphql: Some(GraphqlInfo {
                operation: "GetProducts".into(),
                op_type: "query".into(),
                variables: r#"{
  "first": 12,
  "filter": { "collection": "running", "inStock": true }
}"#
                .into(),
            }),
            mock: None,
            pinned: true,
            aborted: false,
        },
        Capture {
            id: 100,
            method: "GET".into(),
            url: "https://api.shopwave.dev/v1/products?limit=20&collection=running".into(),
            status: 200,
            latency_ms: 48,
            size_bytes: 2184,
            kind: ApiKind::Rest,
            started: "12:04:38.740".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("accept", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
                ("x-client-version", "3.14.0"),
            ]),
            req_body: String::new(),
            resp_headers: hdr(&[
                ("content-type", "application/json; charset=utf-8"),
                ("cache-control", "public, max-age=60"),
                ("server", "cloudflare"),
            ]),
            resp_body: r#"{
  "items": [
    { "id": "prd_1042", "name": "Aero Running Shoe", "price": 128.00 },
    { "id": "prd_1043", "name": "Trail Runner GTX", "price": 164.50 }
  ],
  "page": { "limit": 20, "offset": 0, "total": 42 }
}"#
            .into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 99,
            method: "POST".into(),
            url: "https://api.shopwave.dev/graphql".into(),
            status: 200,
            latency_ms: 61,
            size_bytes: 1320,
            kind: ApiKind::Graphql,
            started: "12:04:37.981".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
                ("x-request-id", "req_01HQ8T4JX2"),
            ]),
            req_body: cart_req.to_string(),
            resp_headers: hdr(&[
                ("content-type", "application/json; charset=utf-8"),
                ("cache-control", "no-store"),
            ]),
            resp_body: cart_resp.to_string(),
            graphql: Some(GraphqlInfo {
                operation: "UpdateCart".into(),
                op_type: "mutation".into(),
                variables: r#"{
  "input": { "cartId": "cart_9f21", "quantity": 3 }
}"#
                .into(),
            }),
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 98,
            method: "GET".into(),
            url: "https://cdn.shopwave.dev/assets/hero-shoes.webp".into(),
            status: 200,
            latency_ms: 12,
            size_bytes: 188416,
            kind: ApiKind::Media,
            started: "12:04:37.204".into(),
            remote: "104.18.33.12:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("accept", "image/avif,image/webp,*/*"),
                ("referer", "https://shopwave.dev/"),
            ]),
            req_body: String::new(),
            resp_headers: hdr(&[
                ("content-type", "image/webp"),
                ("cache-control", "public, max-age=31536000, immutable"),
                ("content-length", "188416"),
            ]),
            resp_body: "[binary image data - 184 KB]".into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 97,
            method: "POST".into(),
            url: "https://auth.shopwave.dev/oauth/token".into(),
            status: 200,
            latency_ms: 88,
            size_bytes: 892,
            kind: ApiKind::Rest,
            started: "12:04:36.889".into(),
            remote: "104.18.31.90:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/x-www-form-urlencoded"),
                ("accept", "application/json"),
            ]),
            req_body: "grant_type=refresh_token&client_id=web_store&refresh_token=demo_refresh_4c2".into(),
            resp_headers: hdr(&[
                ("content-type", "application/json"),
                ("cache-control", "no-store"),
            ]),
            resp_body: r#"{
  "access_token": "eyJhbGciOiJIUzI1NiJ9.demo.token",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "demo_refresh_4c2"
}"#
            .into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 96,
            method: "GET".into(),
            url: "https://api.shopwave.dev/v1/recommendations?userId=u_8841".into(),
            status: 200,
            latency_ms: 21,
            size_bytes: 1140,
            kind: ApiKind::Rest,
            started: "12:04:36.102".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("accept", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
            ]),
            req_body: String::new(),
            resp_headers: hdr(&[
                ("content-type", "application/json"),
                ("x-proxzy-rule", "Recommendations cache"),
                ("cache-control", "no-store"),
            ]),
            resp_body: r#"{
  "cached": true,
  "forUser": "u_8841",
  "items": [
    { "id": "prd_2011", "name": "Everyday Crew Sock", "score": 0.94 },
    { "id": "prd_1880", "name": "Recovery Slide", "score": 0.88 },
    { "id": "prd_1755", "name": "Run Club Cap", "score": 0.81 }
  ]
}"#
            .into(),
            graphql: None,
            mock: Some("Recommendations cache".into()),
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 95,
            method: "GET".into(),
            url: "https://api.shopwave.dev/v1/orders/8421".into(),
            status: 401,
            latency_ms: 17,
            size_bytes: 334,
            kind: ApiKind::Rest,
            started: "12:04:35.774".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("accept", "application/json"),
                ("authorization", "Bearer demo_expired_8f3c"),
            ]),
            req_body: String::new(),
            resp_headers: hdr(&[
                ("content-type", "application/json"),
                ("www-authenticate", "Bearer error=\"invalid_token\""),
            ]),
            resp_body: r#"{
  "error": "unauthorized",
  "message": "Access token expired",
  "code": "AUTH_TOKEN_EXPIRED",
  "requestId": "req_01HQ8T4H11"
}"#
            .into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 94,
            method: "POST".into(),
            url: "https://api.shopwave.dev/graphql".into(),
            status: 503,
            latency_ms: 120,
            size_bytes: 512,
            kind: ApiKind::Graphql,
            started: "12:04:35.120".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
                ("x-request-id", "req_01HQ8T4G80"),
            ]),
            req_body: r#"mutation PlaceOrder($input: PlaceOrderInput!) {
  placeOrder(input: $input) {
    order { id status total { amount currency } }
    userErrors { field message }
  }
}"#
            .into(),
            resp_headers: hdr(&[
                ("content-type", "application/json"),
                ("retry-after", "30"),
                ("x-proxzy-rule", "Checkout maintenance"),
            ]),
            resp_body: r#"{
  "error": "service_unavailable",
  "message": "Checkout is in maintenance mode. Please try again later.",
  "code": "CHECKOUT_MAINTENANCE",
  "maintenanceWindow": "2025-11-09T12:00:00Z/2025-11-09T13:00:00Z"
}"#
            .into(),
            graphql: Some(GraphqlInfo {
                operation: "PlaceOrder".into(),
                op_type: "mutation".into(),
                variables: r#"{
  "input": { "cartId": "cart_9f21", "paymentMethod": "card_demo" }
}"#
                .into(),
            }),
            mock: Some("Checkout maintenance".into()),
            pinned: true,
            aborted: false,
        },
        Capture {
            id: 93,
            method: "GET".into(),
            url: "https://api.shopwave.dev/v1/users/me".into(),
            status: 200,
            latency_ms: 39,
            size_bytes: 1876,
            kind: ApiKind::Rest,
            started: "12:04:34.512".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("accept", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
            ]),
            req_body: String::new(),
            resp_headers: hdr(&[
                ("content-type", "application/json; charset=utf-8"),
                ("cache-control", "private, max-age=0"),
            ]),
            resp_body: r#"{
  "id": "u_8841",
  "email": "alex@example.com",
  "firstName": "Alex",
  "lastName": "Morgan",
  "tier": "gold",
  "memberSince": "2022-04-18",
  "preferences": { "currency": "USD", "locale": "en-US" }
}"#
            .into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 92,
            method: "PUT".into(),
            url: "https://api.shopwave.dev/v1/cart/items/14".into(),
            status: 200,
            latency_ms: 27,
            size_bytes: 648,
            kind: ApiKind::Rest,
            started: "12:04:33.908".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
            ]),
            req_body: r#"{ "quantity": 2, "variantId": "var_1042_42" }"#.into(),
            resp_headers: hdr(&[("content-type", "application/json")]),
            resp_body: r#"{
  "item": {
    "id": "item_14",
    "productId": "prd_1042",
    "quantity": 2,
    "unitPrice": 128.00
  },
  "cart": { "id": "cart_9f21", "totalQuantity": 3 }
}"#
            .into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 91,
            method: "DELETE".into(),
            url: "https://api.shopwave.dev/v1/cart/items/9".into(),
            status: 204,
            latency_ms: 14,
            size_bytes: 0,
            kind: ApiKind::Rest,
            started: "12:04:33.301".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[("authorization", "Bearer demo_8f3c...a91")]),
            req_body: String::new(),
            resp_headers: hdr(&[("date", "Sun, 09 Nov 2025 12:04:33 GMT")]),
            resp_body: String::new(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 90,
            method: "POST".into(),
            url: "https://telemetry.shopwave.dev/collect".into(),
            status: 204,
            latency_ms: 9,
            size_bytes: 0,
            kind: ApiKind::Rest,
            started: "12:04:32.884".into(),
            remote: "34.120.54.18:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/json"),
                ("user-agent", "Shopwave/3.14.0 (macOS)"),
            ]),
            req_body: r#"{ "event": "product_list_view", "props": { "surface": "search", "count": 20 } }"#.into(),
            resp_headers: hdr(&[("date", "Sun, 09 Nov 2025 12:04:32 GMT")]),
            resp_body: String::new(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 89,
            method: "GET".into(),
            url: "https://api.shopwave.dev/v1/search?q=running+shoe&limit=20".into(),
            status: 200,
            latency_ms: 55,
            size_bytes: 3412,
            kind: ApiKind::Rest,
            started: "12:04:32.113".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("accept", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
            ]),
            req_body: String::new(),
            resp_headers: hdr(&[("content-type", "application/json")]),
            resp_body: r#"{
  "query": "running shoe",
  "took": 54,
  "hits": 42,
  "results": [
    { "id": "prd_1042", "name": "Aero Running Shoe", "score": 9.8 },
    { "id": "prd_1043", "name": "Trail Runner GTX", "score": 8.9 },
    { "id": "prd_1177", "name": "Velocity 2", "score": 8.1 }
  ]
}"#
            .into(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 88,
            method: "POST".into(),
            url: "https://api.shopwave.dev/graphql".into(),
            status: 200,
            latency_ms: 44,
            size_bytes: 736,
            kind: ApiKind::Graphql,
            started: "12:04:31.502".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[
                ("content-type", "application/json"),
                ("authorization", "Bearer demo_8f3c...a91"),
            ]),
            req_body: r#"mutation AddWishlist($productId: ID!) {
  addToWishlist(productId: $productId) {
    wishlist { id itemCount }
    userErrors { field message }
  }
}"#
            .into(),
            resp_headers: hdr(&[("content-type", "application/json")]),
            resp_body: wishlist_body.to_string(),
            graphql: Some(GraphqlInfo {
                operation: "AddWishlist".into(),
                op_type: "mutation".into(),
                variables: "{\n  \"productId\": \"prd_1042\"\n}".into(),
            }),
            mock: None,
            pinned: false,
            aborted: false,
        },
        Capture {
            id: 87,
            method: "POST".into(),
            url: "https://api.shopwave.dev/graphql".into(),
            status: 200,
            latency_ms: 28,
            size_bytes: 408,
            kind: ApiKind::Graphql,
            started: "12:04:30.874".into(),
            remote: "104.18.32.47:443".into(),
            protocol: "HTTP/2".into(),
            req_headers: hdr(&[("content-type", "application/json")]),
            req_body: r#"query GetCart { cart { id totalQuantity } }"#.into(),
            resp_headers: hdr(&[("content-type", "application/json")]),
            resp_body: r#"{ "data": { "cart": { "id": "cart_9f21", "totalQuantity": 3 } } }"#.into(),
            graphql: Some(GraphqlInfo {
                operation: "GetCart".into(),
                op_type: "query".into(),
                variables: "{}".into(),
            }),
            mock: None,
            pinned: false,
            aborted: false,
        },
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuleMethod {
    Any,
    Get,
    Post,
    Put,
    Delete,
}

impl RuleMethod {
    pub fn label(self) -> &'static str {
        match self {
            RuleMethod::Any => "ANY",
            RuleMethod::Get => "GET",
            RuleMethod::Post => "POST",
            RuleMethod::Put => "PUT",
            RuleMethod::Delete => "DELETE",
        }
    }
    pub fn matches(self, method: &str) -> bool {
        match self {
            RuleMethod::Any => true,
            RuleMethod::Get => method == "GET",
            RuleMethod::Post => method == "POST",
            RuleMethod::Put => method == "PUT",
            RuleMethod::Delete => method == "DELETE",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MockScenario {
    Success,
    Error,
    Empty,
}

impl MockScenario {
    pub fn label(self) -> &'static str {
        match self {
            MockScenario::Success => "Success",
            MockScenario::Error => "Error",
            MockScenario::Empty => "Empty",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            MockScenario::Success => "success",
            MockScenario::Error => "error",
            MockScenario::Empty => "empty",
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "success" | "ok" | "200" => Some(MockScenario::Success),
            "error" | "err" | "fail" | "500" | "503" => Some(MockScenario::Error),
            "empty" | "none" | "blank" => Some(MockScenario::Empty),
            _ => None,
        }
    }

    pub fn default_status(self) -> u16 {
        match self {
            MockScenario::Success => 200,
            MockScenario::Error => 500,
            MockScenario::Empty => 200,
        }
    }

    pub fn default_body(self) -> String {
        match self {
            MockScenario::Success => r#"{
  "ok": true,
  "data": {
    "id": "mock_1",
    "status": "ready"
  },
  "source": "proxzy-mock"
}"#
            .into(),
            MockScenario::Error => r#"{
  "error": "internal_server_error",
  "message": "Simulated error state from ProxzyCraft UI test mock",
  "code": "MOCK_ERROR_500"
}"#
            .into(),
            MockScenario::Empty => r#"{
  "items": [],
  "data": [],
  "total": 0,
  "source": "proxzy-mock-empty"
}"#
            .into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCurl {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// Tokenize a shell command (supporting single quotes, double quotes, and backslash line continuations).
fn shell_tokens(input: &str) -> Vec<String> {
    let normalized = input.replace("\\\r\n", " ").replace("\\\n", " ");
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = normalized.chars().peekable();

    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
            }
        } else if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' || next == '$' || next == '`' {
                        current.push(chars.next().unwrap());
                    } else if next == 'n' {
                        let _ = chars.next();
                        current.push('\n');
                    } else {
                        current.push(c);
                    }
                }
            } else {
                current.push(c);
            }
        } else {
            match c {
                '\'' => in_single = true,
                '"' => in_double = true,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(c),
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Parse a pasted `curl` command (or direct URL) into method, URL, headers, and body.
pub fn parse_curl(input: &str) -> Option<ParsedCurl> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Also accept a plain direct URL if someone pastes https://... into the cURL box
    if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        && !trimmed.contains(char::is_whitespace)
    {
        return Some(ParsedCurl {
            method: "GET".into(),
            url: trimmed.to_string(),
            headers: Vec::new(),
            body: String::new(),
        });
    }

    let tokens = shell_tokens(trimmed);
    if tokens.is_empty() {
        return None;
    }

    let mut method: Option<String> = None;
    let mut url: Option<String> = None;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body_parts: Vec<String> = Vec::new();

    let mut i = 0;
    if tokens[0].eq_ignore_ascii_case("curl") {
        i = 1;
    }

    while i < tokens.len() {
        let tok = &tokens[i];
        match tok.as_str() {
            "-X" | "--request" => {
                if i + 1 < tokens.len() {
                    method = Some(tokens[i + 1].to_ascii_uppercase());
                    i += 2;
                    continue;
                }
            }
            "-H" | "--header" => {
                if i + 1 < tokens.len() {
                    if let Some((k, v)) = tokens[i + 1].split_once(':') {
                        let k = k.trim();
                        if !k.is_empty() {
                            headers.push((k.to_string(), v.trim().to_string()));
                        }
                    }
                    i += 2;
                    continue;
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode" | "--json" => {
                if tok == "--json"
                    && !headers
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                {
                    headers.push(("content-type".into(), "application/json".into()));
                }
                if i + 1 < tokens.len() {
                    body_parts.push(tokens[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--url" => {
                if i + 1 < tokens.len() {
                    url = Some(tokens[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "-u" | "--user" | "-A" | "--user-agent" | "-x" | "--proxy" | "-o" | "--output"
            | "-b" | "--cookie" | "-c" | "--cookie-jar" | "--cacert" | "--connect-timeout"
            | "-m" | "--max-time" => {
                i += 2;
                continue;
            }
            _ => {
                if let Some(m) = tok.strip_prefix("-X") {
                    if !m.is_empty() {
                        method = Some(m.to_ascii_uppercase());
                        i += 1;
                        continue;
                    }
                }
                if tok.starts_with("http://") || tok.starts_with("https://") {
                    url = Some(tok.clone());
                } else if !tok.starts_with('-') && url.is_none() && tok.contains('.') {
                    url = Some(format!("https://{}", tok));
                }
            }
        }
        i += 1;
    }

    let url = url?;
    let body = body_parts.join("&");
    let final_method = method.unwrap_or_else(|| {
        if !body.is_empty() {
            "POST".into()
        } else {
            "GET".into()
        }
    });

    Some(ParsedCurl {
        method: final_method,
        url,
        headers,
        body,
    })
}

/// Extract a clean hostname (or wildcard domain) from a URL or rule pattern so
/// adding a mock rule can automatically register the host in `ssl_allowlist`.
pub fn extract_domain_from_pattern(pattern: &str) -> Option<String> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return None;
    }
    let without_scheme = if let Some((_, rest)) = trimmed.split_once("://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix('*') {
        if rest.starts_with('/') {
            return None;
        }
        trimmed
    } else {
        trimmed
    };
    let host_port = without_scheme
        .split(&['/', '?', '#'][..])
        .next()
        .unwrap_or("")
        .trim();
    if host_port.is_empty() || !host_port.contains('.') {
        return None;
    }
    // Strip trailing wildcard unless it's a leading `*.domain.com`
    let cleaned = host_port.trim_end_matches('*');
    if cleaned.is_empty() || !cleaned.contains('.') {
        return None;
    }
    Some(cleaned.to_ascii_lowercase())
}

/// Sanitize sensitive PII and secrets (tokens, passwords, phone numbers, citizen IDs)
/// from real response bodies, converting them into safe test template placeholders.
pub fn sanitize_mock_payload(body: &str) -> String {
    let sensitive_keys = [
        ("access_token", "\"{{TOKEN}}\""),
        ("refresh_token", "\"{{TOKEN}}\""),
        ("token", "\"{{TOKEN}}\""),
        ("password", "\"{{PASSWORD}}\""),
        ("secret", "\"{{SECRET}}\""),
        ("otp", "\"{{OTP}}\""),
        ("otp_code", "\"{{OTP}}\""),
        ("phone", "\"{{USERNAME}}\""),
        ("mobile", "\"{{USERNAME}}\""),
        ("phone_number", "\"{{USERNAME}}\""),
        ("citizen_id", "\"1100000000000\""),
        ("national_id", "\"1100000000000\""),
        ("id_card", "\"1100000000000\""),
    ];

    let mut out = body.to_string();
    for (key, replacement) in sensitive_keys {
        let needle = format!("\"{}\"", key);
        let mut search_from = 0;
        while let Some(rel_pos) = out[search_from..].find(&needle) {
            let key_pos = search_from + rel_pos;
            let after_key = &out[key_pos + needle.len()..];
            if let Some(colon) = after_key.find(':') {
                let val_start = key_pos + needle.len() + colon + 1;
                let remaining = &out[val_start..];
                let leading_spaces = remaining.len() - remaining.trim_start().len();
                let trimmed = remaining.trim_start();
                if trimmed.starts_with('"') {
                    if let Some(end_quote) = trimmed[1..].find('"') {
                        let total_old_len = leading_spaces + 1 + end_quote + 1;
                        out.replace_range(val_start..val_start + total_old_len, &format!(" {}", replacement));
                        search_from = val_start + replacement.len() + 1;
                        continue;
                    }
                } else if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
                    let num_len = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
                    let total_old_len = leading_spaces + num_len;
                    out.replace_range(val_start..val_start + total_old_len, &format!(" {}", replacement));
                    search_from = val_start + replacement.len() + 1;
                    continue;
                }
            }
            search_from = key_pos + needle.len();
        }
    }
    out
}

/// Transform a real URL into a cross-environment wildcard pattern (Prod <-> Preprod compatible).
/// e.g. "https://api.sample.com/v1/wallet" -> "*sample.com*/v1/wallet*"
pub fn make_environment_wildcard_pattern(url: &str) -> String {
    let clean = url.trim();
    if clean.starts_with('*') {
        return clean.to_string();
    }
    let without_proto = clean
        .strip_prefix("https://")
        .or_else(|| clean.strip_prefix("http://"))
        .unwrap_or(clean);

    let base = without_proto.split('?').next().unwrap_or(without_proto).trim_end_matches('/');
    if let Some(slash_idx) = base.find('/') {
        let host = &base[..slash_idx];
        let path = &base[slash_idx..];
        let parts: Vec<&str> = host.split('.').collect();
        let domain_base = if parts.len() >= 2 {
            parts[parts.len() - 2..].join(".")
        } else {
            host.to_string()
        };
        format!("*{}*{}*", domain_base, path)
    } else {
        format!("*{}*", base)
    }
}

/// Automatically derive an empty-state scenario body (200 OK with empty arrays and zeroes).
pub fn auto_derive_empty_scenario(body: &str) -> String {
    let mut out = body.to_string();
    let mut i = 0;
    while i < out.len() {
        if let Some(bracket_start) = out[i..].find('[') {
            let start = i + bracket_start;
            let mut depth = 0;
            let mut end = None;
            for (idx, c) in out[start..].char_indices() {
                if c == '[' {
                    depth += 1;
                } else if c == ']' {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + idx);
                        break;
                    }
                }
            }
            if let Some(bracket_end) = end {
                out.replace_range(start..=bracket_end, "[]");
                i = start + 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    out
}

/// Canonical readable & scalable file convention for a mock rule scenario:
/// `mocks/<domain>/<method>_<path_slug>/<scenario>.json`
pub fn mock_fixture_path(method: RuleMethod, pattern: &str, scenario: MockScenario) -> String {
    let domain = extract_domain_from_pattern(pattern)
        .unwrap_or_else(|| "local.app".to_string())
        .replace("*.", "wildcard.")
        .replace(':', "_");
    let raw_path = if let Some((_, rest)) = pattern.split_once("://") {
        rest.find('/').map(|idx| &rest[idx..]).unwrap_or("/root")
    } else if let Some(idx) = pattern.find('/') {
        &pattern[idx..]
    } else {
        pattern
    };
    let base_path = raw_path.split('?').next().unwrap_or(raw_path);
    let mut slug: String = base_path
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    while slug.contains("__") {
        slug = slug.replace("__", "_");
    }
    let slug = slug.trim_matches('_');
    let slug = if slug.is_empty() { "root" } else { slug };
    format!(
        "mocks/{}/{}_{}/{}.json",
        domain,
        method.label().to_ascii_lowercase(),
        slug,
        scenario.slug()
    )
}

/// Canonical scalable versioned fixture path:
/// `mocks/<env>/v<version>/<domain>/<method>_<path_slug>/<scenario>.json`
/// If `env` or `version` are empty, falls back gracefully to `mocks/<domain>/...`
pub fn mock_versioned_fixture_path(
    env: &str,
    version: &str,
    method: RuleMethod,
    pattern: &str,
    scenario: MockScenario,
) -> String {
    let clean_env = env.trim().to_lowercase();
    let clean_ver = version.trim().trim_start_matches('v').to_lowercase();
    let base_fixture = mock_fixture_path(method, pattern, scenario);
    if clean_env.is_empty() && clean_ver.is_empty() {
        return base_fixture;
    }
    let prefix = match (!clean_env.is_empty(), !clean_ver.is_empty()) {
        (true, true) => format!("mocks/{}/v{}/", clean_env, clean_ver),
        (true, false) => format!("mocks/{}/", clean_env),
        (false, true) => format!("mocks/v{}/", clean_ver),
        (false, false) => "mocks/".into(),
    };
    base_fixture.replacen("mocks/", &prefix, 1)
}

#[derive(Clone)]
pub struct MapRule {
    pub id: u64,
    pub name: String,
    pub folder: String,
    pub pattern: String,
    pub method: RuleMethod,
    pub status: u16,
    pub content_type: String,
    pub delay_ms: u64,
    pub body: String,
    pub graphql_name: String,
    pub enabled: bool,
    pub hits: u32,
    pub active_scenario: MockScenario,
    pub success_status: u16,
    pub success_body: String,
    pub error_status: u16,
    pub error_body: String,
    pub empty_status: u16,
    pub empty_body: String,
    pub inject_props: std::collections::HashMap<String, String>,
    pub app_version: String,
    pub env: String,
}

impl Default for MapRule {
    fn default() -> Self {
        let success_body = MockScenario::Success.default_body();
        Self {
            id: 0,
            name: String::new(),
            folder: "General".into(),
            pattern: String::new(),
            method: RuleMethod::Any,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 20,
            body: success_body.clone(),
            graphql_name: String::new(),
            enabled: true,
            hits: 0,
            active_scenario: MockScenario::Success,
            success_status: 200,
            success_body,
            error_status: 500,
            error_body: MockScenario::Error.default_body(),
            empty_status: 200,
            empty_body: MockScenario::Empty.default_body(),
            inject_props: std::collections::HashMap::new(),
            app_version: String::new(),
            env: String::new(),
        }
    }
}

impl MapRule {
    pub fn folder_name(&self) -> &str {
        let trimmed = self.folder.trim();
        if trimmed.is_empty() {
            "General"
        } else {
            trimmed
        }
    }

    /// Render body applying dynamic injections:
    /// 1. Replaces `{{KEY}}`, `{{key}}`, `{{Key}}` placeholders from `inject_props`
    /// 2. Replaces direct JSON key values if present
    pub fn render_injected_body(&self) -> String {
        if self.inject_props.is_empty() {
            return self.body.clone();
        }
        let mut rendered = self.body.clone();
        for (k, v) in &self.inject_props {
            let upper = format!("{{{{{}}}}}", k.to_ascii_uppercase());
            let lower = format!("{{{{{}}}}}", k.to_ascii_lowercase());
            let exact = format!("{{{{{}}}}}", k);
            rendered = rendered.replace(&upper, v);
            rendered = rendered.replace(&lower, v);
            rendered = rendered.replace(&exact, v);

            let key_needle = format!("\"{}\"", k);
            if let Some(pos) = rendered.find(&key_needle) {
                let after_key = &rendered[pos + key_needle.len()..];
                if let Some(colon) = after_key.find(':') {
                    let val_start = pos + key_needle.len() + colon + 1;
                    let remaining = &rendered[val_start..];
                    let leading_spaces = remaining.len() - remaining.trim_start().len();
                    let trimmed = remaining.trim_start();
                    if trimmed.starts_with('"') {
                        if let Some(end_quote) = trimmed[1..].find('"') {
                            let total_old_len = leading_spaces + 1 + end_quote + 1;
                            let prefix = if leading_spaces > 0 { " " } else { "" };
                            let replacement = format!("{}\"{}\"", prefix, v);
                            rendered.replace_range(val_start..val_start + total_old_len, &replacement);
                        }
                    }
                }
            }
        }
        rendered
    }

    pub fn matches(&self, method: &str, url: &str, body: &str) -> bool {
        self.enabled
            && self.method.matches(method)
            && pattern_matches_url(&self.pattern, url)
            && pattern_matches_graphql(&self.graphql_name, body)
    }

    /// Save the currently active `status` and `body` back into its scenario slot,
    /// then activate `target` and load its `status` and `body`.
    pub fn switch_scenario(&mut self, target: MockScenario) {
        match self.active_scenario {
            MockScenario::Success => {
                self.success_status = self.status;
                self.success_body = self.body.clone();
            }
            MockScenario::Error => {
                self.error_status = self.status;
                self.error_body = self.body.clone();
            }
            MockScenario::Empty => {
                self.empty_status = self.status;
                self.empty_body = self.body.clone();
            }
        }
        self.active_scenario = target;
        match target {
            MockScenario::Success => {
                self.status = self.success_status;
                self.body = self.success_body.clone();
            }
            MockScenario::Error => {
                self.status = self.error_status;
                self.body = self.error_body.clone();
            }
            MockScenario::Empty => {
                self.status = self.empty_status;
                self.body = self.empty_body.clone();
            }
        }
    }

    pub fn fixture_path(&self) -> String {
        mock_fixture_path(self.method, &self.pattern, self.active_scenario)
    }

    pub fn fixture_versioned_path(&self) -> String {
        mock_versioned_fixture_path(&self.env, &self.app_version, self.method, &self.pattern, self.active_scenario)
    }
}

/// Extract all JSON key names from a JSON string without external dependencies.
pub fn extract_json_keys(json: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            let mut escaped = false;
            while j < len {
                if bytes[j] == b'\\' && !escaped {
                    escaped = true;
                } else if bytes[j] == b'"' && !escaped {
                    break;
                } else {
                    escaped = false;
                }
                j += 1;
            }
            if j < len {
                let candidate = &json[start..j];
                let mut k = j + 1;
                while k < len && (bytes[k] == b' ' || bytes[k] == b'\t' || bytes[k] == b'\r' || bytes[k] == b'\n') {
                    k += 1;
                }
                if k < len && bytes[k] == b':' {
                    let key_str = candidate.trim().to_string();
                    if !key_str.is_empty() && !keys.contains(&key_str) {
                        keys.push(key_str);
                    }
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    keys
}

/// Detailed contract drift breakdown classifying breaking vs additive changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractDriftReport {
    pub is_drift_detected: bool,
    pub missing_in_live: Vec<String>, // Potential breaking change: client expects field, live API omitted it!
    pub added_in_live: Vec<String>,   // Additive / new feature change: live API sent new field client didn't mock
    pub notes: Vec<String>,
}

/// Analyze contract drift and categorize breaking vs additive changes.
pub fn analyze_contract_drift(mock_json: &str, live_json: &str) -> ContractDriftReport {
    let mock_keys = extract_json_keys(mock_json);
    let live_keys = extract_json_keys(live_json);
    let mut missing_in_live = Vec::new();
    let mut added_in_live = Vec::new();
    let mut notes = Vec::new();

    for k in &mock_keys {
        if !live_keys.contains(k) {
            missing_in_live.push(k.clone());
            notes.push(format!("BREAKING: Missing key in live response: \"{}\"", k));
        }
    }
    for k in &live_keys {
        if !mock_keys.contains(k) {
            added_in_live.push(k.clone());
            notes.push(format!("ADDITIVE: New key in live response: \"{}\"", k));
        }
    }
    let is_drift_detected = !missing_in_live.is_empty() || !added_in_live.is_empty();
    ContractDriftReport {
        is_drift_detected,
        missing_in_live,
        added_in_live,
        notes,
    }
}

/// Detect contract drift between expected mock payload and actual live response payload.
/// Returns a list of drift notes (e.g. missing keys or new unmocked keys).
pub fn detect_json_contract_drift(mock_json: &str, live_json: &str) -> Vec<String> {
    analyze_contract_drift(mock_json, live_json).notes
}

/// Audit entry for each network request processed during an active test session.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionCallAudit {
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub latency_ms: u64,
    pub source: String,
    pub app_version: String,
    pub baseline_version: String,
    pub env: String,
    pub is_drift_detected: bool,
    pub drift_notes: Vec<String>,
    pub breaking_changes_count: usize,
    pub additive_changes_count: usize,
}

/// Active Test Session for QA Testcase screen play execution.
/// Scopes auto-recording and captures a comprehensive audit trail.
#[derive(Clone, Debug, PartialEq)]
pub struct TestSession {
    pub name: String,
    pub folder: String,
    pub app_version: String,
    pub baseline_version: String,
    pub env: String,
    pub filter_domains: Vec<String>,
    pub filter_paths: Vec<String>,
    pub started_at: String,
    pub total_calls: usize,
    pub mock_calls: usize,
    pub upstream_calls: usize,
    pub auto_recorded_calls: usize,
    pub drift_warnings: usize,
    pub breaking_drifts: usize,
    pub additive_drifts: usize,
    pub audit_log: Vec<SessionCallAudit>,
}

impl TestSession {
    pub fn new(
        name: &str,
        folder: &str,
        app_version: &str,
        baseline_version: &str,
        env: &str,
        filter_domains: Vec<String>,
        filter_paths: Vec<String>,
    ) -> Self {
        Self {
            name: name.to_string(),
            folder: if folder.trim().is_empty() { "General".into() } else { folder.trim().to_string() },
            app_version: app_version.trim().to_string(),
            baseline_version: baseline_version.trim().to_string(),
            env: env.trim().to_string(),
            filter_domains,
            filter_paths,
            started_at: now_stamp(),
            total_calls: 0,
            mock_calls: 0,
            upstream_calls: 0,
            auto_recorded_calls: 0,
            drift_warnings: 0,
            breaking_drifts: 0,
            additive_drifts: 0,
            audit_log: Vec::new(),
        }
    }

    pub fn matches_scope(&self, host: &str, path: &str) -> bool {
        let domain_match = self.filter_domains.is_empty()
            || self.filter_domains.iter().any(|d| domain_matches(d, host));
        let path_match = self.filter_paths.is_empty()
            || self.filter_paths.iter().any(|p| pattern_matches_url(p, path));
        domain_match && path_match
    }

    pub fn record_call(&mut self, call: SessionCallAudit) {
        self.total_calls += 1;
        if call.source.starts_with("MOCK") {
            self.mock_calls += 1;
        } else if call.source.starts_with("AUTO_RECORDED") {
            self.auto_recorded_calls += 1;
        } else {
            self.upstream_calls += 1;
        }
        if call.is_drift_detected {
            self.drift_warnings += 1;
            self.breaking_drifts += call.breaking_changes_count;
            self.additive_drifts += call.additive_changes_count;
        }
        self.audit_log.push(call);
    }
}

pub fn sample_map_rules() -> Vec<MapRule> {
    let prod_body = sample_captures()[0].resp_body.clone();
    let rec_body = sample_captures()[5].resp_body.clone();
    let maint_body = r#"{
  "error": "service_unavailable",
  "message": "Checkout is in maintenance mode.",
  "code": "CHECKOUT_MAINTENANCE"
}"#
    .to_string();
    vec![
        MapRule {
            id: 1,
            name: "GraphQL products fixture".into(),
            pattern: "graphql".into(),
            method: RuleMethod::Any,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 18,
            body: prod_body.clone(),
            graphql_name: "GetProducts".into(),
            enabled: true,
            hits: 12,
            active_scenario: MockScenario::Success,
            success_status: 200,
            success_body: prod_body,
            ..MapRule::default()
        },
        MapRule {
            id: 2,
            name: "Checkout maintenance".into(),
            pattern: "/v1/checkout".into(),
            method: RuleMethod::Post,
            status: 503,
            content_type: "application/json".into(),
            delay_ms: 120,
            body: maint_body.clone(),
            graphql_name: String::new(),
            enabled: true,
            hits: 3,
            active_scenario: MockScenario::Error,
            error_status: 503,
            error_body: maint_body,
            ..MapRule::default()
        },
        MapRule {
            id: 3,
            name: "Recommendations cache".into(),
            pattern: "/v1/recommendations".into(),
            method: RuleMethod::Get,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 21,
            body: rec_body.clone(),
            graphql_name: String::new(),
            enabled: false,
            hits: 8,
            active_scenario: MockScenario::Success,
            success_status: 200,
            success_body: rec_body,
            ..MapRule::default()
        },
        MapRule {
            id: 4,
            name: "Wallet balance".into(),
            folder: "Wallet".into(),
            pattern: "/v1/wallet*".into(),
            method: RuleMethod::Get,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 14,
            body: r#"{
  "balance": 128.50,
  "currency": "THB",
  "user": "{{USERNAME}}",
  "updated_at": "2026-03-14T10:40:20Z"
}"#
            .into(),
            graphql_name: String::new(),
            enabled: true,
            hits: 4,
            active_scenario: MockScenario::Success,
            success_status: 200,
            success_body: r#"{
  "balance": 128.50,
  "currency": "THB",
  "user": "{{USERNAME}}",
  "updated_at": "2026-03-14T10:40:20Z"
}"#
            .into(),
            error_status: 500,
            error_body: r#"{
  "error": "wallet_unavailable",
  "message": "Balance service is offline.",
  "code": "WALLET_500"
}"#
            .into(),
            empty_status: 200,
            empty_body: r#"{
  "balance": 0,
  "currency": "THB",
  "items": []
}"#
            .into(),
            inject_props: [("USERNAME".to_string(), "papon".to_string())]
                .into_iter()
                .collect(),
            ..MapRule::default()
        },
        MapRule {
            id: 5,
            name: "Counter increment".into(),
            folder: "Agent".into(),
            pattern: "/v1/counter".into(),
            method: RuleMethod::Post,
            status: 201,
            content_type: "application/json".into(),
            delay_ms: 8,
            body: r#"{
  "count": 1,
  "delta": 1,
  "user": "{{USERNAME}}"
}"#
            .into(),
            graphql_name: String::new(),
            enabled: true,
            hits: 1,
            active_scenario: MockScenario::Success,
            success_status: 201,
            success_body: r#"{
  "count": 1,
  "delta": 1,
  "user": "{{USERNAME}}"
}"#
            .into(),
            error_status: 429,
            error_body: r#"{
  "error": "rate_limited",
  "message": "Too many counter increments."
}"#
            .into(),
            empty_status: 200,
            empty_body: r#"{
  "count": 0,
  "items": []
}"#
            .into(),
            ..MapRule::default()
        },
        MapRule {
            id: 6,
            name: "Auth session".into(),
            folder: "Auth".into(),
            pattern: "/v1/auth*".into(),
            method: RuleMethod::Post,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 22,
            body: r#"{
  "token": "{{TOKEN}}",
  "user": { "phone": "{{phone}}", "otp_verified": true },
  "tier": "Gold"
}"#
            .into(),
            graphql_name: String::new(),
            enabled: true,
            hits: 0,
            active_scenario: MockScenario::Success,
            success_status: 200,
            success_body: r#"{
  "token": "{{TOKEN}}",
  "user": { "phone": "{{phone}}", "otp_verified": true },
  "tier": "Gold"
}"#
            .into(),
            error_status: 401,
            error_body: r#"{
  "error": "invalid_otp",
  "message": "The one-time code has expired."
}"#
            .into(),
            empty_status: 200,
            empty_body: r#"{
  "token": "",
  "user": {}
}"#
            .into(),
            ..MapRule::default()
        },
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BpStage {
    Request,
    Response,
}

impl BpStage {
    pub fn label(self) -> &'static str {
        match self {
            BpStage::Request => "Request",
            BpStage::Response => "Response",
        }
    }
}

#[derive(Clone)]
pub struct BreakpointRule {
    pub id: u64,
    pub name: String,
    pub pattern: String,
    pub graphql_name: String,
    pub stage: BpStage,
    pub enabled: bool,
    pub hits: u32,
}

impl BreakpointRule {
    pub fn matches(&self, _method: &str, url: &str, body: &str) -> bool {
        self.enabled
            && pattern_matches_url(&self.pattern, url)
            && pattern_matches_graphql(&self.graphql_name, body)
    }
}

pub fn sample_breakpoints() -> Vec<BreakpointRule> {
    vec![
        BreakpointRule {
            id: 1,
            name: "Edit checkout payload".into(),
            pattern: "*/v1/checkout*".into(),
            graphql_name: String::new(),
            stage: BpStage::Request,
            enabled: true,
            hits: 2,
        },
        BreakpointRule {
            id: 2,
            name: "Inspect GraphQL errors".into(),
            pattern: "*/graphql*".into(),
            graphql_name: String::new(),
            stage: BpStage::Response,
            enabled: false,
            hits: 0,
        },
        BreakpointRule {
            id: 3,
            name: "Auth token refresh".into(),
            pattern: "*/oauth/token*".into(),
            graphql_name: String::new(),
            stage: BpStage::Request,
            enabled: false,
            hits: 1,
        },
    ]
}

pub fn composer_preset_graphql() -> (String, String, String, String) {
    (
        "POST".into(),
        "https://api.shopwave.dev/graphql".into(),
        "content-type: application/json\nauthorization: Bearer demo_8f3c...a91".into(),
        sample_captures()[0].req_body.clone(),
    )
}

pub fn composer_preset_rest() -> (String, String, String, String) {
    (
        "GET".into(),
        "https://jsonplaceholder.typicode.com/posts/1".into(),
        "accept: application/json".into(),
        String::new(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            Level::Info => CYAN,
            Level::Warn => AMBER,
            Level::Error => RED,
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub time: String,
    pub level: Level,
    pub source: String,
    pub message: String,
}

pub fn sample_logs() -> Vec<LogEntry> {
    let rows = [
        ("12:04:38", Level::Info, "Capture", "Captured POST api.shopwave.dev/graphql in 32 ms"),
        ("12:04:37", Level::Info, "Capture", "Captured POST api.shopwave.dev/graphql in 61 ms"),
        ("12:04:36", Level::Warn, "Map Local", "Rule \"Recommendations cache\" served a cached response"),
        ("12:04:35", Level::Error, "Capture", "GET /v1/orders/8421 returned 401 Unauthorized"),
        ("12:04:35", Level::Warn, "Map Local", "Rule \"Checkout maintenance\" replaced the live 503 response"),
        ("12:04:34", Level::Info, "Capture", "Session token refreshed for shopwave.dev"),
        ("12:04:33", Level::Info, "Capture", "DELETE /v1/cart/items/9 completed with 204"),
        ("12:04:32", Level::Info, "Capture", "Search query captured: running shoe (55 ms)"),
        ("12:04:31", Level::Info, "Breakpoints", "Breakpoint rule \"Edit checkout payload\" is armed"),
        ("12:04:30", Level::Info, "System", "Demo capture session started"),
        ("12:04:30", Level::Info, "System", "Map Local engine loaded 3 rules"),
        ("12:04:30", Level::Info, "System", "Log dashboard connected to local session"),
    ];
    rows.iter()
        .map(|(time, level, source, message)| LogEntry {
            time: time.to_string(),
            level: *level,
            source: source.to_string(),
            message: message.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod rule_tests {
    use super::*;

    #[test]
    fn wildcard_matches_query_strings() {
        let url = "https://api.shopwave.dev/v1/products?limit=20&collection=running";
        assert!(pattern_matches_url("*/v1/products?*", url));
        assert!(pattern_matches_url("*/v1/products?limit=20*", url));
        assert!(pattern_matches_url("*/products?*collection=running*", url));
        assert!(pattern_matches_url("/v1/*", url));
        assert!(pattern_matches_url("*graphql*", "https://api.shopwave.dev/graphql"));
        assert!(!pattern_matches_url("*/v1/orders?*", url));
    }

    #[test]
    fn plain_pattern_is_still_contains() {
        assert!(pattern_matches_url("graphql", "https://api.shopwave.dev/graphql"));
        assert!(pattern_matches_url("", "https://api.shopwave.dev/graphql") == false);
    }

    #[test]
    fn question_mark_stays_literal_for_queries() {
        assert!(!pattern_matches_url("/v1/product?", "https://api.shopwave.dev/v1/products?x=1"));
    }

    #[test]
    fn test_domain_matches_wildcard_and_exact() {
        assert!(domain_matches("*.example.com", "api.example.com"));
        assert!(domain_matches("*.example.com", "v2.api.example.com"));
        assert!(domain_matches("*.example.com", "example.com"));
        assert!(domain_matches("*.example.com", "api.example.com:443"));
        assert!(!domain_matches("*.example.com", "notexample.com"));
        assert!(!domain_matches("*.example.com", "example.org"));

        assert!(domain_matches("api.github.com", "api.github.com"));
        assert!(domain_matches("api.github.com", "api.github.com:443"));
        assert!(!domain_matches("api.github.com", "github.com"));

        assert!(domain_matches("api.dev.local:8443", "api.dev.local:8443"));
        assert!(!domain_matches("api.dev.local:8443", "api.dev.local:443"));
        assert!(domain_matches("api.dev.local", "api.dev.local:8443"));
        assert!(domain_matches("*.internal.net:9000", "svc.internal.net:9000"));
        assert!(!domain_matches("*.internal.net:9000", "svc.internal.net:9001"));

        assert!(domain_matches("*", "google.com"));
        assert!(!domain_matches("", "google.com"));
    }

    #[test]
    fn graphql_operation_names_parse_for_raw_documents() {
        let query = graphql_info("POST", "query GetProducts($first: Int!) { products { id } }").unwrap();
        assert_eq!(query.operation, "GetProducts");
        assert_eq!(query.op_type, "query");

        let mutation = graphql_info(
            "POST",
            "mutation UpdateCart($input: CartInput!) { updateCart { cart { id } } }",
        )
        .unwrap();
        assert_eq!(mutation.operation, "UpdateCart");
        assert_eq!(mutation.op_type, "mutation");

        let shorthand = graphql_info("POST", "{ cart { id totalQuantity } }").unwrap();
        assert_eq!(shorthand.operation, "Anonymous");
        assert_eq!(shorthand.op_type, "query");
    }

    #[test]
    fn graphql_names_parse_from_json_envelopes() {
        let body = r#"{"query":"mutation PlaceOrder($input: PlaceOrderInput!) { placeOrder { order { id } } }","variables":{"input":{"cartId":"cart_9f21"}}}"#;
        let info = graphql_info("POST", body).unwrap();
        assert_eq!(info.operation, "PlaceOrder");
        assert_eq!(info.op_type, "mutation");
        assert!(info.variables.contains("cart_9f21"));
        assert!(pattern_matches_graphql("PlaceOrder", body));
        assert!(!pattern_matches_graphql("GetProducts", body));
    }

    #[test]
    fn plain_json_is_not_mistaken_for_graphql() {
        assert!(graphql_info("POST", r#"{"quantity": 2, "variantId": "var_1"}"#).is_none());
        assert!(!pattern_matches_graphql("GetProducts", r#"{"quantity": 2}"#));
    }

    #[test]
    fn json_pretty_normalizes_existing_whitespace() {
        let pretty = json_pretty("{\n  \"a\": [1,2],\n  \"b\": {\"c\": true}\n}").unwrap();
        assert_eq!(
            pretty,
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": true\n  }\n}"
        );
        assert_eq!(json_pretty("{}").unwrap(), "{}");
        assert_eq!(json_pretty("[ ]").unwrap(), "[]");
        assert!(json_pretty("not json").is_none());
        assert!(json_pretty("{ \"unfinished\": [1, 2 }").is_none());
    }

    #[test]
    fn markup_pretty_indents_tags_and_text() {
        let html = markup_pretty("<html><body><h1>Hi</h1><br></body></html>").unwrap();
        assert_eq!(
            html,
            "<html>\n  <body>\n    <h1>\n      Hi\n    </h1>\n    <br>\n  </body>\n</html>"
        );
        assert!(markup_pretty("plain text").is_none());
        assert!(markup_pretty(r#"{"a": 1}"#).is_none());
        let xml = markup_pretty(r#"<?xml version="1.0"?><root><item id="1">A</item></root>"#)
            .unwrap();
        assert_eq!(
            xml,
            "<?xml version=\"1.0\"?>\n<root>\n  <item id=\"1\">\n    A\n  </item>\n</root>"
        );
    }

    #[test]
    fn parse_curl_handles_post_headers_and_multiline_body() {
        let cmd = r#"curl -X POST 'https://api.shopwave.dev/v1/checkout?currency=THB' \
          -H 'Content-Type: application/json' \
          -H 'Authorization: Bearer tok_123' \
          --data-raw '{"cartId":"cart_99","items":[1,2]}'"#;
        let parsed = parse_curl(cmd).expect("should parse curl");
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.url, "https://api.shopwave.dev/v1/checkout?currency=THB");
        assert_eq!(parsed.headers.len(), 2);
        assert_eq!(
            parsed.headers[0],
            ("Content-Type".into(), "application/json".into())
        );
        assert_eq!(
            parsed.headers[1],
            ("Authorization".into(), "Bearer tok_123".into())
        );
        assert_eq!(parsed.body, r#"{"cartId":"cart_99","items":[1,2]}"#);
    }

    #[test]
    fn parse_curl_infers_post_from_data_and_accepts_direct_url() {
        let implicit_post = parse_curl(r#"curl https://api.example.com/v1/login -d '{"u":"a"}'"#)
            .expect("should parse implicit POST");
        assert_eq!(implicit_post.method, "POST");
        assert_eq!(implicit_post.url, "https://api.example.com/v1/login");
        assert_eq!(implicit_post.body, r#"{"u":"a"}"#);

        let direct = parse_curl("https://api.example.com/v1/profile").expect("direct URL");
        assert_eq!(direct.method, "GET");
        assert_eq!(direct.url, "https://api.example.com/v1/profile");
    }

    #[test]
    fn map_rule_switches_between_success_error_and_empty_preserving_payloads() {
        let mut rule = MapRule {
            name: "Orders API".into(),
            pattern: "https://api.shopwave.dev/v1/orders*".into(),
            method: RuleMethod::Get,
            status: 200,
            body: r#"{"orders":[{"id":"ord_1"}]}"#.into(),
            success_status: 200,
            success_body: r#"{"orders":[{"id":"ord_1"}]}"#.into(),
            ..MapRule::default()
        };

        rule.switch_scenario(MockScenario::Error);
        assert_eq!(rule.active_scenario, MockScenario::Error);
        assert_eq!(rule.status, 500);
        assert!(rule.body.contains("MOCK_ERROR_500"));

        rule.switch_scenario(MockScenario::Empty);
        assert_eq!(rule.active_scenario, MockScenario::Empty);
        assert_eq!(rule.status, 200);
        assert!(rule.body.contains("\"items\": []"));

        rule.switch_scenario(MockScenario::Success);
        assert_eq!(rule.active_scenario, MockScenario::Success);
        assert_eq!(rule.status, 200);
        assert_eq!(rule.body, r#"{"orders":[{"id":"ord_1"}]}"#);
    }

    #[test]
    fn extracts_domain_and_builds_fixture_path_convention() {
        assert_eq!(
            extract_domain_from_pattern("https://api.shopwave.dev/v1/orders?*"),
            Some("api.shopwave.dev".into())
        );
        assert_eq!(
            extract_domain_from_pattern("*.shopwave.dev/v1/*"),
            Some("*.shopwave.dev".into())
        );
        assert_eq!(extract_domain_from_pattern("/v1/orders"), None);

        let path = mock_fixture_path(
            RuleMethod::Get,
            "https://api.shopwave.dev/v1/orders?*",
            MockScenario::Success,
        );
        assert_eq!(path, "mocks/api.shopwave.dev/get_v1_orders/success.json");
    }

    #[test]
    fn sanitize_mock_payload_replaces_pii_with_placeholders() {
        let raw = r#"{"status":"OK","access_token":"secret_jwt_xyz","phone":"0812345678","otp":"456789","citizen_id":"1234567890123"}"#;
        let sanitized = sanitize_mock_payload(raw);
        assert!(!sanitized.contains("secret_jwt_xyz"));
        assert!(!sanitized.contains("0812345678"));
        assert!(!sanitized.contains("456789"));
        assert!(!sanitized.contains("1234567890123"));
        assert!(sanitized.contains("{{TOKEN}}"));
        assert!(sanitized.contains("{{USERNAME}}"));
        assert!(sanitized.contains("{{OTP}}"));
        assert!(sanitized.contains("1100000000000"));
    }

    #[test]
    fn render_injected_body_substitutes_placeholders_and_keys() {
        let mut rule = MapRule {
            name: "Login Mock".into(),
            folder: "Auth".into(),
            body: r#"{"status":"OK","phone":"{{USERNAME}}","otp":"{{OTP}}","tier":"basic"}"#.into(),
            ..MapRule::default()
        };
        assert_eq!(rule.folder_name(), "Auth");

        rule.inject_props.insert("USERNAME".into(), "0812345678".into());
        rule.inject_props.insert("OTP".into(), "111111".into());
        rule.inject_props.insert("tier".into(), "vip".into());

        let rendered = rule.render_injected_body();
        assert!(rendered.contains("0812345678"));
        assert!(rendered.contains("111111"));
        assert!(rendered.contains("vip"));
    }

    #[test]
    fn environment_wildcard_pattern_and_empty_scenario_derivation() {
        assert_eq!(
            make_environment_wildcard_pattern("https://api.sample.com/v1/wallet"),
            "*sample.com*/v1/wallet*"
        );
        assert_eq!(
            make_environment_wildcard_pattern("https://api-preprod.sample.com/v1/auth/login?param=1"),
            "*sample.com*/v1/auth/login*"
        );

        let body = r#"{"data":{"items":[{"id":1},{"id":2}],"total":2}}"#;
        let empty = auto_derive_empty_scenario(body);
        assert!(empty.contains(r#""items":[]"#));
    }

    #[test]
    fn contract_drift_and_session_audit_flow() {
        let mock_body = r#"{"status": "ok", "balance": 100.0, "currency": "THB"}"#;
        let real_body = r#"{"status": "ok", "wallet_balance": 100.0, "currency": "THB", "extra_meta": true}"#;

        let report = analyze_contract_drift(mock_body, real_body);
        assert!(report.is_drift_detected);
        assert_eq!(report.missing_in_live, vec!["balance".to_string()]);
        assert_eq!(report.added_in_live, vec!["wallet_balance".to_string(), "extra_meta".to_string()]);
        assert!(report.notes.iter().any(|d| d.contains("BREAKING: Missing key in live response: \"balance\"")));
        assert!(report.notes.iter().any(|d| d.contains("ADDITIVE: New key in live response: \"wallet_balance\"")));

        // Test versioned fixture paths
        let v_path = mock_versioned_fixture_path(
            "preprod",
            "1.1.0",
            RuleMethod::Get,
            "https://api.sample.com/v1/wallet/balance",
            MockScenario::Success,
        );
        assert_eq!(
            v_path,
            "mocks/preprod/v1.1.0/api.sample.com/get_v1_wallet_balance/success.json"
        );

        let default_v_path = mock_versioned_fixture_path(
            "",
            "",
            RuleMethod::Get,
            "https://api.sample.com/v1/wallet/balance",
            MockScenario::Success,
        );
        assert_eq!(
            default_v_path,
            "mocks/api.sample.com/get_v1_wallet_balance/success.json"
        );

        let mut session = TestSession::new(
            "TC_WALLET_01",
            "Wallet",
            "1.1.0",
            "1.0.0",
            "preprod",
            vec!["*.sample.com".into()],
            vec!["/v1/wallet/*".into()],
        );
        assert_eq!(session.app_version, "1.1.0");
        assert_eq!(session.baseline_version, "1.0.0");
        assert_eq!(session.env, "preprod");
        assert!(session.matches_scope("api.sample.com", "/v1/wallet/balance"));
        assert!(!session.matches_scope("google.com", "/search"));
        assert!(!session.matches_scope("api.sample.com", "/v1/auth/login"));

        session.record_call(SessionCallAudit {
            timestamp: "12:00:00".into(),
            method: "GET".into(),
            url: "https://api.sample.com/v1/wallet/balance".into(),
            status: 200,
            latency_ms: 0,
            source: "MOCK (rule: 'Wallet', scenario: 'success')".into(),
            app_version: "1.1.0".into(),
            baseline_version: "1.0.0".into(),
            env: "preprod".into(),
            is_drift_detected: false,
            drift_notes: Vec::new(),
            breaking_changes_count: 0,
            additive_changes_count: 0,
        });
        session.record_call(SessionCallAudit {
            timestamp: "12:00:01".into(),
            method: "GET".into(),
            url: "https://api.sample.com/v1/wallet/history".into(),
            status: 200,
            latency_ms: 85,
            source: "UPSTREAM (200 OK)".into(),
            app_version: "1.1.0".into(),
            baseline_version: "1.0.0".into(),
            env: "preprod".into(),
            is_drift_detected: true,
            drift_notes: report.notes,
            breaking_changes_count: report.missing_in_live.len(),
            additive_changes_count: report.added_in_live.len(),
        });

        assert_eq!(session.total_calls, 2);
        assert_eq!(session.mock_calls, 1);
        assert_eq!(session.upstream_calls, 1);
        assert_eq!(session.drift_warnings, 1);
        assert_eq!(session.breaking_drifts, 1);
        assert_eq!(session.additive_drifts, 2);
    }

    #[test]
    fn protected_system_domains_and_tunnel_capture_attributes() {
        assert!(is_builtin_protected_domain("courier.push.apple.com"));
        assert!(is_builtin_protected_domain("ocsp.digicert.com"));
        assert!(is_builtin_protected_domain("swscan.apple.com:443"));
        assert!(is_builtin_protected_domain("connectivitycheck.gstatic.com"));
        assert!(!is_builtin_protected_domain("api.trueid.net"));
        assert!(!is_builtin_protected_domain("api-preprod.trueid.net:443"));

        let tunnel_capture = Capture {
            id: 1,
            method: "CONNECT".to_string(),
            url: "https://ocsp.digicert.com:443".to_string(),
            status: 200,
            latency_ms: 12,
            size_bytes: 840,
            kind: detect_kind("CONNECT", "https://ocsp.digicert.com:443", ""),
            started: "10:00:00".to_string(),
            remote: "ocsp.digicert.com:443 via 127.0.0.1".to_string(),
            protocol: "TCP Tunnel (Raw)".to_string(),
            req_headers: vec![],
            req_body: String::new(),
            resp_headers: vec![],
            resp_body: "Raw bytes".to_string(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        };

        assert_eq!(tunnel_capture.kind, ApiKind::Tunnel);
        assert!(tunnel_capture.is_tunnel());
        assert!(tunnel_capture.is_system_protected());
        assert!(tunnel_capture.is_live());

        let app_capture = Capture {
            id: 2,
            method: "CONNECT".to_string(),
            url: "https://api-preprod.trueid.net:443".to_string(),
            status: 200,
            latency_ms: 25,
            size_bytes: 1200,
            kind: detect_kind("CONNECT", "https://api-preprod.trueid.net:443", ""),
            started: "10:00:01".to_string(),
            remote: "api-preprod.trueid.net:443 via 127.0.0.1".to_string(),
            protocol: "TCP Tunnel (Raw)".to_string(),
            req_headers: vec![],
            req_body: String::new(),
            resp_headers: vec![],
            resp_body: "Raw bytes".to_string(),
            graphql: None,
            mock: None,
            pinned: false,
            aborted: false,
        };

        assert!(app_capture.is_tunnel());
        assert!(!app_capture.is_system_protected());
    }
}
