export type MockScenario = 'success' | 'error' | 'empty' | 'passthrough';

export interface ProxzyRule {
  id: number;
  name: string;
  pattern: string;
  method: string;
  status: number;
  enabled: boolean;
  active_scenario: 'success' | 'error' | 'empty';
  hits: number;
  folder?: string;
}

export interface ProxzyHealth {
  ok: boolean;
  engine: string;
  recording: boolean;
  auto_record: boolean;
  auto_record_domains: string[];
  map_enabled: boolean;
  ssl_proxying_enabled: boolean;
  ssl_allowlist: string[];
  rules_count: number;
  rules: ProxzyRule[];
}

export interface MockOptions {
  name?: string;
  pattern?: string;
  url?: string;
  curl?: string;
  method?: string;
  scenario?: 'success' | 'error' | 'empty';
  status?: number;
  body?: string | Record<string, unknown>;
  folder?: string;
  success_status?: number;
  success_body?: string | Record<string, unknown>;
  error_status?: number;
  error_body?: string | Record<string, unknown>;
  empty_status?: number;
  empty_body?: string | Record<string, unknown>;
  delay_ms?: number;
}

export interface InjectOptions {
  pattern?: string;
  folder?: string;
  inject: Record<string, string | number | boolean>;
}

export interface SessionCallAudit {
  timestamp: string;
  method: string;
  url: string;
  status: number;
  latency_ms: number;
  source: string;
  is_drift_detected: boolean;
  drift_notes: string[];
}

export interface SessionSummary {
  ok: boolean;
  active?: boolean;
  session_started?: boolean;
  session_ended?: boolean;
  name: string;
  folder: string;
  started_at?: string;
  total_calls: number;
  mock_calls: number;
  upstream_calls: number;
  auto_recorded_calls: number;
  drift_warnings: number;
  calls: SessionCallAudit[];
}

export interface SessionStartOptions {
  name: string;
  folder?: string;
  domains?: string[];
  paths?: string[];
}

export interface ProxzyClientOptions {
  baseUrl?: string;
  timeoutMs?: number;
}

/**
 * Universal ProxzyCraft Client
 *
 * Provides a zero-dependency, type-safe API to operate the local ProxzyCraft
 * MITM proxy engine (127.0.0.1:9090) from any E2E UI testing framework
 * (MobileWright, mobile-mcp, Playwright, Appium, Maestro, XCTest, Espresso).
 */
export class ProxzyClient {
  readonly baseUrl: string;
  readonly timeoutMs: number;

