const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { once } = require('node:events');
const WebSocket = require('ws');
const { fixture, succeeds, fails } = require('./helpers/cli.cjs');

function embeddedConfig(html) {
  return JSON.parse(html.match(/<script id="aix-preview-config" type="application\/json">([\s\S]*?)<\/script>/)[1]);
}

test('preview HTML escapes metadata and embeds source bytes and selected runtime', (t) => {
  const f = fixture(t);
  const title = `A & B <script>"'</script>\u2028\u2029`;
  const project = f.project({ window: { navigationBarTitleText: title } });
  f.write('agent/VERSION', 'build-id\n');
  f.write('agent/.aix/hidden.txt', 'local cache');
  f.write('agent/assets/icon.bin', Buffer.from([0, 255, 128]));
  f.write('home/.aix/runtime.json', { selectedVersion: '0.18.1' });
  const output = path.join(f.root, 'nested/preview.html');
  assert.match(succeeds(f.run(['preview', project, '--html-out', output])), /Preview HTML created:/);
  const html = fs.readFileSync(output, 'utf8');
  assert.match(html, /A &amp; B &lt;script&gt;&quot;&#39;&lt;\/script&gt;/);
  assert(!html.includes(title), 'unescaped metadata must never reach inline scripts');
  assert.match(html, /\\u003cscript\\u003e/);
  assert.match(html, /\\u2028\\u2029/);
  const config = embeddedConfig(html);
  assert.equal(config.mode, 'static');
  assert.equal(config.inkRuntimeVersion, '0.18.1');
  assert.equal(config.initialState.title, title);
  assert.equal(config.initialState.appId, 'agent');
  assert.equal(config.initialState.version, 'build-id');
  assert.equal(config.initialState.sourceKind, 'directory');
  const files = config.initialState.files;
  assert(!files.some((file) => file.path.startsWith('.aix/')));
  assert.deepEqual(Buffer.from(files.find((file) => file.path === 'assets/icon.bin').base64, 'base64'), Buffer.from([0, 255, 128]));
  const importMap = JSON.parse(html.match(/<script type="importmap">([^]*?)<\/script>/)[1]);
  assert.equal(importMap.imports['@yodaos-pkg/ink'], 'https://ga.jspm.io/npm:@yodaos-pkg/ink@0.18.1/index.js');
});

test('preview falls back from an unpublished selection and supports archives', (t) => {
  const f = fixture(t);
  const project = f.project();
  f.write('home/.aix/runtime.json', { selectedVersion: '0.0.0' });
  const archive = path.join(f.root, 'My Agent.aix');
  succeeds(f.run(['pack', project, '-o', archive]));
  const output = path.join(f.root, 'archive.html');
  succeeds(f.run(['preview', archive, '--html-out', output]));
  const config = embeddedConfig(fs.readFileSync(output, 'utf8'));
  assert.equal(config.inkRuntimeVersion, '0.18.2');
  assert.equal(config.initialState.sourceKind, 'aix-file');
  assert.equal(config.initialState.appId, 'my-agent');
  assert(config.initialState.files.some((file) => file.path === 'VERSION'));
  assert.equal(f.calls().filter((call) => call.kind === 'fetch').length, 2);
});

test('preview validates conflicting flags and output paths', async (t) => {
  const f = fixture(t);
  const project = f.project();
  for (const [args, error] of [
    [['--launch-target', 'other'], /--launch-target must be either/],
    [['--html-out', 'out.html', '--dev'], /--html-out cannot be used with --dev/],
    [['--html-out', 'out.html', '--launch'], /--launch cannot be used with --html-out/],
    [['--html-out', project], /--html-out must point to a file path/],
  ]) await t.test(args.join(' '), () => fails(f.run(['preview', project, ...args]), error));
});

test('static preview serves only its public routes and forwards the requested launch target', { timeout: 15000 }, async (t) => {
  const f = fixture(t);
  const command = f.start(['preview', f.project(), '--launch', '--launch-target', 'current']);
  const [, url] = await command.output(/Preview server running at (http:\/\/127\.0\.0\.1:\d+\/)/);
  for (const suffix of ['', '?target=current']) {
    const response = await fetch(url + suffix);
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.match(response.headers.get('content-type'), /text\/html/);
    assert.equal(embeddedConfig(await response.text()).mode, 'static');
  }
  assert.equal(await (await fetch(url + 'health')).text(), 'ok');
  assert.equal((await fetch(url + 'app.json')).status, 404);
  const launches = f.calls().filter((call) => call.kind === 'browser');
  assert.equal(launches.length, 1);
  assert.equal(launches[0].args.at(-1), `${url}?target=current`);
  const stopped = await command.stop();
  // Windows terminates processes directly instead of delivering POSIX signals.
  if (process.platform !== 'win32') assert.equal(stopped.status, 0);
  await assert.rejects(fetch(url));
});

test('dev preview broadcasts changes and retains the last valid state after archive errors', { timeout: 20000 }, async (t) => {
  const f = fixture(t);
  const source = f.project();
  const archive = path.join(f.root, 'preview.aix');
  succeeds(f.run(['pack', source, '-o', archive]));
  const valid = fs.readFileSync(archive);
  const command = f.start(['preview', archive, '--dev']);
  const [, url] = await command.output(/Preview dev server running at (http:\/\/127\.0\.0\.1:\d+\/)/);
  const html = await (await fetch(url)).text();
  assert.equal(embeddedConfig(html).mode, 'dev');
  const stateUrl = url + '__aix_preview_state';
  const initial = await (await fetch(stateUrl)).json();
  assert.equal(initial.sourceKind, 'aix-file');
  assert.equal(await (await fetch(url + 'health')).text(), 'ok');
  assert.equal((await fetch(url + 'private')).status, 404);
  const ws = new WebSocket(url.replace('http:', 'ws:') + '__aix_preview_ws');
  t.after(() => ws.terminate());
  await once(ws, 'open', { signal: AbortSignal.timeout(5000) });
  let message = once(ws, 'message', { signal: AbortSignal.timeout(5000) });
  fs.writeFileSync(archive, 'corrupt archive');
  const error = JSON.parse((await message)[0].toString());
  assert.equal(error.type, 'error');
  assert.equal(typeof error.message, 'string');
  assert(error.message.length > 0);
  assert.deepEqual(await (await fetch(stateUrl)).json(), initial);
  message = once(ws, 'message', { signal: AbortSignal.timeout(5000) });
  fs.writeFileSync(archive, valid);
  const reload = JSON.parse((await message)[0].toString());
  assert.equal(reload.type, 'reload');
  assert(reload.revision > error.revision);
  assert.deepEqual(await (await fetch(stateUrl)).json(), initial);
  const stopped = await command.stop();
  if (process.platform !== 'win32') assert.equal(stopped.status, 0);
});

test('dev preview observes new nested source files', { timeout: 15000 }, async (t) => {
  const f = fixture(t);
  const command = f.start(['preview', f.project(), '--dev', '--launch']);
  const [, url] = await command.output(/Preview dev server running at (http:\/\/127\.0\.0\.1:\d+\/)/);
  await (await fetch(url)).text();
  assert.equal(f.calls().find((call) => call.kind === 'browser').args.at(-1), `${url}?target=blank`);
  const ws = new WebSocket(url.replace('http:', 'ws:') + '__aix_preview_ws');
  t.after(() => ws.terminate());
  await once(ws, 'open', { signal: AbortSignal.timeout(5000) });
  const changed = once(ws, 'message', { signal: AbortSignal.timeout(5000) });
  f.write('agent/new/subfolder/extra.js', 'export const answer = 42;');
  assert.equal(JSON.parse((await changed)[0].toString()).type, 'reload');
  const state = await (await fetch(url + '__aix_preview_state')).json();
  const extra = state.files.find((file) => file.path === 'new/subfolder/extra.js');
  assert.equal(Buffer.from(extra.base64, 'base64').toString(), 'export const answer = 42;');
});
