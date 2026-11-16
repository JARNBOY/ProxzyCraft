import assert from 'node:assert/strict';
import { spawn, type ChildProcess } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { ProxzyClient, type SessionSummary } from '../src/index.ts';

interface CapturedRequest {
  method: string;
  url: string;
  headers: http.IncomingHttpHeaders;
  body: string;
}

async function runUnitTests(): Promise<void> {
  const client = new ProxzyClient({ baseUrl: 'http://127.0.0.1:9090' });
  assert.equal(client.baseUrl, 'http://127.0.0.1:9090');
  assert.equal(typeof client.isHealthy, 'function');
  assert.equal(typeof client.health, 'function');
  assert.equal(typeof client.mock, 'function');
  assert.equal(typeof client.setScenario, 'function');
  assert.equal(typeof client.setFolderScenario, 'function');
  assert.equal(typeof client.inject, 'function');
  assert.equal(typeof client.injectUser, 'function');
  assert.equal(typeof client.enableAutoRecord, 'function');
  assert.equal(typeof client.disableAutoRecord, 'function');
  assert.equal(typeof client.expectHits, 'function');
  assert.equal(typeof client.reset, 'function');
  assert.equal(typeof client.startSession, 'function');
  assert.equal(typeof client.getSessionSummary, 'function');
  assert.equal(typeof client.endSession, 'function');

  // 1. Refuse sending controlToken to non-loopback targets unless explicitly opted in.
  const blockedRemote = new ProxzyClient({
    baseUrl: 'http://192.168.1.20:9090',
    controlToken: 'proxzy_test_secret',
  });
  await assert.rejects(
    () => blockedRemote.health(),
    /Refusing to send X-Proxzy-Control to non-loopback host/,
  );

  // 2. Verify X-Proxzy-Control delivery, reset(true/false), mock payload serialization, and session API shapes.
  const captured: CapturedRequest[] = [];
  const sampleSummary: SessionSummary = {
    ok: true,
    active: true,
    name: 'CheckoutRegression',
    folder: 'Checkout',
    app_version: '1.0.0',
    baseline_version: '0.9.0',
    env: 'staging',
    started_at: '2026-11-16T00:00:00Z',
    total_calls: 2,
    mock_calls: 2,
    upstream_calls: 0,
    auto_recorded_calls: 0,
    drift_warnings: 0,
    breaking_drifts: 0,
    additive_drifts: 0,
    calls: [
      {
        timestamp: '2026-11-16T00:00:01Z',
        method: 'GET',
        url: 'http://practice.proxzy.gateway/v1/items',
        status: 200,
        latency_ms: 42,
        source: "MOCK (rule: 'Items', scenario: 'success')",
        is_drift_detected: false,
        drift_notes: [],
      },
    ],
  };

  const server = http.createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on('data', (chunk: Buffer) => chunks.push(chunk));
    req.on('end', () => {
      const body = Buffer.concat(chunks).toString('utf8');
      captured.push({
        method: req.method ?? 'GET',
        url: req.url ?? '/',
        headers: req.headers,
        body,
      });

      res.writeHead(200, { 'content-type': 'application/json' });
      if (req.url === '/__proxzy/health') {
        res.end(
          JSON.stringify({
            ok: true,
            engine: 'ProxzyCraft',
            recording: true,
            auto_record: false,
            auto_record_domains: [],
            map_enabled: true,
            ssl_proxying_enabled: true,
            ssl_allowlist: [],
            rules_count: 0,
            rules: [],
          }),
        );
        return;
      }
      if (req.url === '/__proxzy/session/summary') {
        res.end(JSON.stringify(sampleSummary));
        return;
      }
      if (req.url === '/__proxzy/session/end') {
        res.end(JSON.stringify({ ...sampleSummary, active: undefined, session_ended: true }));
        return;
      }
      res.end(JSON.stringify({ ok: true }));
    });
  });

  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const address = server.address();
    assert.ok(address && typeof address === 'object');
    const authed = new ProxzyClient({
      baseUrl: `http://127.0.0.1:${address.port}`,
      controlToken: 'proxzy_test_secret',
    });

    const health = await authed.health();
    assert.equal(health.ok, true);
    assert.equal(captured.at(-1)?.headers['x-proxzy-control'], 'proxzy_test_secret');

    assert.equal(await authed.reset(), true);
    assert.equal(captured.at(-1)?.url, '/__proxzy/reset');
    assert.deepEqual(JSON.parse(captured.at(-1)!.body), { clear_rules: false });

    assert.equal(await authed.reset(true), true);
    assert.equal(captured.at(-1)?.url, '/__proxzy/reset');
    assert.deepEqual(JSON.parse(captured.at(-1)!.body), { clear_rules: true });

    const mockOk = await authed.mock({
      curl: "curl -X POST 'https://api.example.test/v1/checkout?env=qa'",
      delay_ms: 125,
      error_status: 503,
      error_body: { error: 'service_unavailable', retry: true },
    });
    assert.equal(mockOk, true);
    const mockReq = captured.at(-1)!;
    assert.equal(mockReq.url, '/__proxzy/mock');
    const mockPayload = JSON.parse(mockReq.body) as Record<string, unknown>;
    assert.equal(mockPayload.curl, "curl -X POST 'https://api.example.test/v1/checkout?env=qa'");
    assert.equal(mockPayload.delay_ms, 125);
    assert.equal(mockPayload.error_status, 503);
    assert.equal(mockPayload.error_body, JSON.stringify({ error: 'service_unavailable', retry: true }));

    assert.equal(
      await authed.startSession({
        name: 'CheckoutRegression',
        folder: 'Checkout',
        app_version: '1.0.0',
        baseline_version: '0.9.0',
        env: 'staging',
        domains: ['practice.proxzy.gateway'],
        paths: ['/v1/items'],
      }),
      true,
    );
    const startPayload = JSON.parse(captured.at(-1)!.body) as Record<string, unknown>;
    assert.equal(startPayload.name, 'CheckoutRegression');
    assert.equal(startPayload.folder, 'Checkout');
    assert.equal(startPayload.app_version, '1.0.0');
    assert.equal(startPayload.baseline_version, '0.9.0');
    assert.equal(startPayload.env, 'staging');
    assert.deepEqual(startPayload.domains, ['practice.proxzy.gateway']);
    assert.deepEqual(startPayload.paths, ['/v1/items']);

    const summary = await authed.getSessionSummary();
    assert.equal(summary.ok, true);
    assert.equal(summary.active, true);
    assert.equal(summary.total_calls, 2);
    assert.equal(summary.calls.length, 1);

    const ended = await authed.endSession();
    assert.equal(ended.ok, true);
    assert.equal(ended.session_ended, true);
    assert.equal(ended.mock_calls, 2);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }

  console.log('Unit: ProxzyClient methods, payloads, session shapes, and token guardrails verified.');
}

