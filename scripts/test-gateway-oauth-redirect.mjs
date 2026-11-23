import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

// Execute the production Rust format-string body against a redirecting browser.
// Playwright routes only the first request in a redirect chain; the callback
// request event still fires before goto rejects because loopback has no server.
const source = await readFile(
  new URL('../crates/services/src/services/reauth/gateway_browser.rs', import.meta.url),
  'utf8'
);
const template = source.match(/r#"([\s\S]*?)"#/)[1];
const code = template
  .replaceAll('{origin}', JSON.stringify('http://localhost:3334'))
  .replaceAll('{path}', JSON.stringify('/api/mcp-auth/callback'))
  .replaceAll('{authorize}', JSON.stringify('https://provider.example/authorize'))
  .replaceAll('{email}', JSON.stringify('user@example.com'))
  .replaceAll('{{', '{')
  .replaceAll('}}', '}');
const execute = new Function('page', `return (async () => {${code}})()`);

for (const [name, url, expected] of [
  ['redirect callback survives navigation refusal', 'http://localhost:3334/api/mcp-auth/callback?state=test&code=test', 'callback'],
  ['foreign origin is ignored', 'https://other.example/api/mcp-auth/callback?state=test&code=test', 'transport'],
  ['foreign path is ignored', 'http://localhost:3334/other?state=test&code=test', 'transport'],
]) {
  test(name, async () => {
    const listeners = new Map();
    let routed = false;
    const page = {
      on(event, listener) { listeners.set(event, listener); },
      off(event, listener) {
        assert.equal(listeners.get(event), listener);
        listeners.delete(event);
      },
      async route() { routed = true; },
      async unroute() { routed = false; },
      async goto() {
        assert.equal(routed, true);
        // A redirected URL does not invoke the route handler.
        listeners.get('request')({ url: () => url });
        throw new Error('net::ERR_CONNECTION_REFUSED');
      },
    };
    const result = await execute(page);
    assert.equal(result.kind, expected);
    if (expected === 'callback') assert.equal(result.url, url);
    assert.equal(listeners.size, 0);
    assert.equal(routed, false);
  });
}