  constructor(options: ProxzyClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? 'http://127.0.0.1:9090').replace(/\/$/, '');
    this.timeoutMs = options.timeoutMs ?? 5000;
  }

  /**
   * Check if ProxzyCraft engine is reachable and responsive.
   */
  async isHealthy(): Promise<boolean> {
    try {
      const res = await this.health();
      return res.ok === true;
    } catch {
      return false;
    }
  }

  /**
   * Fetch current engine health, active rules, hit counts, and SSL allowlist.
   */
  async health(): Promise<ProxzyHealth> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/health`);
    if (!res.ok) {
      throw new Error(`Proxzy health check failed with status ${res.status}`);
    }
    return (await res.json()) as ProxzyHealth;
  }

  /**
   * Enable live Auto-Catch mode for trusted domains.
   *
   * When enabled, live 200..=299 upstream responses for matching domains
   * are automatically sanitized (stripping PII/auth tokens) and cached
   * into Map Local mock rules. Subsequent runs use the mock offline.
   */
  async enableAutoRecord(domains: string[] = []): Promise<boolean> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/record`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: true, domains }),
    });
    return res.ok;
  }

  /**
   * Disable live Auto-Catch mode.
   */
  async disableAutoRecord(): Promise<boolean> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/record`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: false }),
    });
    return res.ok;
  }

  /**
   * Upsert a multi-scenario Map Local mock rule.
   *
   * Automatically adds the domain to SSL allowlist and enables Map Local.
   */
  async mock(options: MockOptions): Promise<boolean> {
    const payload = {
      ...options,
      pattern: options.pattern ?? options.url,
      body: typeof options.body === 'object' ? JSON.stringify(options.body) : options.body,
      success_body:
        typeof options.success_body === 'object'
          ? JSON.stringify(options.success_body)
          : options.success_body,
      error_body:
        typeof options.error_body === 'object'
          ? JSON.stringify(options.error_body)
          : options.error_body,
      empty_body:
        typeof options.empty_body === 'object'
          ? JSON.stringify(options.empty_body)
          : options.empty_body,
    };

    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/mock`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    return res.ok;
  }

  /**
   * Switch the active scenario of a mock rule or group.
   *
   * Scenarios:
   * - 'success': serves success status and payload (default 200)
   * - 'error': serves error status and payload (default 500)
   * - 'empty': serves empty state payload (e.g. empty lists, 0 balance)
   * - 'passthrough': disables mock rule so real upstream network is reached
   */
  async setScenario(
    target: string | { pattern?: string; folder?: string },
    scenario: MockScenario,
  ): Promise<boolean> {
    const body: Record<string, string> = { scenario };
    if (typeof target === 'string') {
      body.pattern = target;
    } else {
      if (target.pattern) body.pattern = target.pattern;
      if (target.folder) body.folder = target.folder;
    }

    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/scenario`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    return res.ok;
  }

  /**
   * Batch switch all mock rules in a module folder (e.g. 'Auth', 'Wallet').
   */
  async setFolderScenario(folder: string, scenario: MockScenario): Promise<boolean> {
    return this.setScenario({ folder }, scenario);
  }

  /**
   * Inject dynamic properties into mock templates on-the-fly.
   *
   * Substitutes both `{{KEY}}` tokens and matching JSON keys without
   * mutating the original disk fixtures.
   */
  async inject(options: InjectOptions): Promise<boolean> {
    const stringifiedMap: Record<string, string> = {};
    for (const [k, v] of Object.entries(options.inject)) {
      stringifiedMap[k] = String(v);
    }

    const payload = {
      pattern: options.pattern,
      folder: options.folder,
      inject: stringifiedMap,
    };

    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/inject`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    return res.ok;
  }

  /**
   * Convenience helper to inject user credentials (phone, username, otp, token, tier)
   * into a target mock rule or module folder.
   */
  async injectUser(
    target: { pattern?: string; folder?: string },
    user: Record<string, unknown>,
  ): Promise<boolean> {
    const injectData: Record<string, string | number | boolean> = {};
    for (const [key, val] of Object.entries(user)) {
      if (typeof val === 'string' || typeof val === 'number' || typeof val === 'boolean') {
        injectData[key] = val;
        injectData[key.toUpperCase()] = val;
      }
    }
    return this.inject({
      pattern: target.pattern,
      folder: target.folder,
      inject: injectData,
    });
  }

  /**
   * Get hit count for a specific URL pattern.
   */
  async getHits(pattern: string): Promise<number> {
    const state = await this.health();
    const rule = state.rules.find(
      (r) => r.pattern.includes(pattern) || pattern.includes(r.pattern),
    );
    return rule ? rule.hits : 0;
  }

  /**
   * Wait for a mock rule to reach an expected hit count.
   * Useful for verifying that mobile UI actions actually triggered network requests.
   */
  async expectHits(
    pattern: string,
    expectedCount: number,
    timeoutMs: number = 5000,
  ): Promise<number> {
    const startTime = Date.now();
    let currentHits = 0;

    while (Date.now() - startTime < timeoutMs) {
      currentHits = await this.getHits(pattern);
      if (currentHits >= expectedCount) {
        return currentHits;
      }
      await new Promise((resolve) => setTimeout(resolve, 150));
    }

    throw new Error(
      `Mock rule "${pattern}" did not reach ${expectedCount} hits within ${timeoutMs}ms (current: ${currentHits})`,
    );
  }

  /**
   * Reset hit counters or clear mock rules.
   *
   * @param clearRules If true, deletes all rules; if false (default), only resets hit counters to 0.
   */
  async reset(clearRules: boolean = false): Promise<boolean> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/reset`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ clear_rules: clearRules }),
    });
    return res.ok;
  }

  /**
   * Start a scoped QA Test Session.
   *
   * Filters auto-recording to the specific testcase application domain/paths
   * and captures an execution audit trail.
   */
  async startSession(options: SessionStartOptions): Promise<boolean> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/session/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: options.name,
        folder: options.folder ?? 'General',
        domains: options.domains ?? [],
        paths: options.paths ?? [],
      }),
    });
    return res.ok;
  }

  /**
   * Retrieve the live execution audit summary of the currently active test session.
   */
  async getSessionSummary(): Promise<SessionSummary> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/session/summary`);
    if (!res.ok) {
      throw new Error(`Failed to fetch session summary: status ${res.status}`);
    }
    return (await res.json()) as SessionSummary;
  }

  /**
   * End the current test session and return the final audit summary.
   */
  async endSession(): Promise<SessionSummary> {
    const res = await this.fetchWithTimeout(`${this.baseUrl}/__proxzy/session/end`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    });
    if (!res.ok) {
      throw new Error(`Failed to end session: status ${res.status}`);
    }
    return (await res.json()) as SessionSummary;
  }

  private async fetchWithTimeout(url: string, options: RequestInit = {}): Promise<Response> {
    const controller = new AbortController();
    const id = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      return await fetch(url, {
        ...options,
        signal: controller.signal,
      });
    } finally {
      clearTimeout(id);
    }
  }
}