async function getFreePort(): Promise<number> {
  return await new Promise<number>((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on('error', reject);
    srv.listen(0, '127.0.0.1', () => {
      const addr = srv.address();
      if (!addr || typeof addr === 'string') {
        srv.close(() => reject(new Error('Failed to obtain ephemeral port')));
        return;
      }
      const { port } = addr;
      srv.close((err) => (err ? reject(err) : resolve(port)));
    });
  });
}

async function waitForProxyReady(
  child: ChildProcess,
  port: number,
  logs: string[],
  timeoutMs = 15000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(
        `Proxzy headless process exited prematurely (code=${child.exitCode}). Logs:\n${logs.join('')}`,
      );
    }
    try {
      const res = await fetch(`http://127.0.0.1:${port}/__proxzy/health`);
      if (res.ok) {
        return;
      }
    } catch {
      // Listener not bound yet.
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`Timed out waiting for headless Proxzy on 127.0.0.1:${port}. Logs:\n${logs.join('')}`);
}

async function stopChild(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.killed) {
    return;
  }
  await new Promise<void>((resolve) => {
    const forceTimer = setTimeout(() => {
      if (child.exitCode === null) {
        child.kill('SIGKILL');
      }
    }, 3000);
    child.once('exit', () => {
      clearTimeout(forceTimer);
      resolve();
    });
    child.kill('SIGTERM');
  });
}

function spawnHeadless(binPath: string, args: string[], homeDir: string): {
  child: ChildProcess;
  logs: string[];
} {
  const logs: string[] = [];
  const child = spawn(binPath, args, {
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      HOME: homeDir,
      USERPROFILE: homeDir,
    },
  });
  child.stdout?.on('data', (d: Buffer) => logs.push(d.toString('utf8')));
  child.stderr?.on('data', (d: Buffer) => logs.push(d.toString('utf8')));
  return { child, logs };
}

