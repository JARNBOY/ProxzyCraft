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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ApiKind {
    Graphql,
    Rest,
    Media,
    Other,
}

impl ApiKind {
    pub fn label(self) -> &'static str {
        match self {
            ApiKind::Graphql => "GraphQL",
            ApiKind::Rest => "REST",
            ApiKind::Media => "Media",
            ApiKind::Other => "Other",
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            ApiKind::Graphql => PURPLE,
            ApiKind::Rest => Color32::from_rgb(120, 180, 255),
            ApiKind::Media => CYAN,
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
        matches!(self.protocol.as_str(), "HTTP/1.1" | "HTTPS (MITM)")
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

#[derive(Clone)]
pub struct MapRule {
    pub id: u64,
    pub name: String,
    pub pattern: String,
    pub method: RuleMethod,
    pub status: u16,
    pub content_type: String,
    pub delay_ms: u64,
    pub body: String,
    pub graphql_name: String,
    pub enabled: bool,
    pub hits: u32,
}

impl MapRule {
    pub fn matches(&self, method: &str, url: &str, body: &str) -> bool {
        self.enabled
            && self.method.matches(method)
            && pattern_matches_url(&self.pattern, url)
            && pattern_matches_graphql(&self.graphql_name, body)
    }
}

pub fn sample_map_rules() -> Vec<MapRule> {
    vec![
        MapRule {
            id: 1,
            name: "GraphQL products fixture".into(),
            pattern: "graphql".into(),
            method: RuleMethod::Any,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 18,
            body: sample_captures()[0].resp_body.clone(),
            graphql_name: "GetProducts".into(),
            enabled: true,
            hits: 12,
        },
        MapRule {
            id: 2,
            name: "Checkout maintenance".into(),
            pattern: "/v1/checkout".into(),
            method: RuleMethod::Post,
            status: 503,
            content_type: "application/json".into(),
            delay_ms: 120,
            body: r#"{
  "error": "service_unavailable",
  "message": "Checkout is in maintenance mode.",
  "code": "CHECKOUT_MAINTENANCE"
}"#
            .into(),
            graphql_name: String::new(),
            enabled: true,
            hits: 3,
        },
        MapRule {
            id: 3,
            name: "Recommendations cache".into(),
            pattern: "/v1/recommendations".into(),
            method: RuleMethod::Get,
            status: 200,
            content_type: "application/json".into(),
            delay_ms: 21,
            body: sample_captures()[5].resp_body.clone(),
            graphql_name: String::new(),
            enabled: false,
            hits: 8,
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
}
