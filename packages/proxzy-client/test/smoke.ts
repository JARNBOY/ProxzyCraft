import assert from 'node:assert/strict';
import http from 'node:http';
import { ProxzyClient } from '../src/index.ts';

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

// 2. Verify X-Proxzy-Control header delivery against a local HTTP server.
let seenToken: string | string[] | undefined;
const server = http.createServer((req, res) => {
  seenToken = req.headers['x-proxzy-control'];
  res.writeHead(200, { 'content-type': 'application/json' });
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
  assert.equal(seenToken, 'proxzy_test_secret');
} finally {
  await new Promise<void>((resolve) => server.close(() => resolve()));
}

console.log('All ProxzyClient methods and control-token guardrails verified.');