async function proxyHttpGet(
  proxyPort: number,
  targetUrl: string,
): Promise<{
  status: number;
  headers: http.IncomingHttpHeaders;
  body: string;
  elapsedMs: number;
}> {
  const parsed = new URL(targetUrl);
  const started = Date.now();
  return await new Promise((resolve, reject) => {
    const req = http.request(
      {
        host: '127.0.0.1',
        port: proxyPort,
        method: 'GET',
        path: targetUrl,
        headers: { Host: parsed.host, Connection: 'close' },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (c: Buffer) => chunks.push(c));
        res.on('end', () => {
          resolve({
            status: res.statusCode ?? 0,
            headers: res.headers,
            body: Buffer.concat(chunks).toString('utf8'),
            elapsedMs: Date.now() - started,
          });
        });
      },
    );
    req.on('error', reject);
    req.end();
  });
}

async function runRawControlHttp(
  port: number,
  requestPath: string,
  headers: Record<string, string>,
): Promise<{ status: number; body: string }> {
  return await new Promise((resolve, reject) => {
    const req = http.request(
      {
        host: '127.0.0.1',
        port,
        method: 'GET',
        path: requestPath,
        headers: { Connection: 'close', ...headers },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (c: Buffer) => chunks.push(c));
        res.on('end', () => {
          resolve({
            status: res.statusCode ?? 0,
            body: Buffer.concat(chunks).toString('utf8'),
          });
        });
      },
    );
    req.on('error', reject);
    req.end();
  });
}

