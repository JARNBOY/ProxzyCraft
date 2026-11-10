# @proxzy/client

Universal TypeScript client for the **ProxzyCraft** HTTP/HTTPS MITM proxy engine (`http://127.0.0.1:9090`).

Zero dependencies. Works in Node.js 18+, Bun, Deno, Playwright, and MobileWright.

---

## 🚀 Quick Start (3 Lines)

```ts
import { ProxzyClient } from '@proxzy/client';

const proxzy = new ProxzyClient();

// Switch 'Wallet' mock to Error state before tapping in mobile UI:
await proxzy.setFolderScenario('Wallet', 'error');
```

---

## 🌟 Key Capabilities

### 1. Dual-Mode Architecture

- **Mode A: Daily Dev & QA Tool (Like Proxyman GUI)**
  - Launch GUI with `cargo run`.
  - Inspect HTTP/HTTPS traffic, headers, payloads, query parameters.
  - Create 3-scenario Map Local mocks (`Success 200`, `Error 500`, `Empty 200`).
  - Auto-strip PII (tokens, passwords, phones, citizen IDs) when saving live responses.

- **Mode B: Autonomous SDLC E2E Engine (MobileWright / mobile-mcp)**
  - Launch headless daemon: `proxzy --headless` (or with `--auto-record="*.domain.com"`).
  - Mobile devices route traffic through `127.0.0.1:9090`.
  - Test suites control mocks dynamically via `/__proxzy/*` REST control plane.
  - Zero brittle test dependencies; zero cross-suite state leakage.

---

### 2. Auto-Catch Live 200 OK Responses (Solving Cold Start)

When you write UI tests for a new app without existing mock files:

```ts
// Tell ProxzyCraft to auto-catch live 200 OK responses from trusted domains:
await proxzy.enableAutoRecord(['*.example.com', 'api.trusted.com']);

// Run your test once against staging/preprod.
// ProxzyCraft automatically:
// 1. Intercepts the live 200 OK response.
// 2. Strips all Authorization tokens, passwords, and PII.
// 3. Auto-derives an empty scenario (empty arrays / zeros).
// 4. Saves the mock rule in memory & disk fixture.
// Subsequent test runs hit the in-memory mock with 0ms latency and zero network flakiness!
```

---

### 3. 3-Scenario Mock Switching

```ts
// 1. Setup mock
await proxzy.mock({
  name: 'User Profile API',
  pattern: 'https://api.example.com/v1/profile*',
  folder: 'Profile',
  scenario: 'success',
  success_status: 200,
  success_body: { name: 'Somchai', tier: 'Gold' },
  error_status: 500,
  error_body: { error: 'internal_server_error' },
  empty_status: 200,
  empty_body: { name: '', tier: 'None' },
});

// 2. Switch to Error
await proxzy.setScenario('profile', 'error');

// 3. Switch to Empty
await proxzy.setScenario('profile', 'empty');

// 4. Switch to Passthrough (real upstream network)
await proxzy.setScenario('profile', 'passthrough');
```

---

### 4. Dynamic Parameter Injection (`injectUser`)

Keep fixtures clean and generic while injecting dynamic credentials per test:

```ts
await proxzy.injectUser(
  { folder: 'Auth' },
  {
    phone: '0812345678',
    username: 'test_user_01',
    otp: '999999',
  }
);
```

---

### 5. Hit Counter Assertion (`expectHits`)

Ensure your mobile screen action actually dispatched the expected network request:

```ts
// Tap mobile button via MobileWright / mobile-mcp
await mobile.tap('#refresh-button');

// Assert ProxzyCraft intercepted exactly 1 request to wallet
await proxzy.expectHits('v1/wallet/balance', 1);
```

---

## 🛠 Integration with MobileWright / Playwright

```ts
import { test as base } from '@playwright/test';
import { ProxzyClient } from '@proxzy/client';

export const test = base.extend<{ proxzy: ProxzyClient }>({
  proxzy: async ({}, use) => {
    const client = new ProxzyClient();
    await client.reset(); // Reset hit counters before test
    await use(client);
  },
});
```