async function runLiveE2E(binPath: string): Promise<void> {
  assert.ok(fs.existsSync(binPath), `PROXZY_E2E_BIN does not exist: ${binPath}`);

  // Phase A: Loopback headless lifecycle + multi-slot mock + delay_ms + inject + session + reset
  const home1 = fs.mkdtempSync(path.join(os.tmpdir(), 'proxzy-e2e-loopback-'));
  const port1 = await getFreePort();
  const { child: child1, logs: logs1 } = spawnHeadless(
    binPath,
    ['--headless', '--port', String(port1)],
    home1,
  );
  try {
    await waitForProxyReady(child1, port1, logs1);
    const client = new ProxzyClient({ baseUrl: `http://127.0.0.1:${port1}` });

    const initialHealth = await client.health();
    assert.equal(initialHealth.ok, true);
    assert.equal(initialHealth.engine, 'ProxzyCraft');
    assert.equal(initialHealth.rules_count, 0);

    assert.equal(
      await client.startSession({
        name: 'V1 E2E Gate',
        folder: 'Regression',
        app_version: '1.0.0',
        baseline_version: '0.9.0',
        env: 'ci',
        domains: ['practice.proxzy.gateway'],
      }),
      true,
    );

    // Register multi-slot rule with delay_ms on local practice host
    const itemsPattern = 'http://practice.proxzy.gateway/v1/items';
    assert.equal(
      await client.mock({
        name: 'Items E2E Mock',
        pattern: itemsPattern,
        folder: 'Regression',
        method: 'GET',
        delay_ms: 45,
        success_status: 200,
        success_body: { ok: true, member: '{{MEMBER_NAME}}', items: ['alpha', 'beta'] },
        error_status: 502,
        error_body: { ok: false, error: 'upstream_timeout' },
        empty_status: 200,
        empty_body: { ok: true, items: [] },
      }),
      true,
    );

    // Register second rule via curl parsing
    assert.equal(
      await client.mock({
        curl: "curl -X POST 'https://curl-e2e.example.test/v1/orders?env=qa'",
        folder: 'Orders',
        success_status: 201,
        success_body: { created: true },
      }),
      true,
    );

    const afterMock = await client.health();
    assert.equal(afterMock.rules_count, 2);
    assert.ok(afterMock.ssl_allowlist.includes('curl-e2e.example.test'));

    // Dynamic inject + live proxy traffic verification (success slot + delay_ms)
    assert.equal(
      await client.inject({
        pattern: itemsPattern,
        inject: { MEMBER_NAME: 'AdaLovelace' },
      }),
      true,
    );

    const successResp = await proxyHttpGet(port1, itemsPattern);
    assert.equal(successResp.status, 200);
    assert.ok(
      successResp.elapsedMs >= 35,
      `Expected delay_ms=45 to take >=35ms, took ${successResp.elapsedMs}ms`,
    );
    assert.equal(successResp.headers['x-proxzy-rule'], 'Items E2E Mock');
    const successJson = JSON.parse(successResp.body) as {
      ok: boolean;
      member: string;
      items: string[];
    };
    assert.equal(successJson.ok, true);
    assert.equal(successJson.member, 'AdaLovelace');
    assert.deepEqual(successJson.items, ['alpha', 'beta']);

    assert.equal(await client.expectHits(itemsPattern, 1), 1);

    // Switch scenario: error -> empty -> success -> passthrough
    assert.equal(await client.setScenario(itemsPattern, 'error'), true);
    const errorResp = await proxyHttpGet(port1, itemsPattern);
    assert.equal(errorResp.status, 502);
    assert.deepEqual(JSON.parse(errorResp.body), { ok: false, error: 'upstream_timeout' });

    assert.equal(await client.setScenario(itemsPattern, 'empty'), true);
    const emptyResp = await proxyHttpGet(port1, itemsPattern);
    assert.equal(emptyResp.status, 200);
    assert.deepEqual(JSON.parse(emptyResp.body), { ok: true, items: [] });

    assert.equal(await client.setScenario(itemsPattern, 'success'), true);
    const restoredResp = await proxyHttpGet(port1, itemsPattern);
    assert.equal(restoredResp.status, 200);

    assert.equal(await client.setScenario(itemsPattern, 'passthrough'), true);
    const passthroughHealth = await client.health();
    const itemsRule = passthroughHealth.rules.find((r) => r.pattern === itemsPattern);
    assert.ok(itemsRule);
    assert.equal(itemsRule.enabled, false);
    const passthroughResp = await proxyHttpGet(port1, itemsPattern);
    assert.equal(passthroughResp.status, 200);
    assert.equal(passthroughResp.headers['x-proxzy-rule'], undefined);

    // Session summary & end
    const summary = await client.getSessionSummary();
    assert.equal(summary.ok, true);
    assert.equal(summary.active, true);
    assert.equal(summary.name, 'V1 E2E Gate');
    assert.ok(summary.mock_calls >= 4);
    assert.ok(summary.total_calls >= 4);

    const ended = await client.endSession();
    assert.equal(ended.ok, true);
    assert.equal(ended.session_ended, true);
    assert.ok(ended.mock_calls >= 4);

    // reset(false) keeps rules, zeroes hits
    assert.equal(await client.reset(false), true);
    const afterSoftReset = await client.health();
    assert.equal(afterSoftReset.rules_count, 2);
    for (const rule of afterSoftReset.rules) {
      assert.equal(rule.hits, 0);
    }

    // reset(true) clears all rules
    assert.equal(await client.reset(true), true);
    const afterHardReset = await client.health();
    assert.equal(afterHardReset.rules_count, 0);
  } finally {
    await stopChild(child1);
    fs.rmSync(home1, { recursive: true, force: true });
  }

  // Phase B: LAN mode + explicit Control Token + browser Origin / non-local Host rejection
  const home2 = fs.mkdtempSync(path.join(os.tmpdir(), 'proxzy-e2e-lan-'));
  const port2 = await getFreePort();
  const { child: child2, logs: logs2 } = spawnHeadless(
    binPath,
    ['--headless', '--port', String(port2), '--lan', '--bind', '127.0.0.1', '--control-token', 'test_secret_v1'],
    home2,
  );
  try {
    await waitForProxyReady(child2, port2, logs2);

    const lanClient = new ProxzyClient({
      baseUrl: `http://127.0.0.1:${port2}`,
      controlToken: 'test_secret_v1',
    });
    const lanHealth = await lanClient.health();
    assert.equal(lanHealth.ok, true);
    assert.equal(
      await lanClient.mock({
        pattern: 'http://practice.proxzy.gateway/v1/lan-check',
        success_status: 200,
        success_body: { lan: true },
      }),
      true,
    );

    const tokenHealth = await runRawControlHttp(port2, '/__proxzy/health', {
      Host: `127.0.0.1:${port2}`,
      'X-Proxzy-Control': 'test_secret_v1',
    });
    assert.equal(tokenHealth.status, 200);

    // Browser Origin header must be rejected with 403 even when X-Proxzy-Control is provided
    const evilOrigin = await runRawControlHttp(port2, '/__proxzy/health', {
      Host: `127.0.0.1:${port2}`,
      Origin: 'https://evil.example',
      'X-Proxzy-Control': 'test_secret_v1',
    });
    assert.equal(evilOrigin.status, 403);

    // Non-local Host header must be rejected with 403
    const evilHost = await runRawControlHttp(port2, '/__proxzy/health', {
      Host: `evil.example:${port2}`,
      'X-Proxzy-Control': 'test_secret_v1',
    });
    assert.equal(evilHost.status, 403);
  } finally {
    await stopChild(child2);
    fs.rmSync(home2, { recursive: true, force: true });
  }

  console.log('E2E: Live headless ProxzyCraft round-trips and LAN control guardrails verified.');
}

await runUnitTests();

const e2eBin = process.env.PROXZY_E2E_BIN?.trim();
if (e2eBin) {
  await runLiveE2E(e2eBin);
} else {
  console.log('Skipping live headless E2E phase (set PROXZY_E2E_BIN to run it).');
}
