const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const Module = require('node:module');
const { source } = require('./helpers/source.cjs');
const { fixture } = require('./helpers/cli.cjs');

function property(t, object, key, value) {
  const original = Object.getOwnPropertyDescriptor(object, key);
  Object.defineProperty(object, key, { configurable: true, value });
  t.after(() => original ? Object.defineProperty(object, key, original) : delete object[key]);
}

test('non-recursive watcher fallback refreshes new subdirectories, debounces changes, and closes all handles', (t) => {
  const f = fixture(t);
  const root = f.project();
  const active = [];
  const timers = new Map();
  let id = 0;
  t.mock.method(global, 'setTimeout', (fn, delay) => { timers.set(++id, { fn, delay }); return id; });
  t.mock.method(global, 'clearTimeout', (timer) => timers.delete(timer));
  t.mock.method(fs, 'watch', (directory, options, listener) => {
    if (options?.recursive) throw new Error('recursive watching unavailable');
    const handle = { directory, listener: listener ?? options, closed: false, close() { this.closed = true; } };
    active.push(handle);
    return handle;
  });
  const { startSourceWatcher } = source('preview/watcher.ts');
  let changes = 0;
  const stop = startSourceWatcher(root, () => { changes++; });
  assert.equal(active.length, 2);
  f.write('agent/new/nested/file.js', 'export const value = 1;');
  active[0].listener();
  active[0].listener();
  assert.equal(timers.size, 2, 'duplicate events must be debounced');
  const fire = (delay) => {
    const [timer, value] = [...timers].find(([, value]) => value.delay === delay);
    timers.delete(timer);
    value.fn();
  };
  fire(120);
  assert.equal(changes, 1);
  fire(160);
  assert(active.slice(0, 2).every((watcher) => watcher.closed));
  assert(active.some((watcher) => watcher.directory === path.join(root, 'new/nested')));
  active.at(-1).listener();
  const pending = [...timers.values()];
  stop();
  stop();
  assert.equal(timers.size, 0);
  for (const timer of pending) timer.fn();
  assert.equal(changes, 1, 'queued callbacks after shutdown must be ignored');
  assert(active.every((watcher) => watcher.closed));
});

test('preview utilities use each platform browser command and report launch errors', (t) => {
  const childProcess = require('node:child_process');
  const launches = [];
  const warnings = [];
  t.mock.method(childProcess, 'spawn', (command, args, options) => {
    const child = new EventEmitter();
    child.unref = () => {};
    launches.push({ command, args, options, child });
    return child;
  });
  t.mock.method(process.stderr, 'write', (text) => { warnings.push(text); return true; });
  const utils = source('preview/utils.ts');
  const platform = Object.getOwnPropertyDescriptor(process, 'platform');
  t.after(() => Object.defineProperty(process, 'platform', platform));
  for (const [value, command, args] of [
    ['darwin', 'open', ['https://example.com']],
    ['win32', 'cmd', ['/c', 'start', '', 'https://example.com']],
    ['linux', 'xdg-open', ['https://example.com']],
  ]) {
    Object.defineProperty(process, 'platform', { value });
    utils.openInBrowser('https://example.com');
    assert.equal(launches.at(-1).command, command);
    assert.deepEqual(launches.at(-1).args, args);
    assert.deepEqual(launches.at(-1).options, { stdio: 'ignore', detached: true });
  }
  launches.at(-1).child.emit('error', new Error('missing browser'));
  assert.match(warnings.join(''), /warning: unable to open browser automatically, visit https:\/\/example.com/);
  const error = new Error('message');
  assert.equal(utils.formatError(error), error.stack);
  error.stack = '';
  assert.equal(utils.formatError(error), 'message');
  assert.equal(utils.formatError('failure'), 'failure');
  assert.equal(utils.sanitizeAppId('  '), 'aix-preview');
  assert.equal(utils.sanitizeAppId(' My Agent! '), 'my-agent-');
});

test('preview directory state tolerates malformed metadata and empty version files', (t) => {
  const f = fixture(t);
  const root = f.project();
  const { buildPreviewState } = source('preview/state.ts');
  f.write('agent/app.json', '{broken');
  f.write('agent/VERSION', ' \n');
  let state = buildPreviewState(root);
  assert.equal(state.title, undefined);
  assert.equal(state.version, undefined);
  f.write('agent/app.json', { title: 'Fallback title' });
  assert.equal(buildPreviewState(root).title, 'Fallback title');
  fs.unlinkSync(path.join(root, 'app.json'));
  assert.equal(buildPreviewState(root).title, undefined);
  // Load the compatibility entry and type-only module as consumers can do.
  assert.equal(typeof source('preview.ts').cmdPreview, 'function');
  assert.deepEqual(source('preview/types.ts'), {});
});

test('runtime selection reports filesystem read and write failures', (t) => {
  const os = require('node:os');
  const f = fixture(t);
  t.mock.method(os, 'homedir', () => f.home);
  const selection = source('services/runtime-selection.ts');
  fs.mkdirSync(path.join(f.home, '.aix/runtime.json'), { recursive: true });
  assert.throws(() => selection.readRuntimeSelection(), /failed to read local runtime selection/);
  assert.throws(() => selection.saveRuntimeSelection('0.18.1'), /failed to save runtime selection/);
});

test('runtime prompt distinguishes cancellation from other rejected values', async (t) => {
  const os = require('node:os');
  const f = fixture(t);
  t.mock.method(os, 'homedir', () => f.home);
  t.mock.method(global, 'fetch', async () => new Response(JSON.stringify({
    'dist-tags': { latest: '1.0.0' }, versions: { '1.0.0': {}, '0.18.1': {} },
  })));
  const exits = [];
  const errors = [];
  t.mock.method(process, 'exit', (code) => { exits.push(code); throw new Error('exit intercepted'); });
  t.mock.method(process.stderr, 'write', (value) => { errors.push(value); return true; });
  let rejection;
  let choices;
  const load = Module._load;
  t.mock.method(Module, '_load', function(request, ...args) {
    if (request === '@inquirer/prompts') return { select: async (options) => { choices = options.choices; throw rejection; } };
    return load.call(this, request, ...args);
  });
  const { cmdRuntimeSelect } = source('commands/runtime/select.ts', ['@inquirer/prompts']);
  for (const value of ['failure', new Error('prompt failed'), Object.assign(new Error('cancel'), { name: 'ExitPromptError' })]) {
    rejection = value;
    await assert.rejects(cmdRuntimeSelect(), (error) => value.name === 'ExitPromptError' ? error.message === 'exit intercepted' : error === value);
  }
  assert.deepEqual(exits, [130]);
  assert.match(errors.join(''), /runtime selection cancelled/);
  assert.equal(choices[0].name, '1.0.0 (current default)');
  fs.mkdirSync(path.join(f.home, '.aix'), { recursive: true });
  f.write('home/.aix/runtime.json', { selectedVersion: '0.18.1' });
  rejection = new Error('prompt failed');
  await assert.rejects(cmdRuntimeSelect(), /prompt failed/);
  assert.equal(choices[1].name, '0.18.1 (selected)');
});

test('legacy packer compatibility paths produce readable packages and validate direct command arguments', async (t) => {
  const f = fixture(t);
  const project = f.project();
  const output = path.join(f.root, 'fallback.aix');
  const engine = require('../dist/pkg/aix_web.js');
  const lines = [];
  t.mock.method(process.stdout, 'write', (value) => { lines.push(value); return true; });
  t.mock.method(process.stderr, 'write', () => true);
  const load = Module._load;
  let mode;
  const invocations = [];
  t.mock.method(Module, '_load', function(request, ...args) {
    if (request === path.resolve(__dirname, '../dist/pkg/aix_web.js')) return {
      ...engine, AixSourcePackBuilderWasm: undefined,
      pack_aix_from_source_with_progress: mode === 'classic' ? undefined : (...args) => {
        invocations.push('progress');
        return engine.pack_aix_from_source_with_progress(...args);
      },
      pack_aix_from_source: (...args) => {
        invocations.push('classic');
        return engine.pack_aix_from_source(...args);
      },
    };
    return load.call(this, request, ...args);
  });
  for (mode of ['progress', 'classic']) {
    // A separate host instance keeps loadEngine's cache from masking fallback.
    const commands = source('commands/legacy.ts');
    await commands.cmdPack([project, '-o', output]);
    const reader = new engine.AixReaderWasm(fs.readFileSync(output));
    try { assert(reader.get_version()); } finally { reader.free(); }
  }
  assert.deepEqual(invocations, ['progress', 'classic']);
  assert.match(lines.join(''), /Invoking WASM packer/);
  assert.match(lines.join(''), /Adding file: app.json/);
  const commands = source('commands/legacy.ts');
  t.mock.method(process, 'exit', (code) => { throw new Error(`exit ${code}`); });
  await assert.rejects(commands.cmdPack([]), /exit 1/);
  assert.throws(() => commands.cmdList([]), /exit 1/);
  assert.throws(() => commands.cmdOptimize([]), /exit 1/);
  assert.throws(() => commands.cmdOptimize(['missing.aix']), /exit 1/);
  assert.throws(() => commands.cmdOptimize(['missing.aix', '-o', output, '--level', '9']), /exit 1/);
});

test('terminal package progress succeeds and propagates pack errors after stopping its spinner', async (t) => {
  const f = fixture(t);
  const project = f.project();
  const lines = [];
  property(t, process.stderr, 'isTTY', true);
  t.mock.method(process.stderr, 'write', (value) => { lines.push(value); return true; });
  t.mock.method(process.stdout, 'write', () => true);
  const commands = source('commands/legacy.ts');
  await commands.cmdPack([project, '-o', path.join(f.root, 'tty.aix')]);
  assert.match(lines.join(''), /Package created:/);
  f.write('agent/broken.json', '{broken');
  await assert.rejects(commands.cmdPack([project, '-o', path.join(f.root, 'broken.aix')]), /Invalid JSON/);
});

test('runtime registry handles rejected fetch values, HTTP errors, missing metadata, and oversized versions', async (t) => {
  const os = require('node:os');
  const f = fixture(t);
  t.mock.method(os, 'homedir', () => f.home);
  const registry = source('services/runtime-registry.ts');
  t.mock.method(global, 'fetch', async () => new Response('{"dist-tags":{"latest":"1.0.0"}}'));
  assert.equal(await registry.getCurrentRuntimeVersion(), '1.0.0');
  f.write('home/.aix/runtime.json', { selectedVersion: '0.18.1' });
  assert.equal(await registry.getCurrentRuntimeVersion(), '0.18.1');
  for (const error of ['offline', new Error(''), new Error('outer'), new Error('outer', { cause: 'not an Error' })]) {
    t.mock.method(global, 'fetch', async () => { throw error; });
    await assert.rejects(registry.listPublishedRuntimeVersions(), /failed to fetch runtime versions/);
  }
  t.mock.method(global, 'fetch', async () => new Response('{}', { status: 503 }));
  await assert.rejects(registry.listPublishedRuntimeVersions(), /HTTP 503/);
  t.mock.method(global, 'fetch', async () => new Response('{}'));
  await assert.rejects(registry.listPublishedRuntimeVersions(), /no stable runtime versions/);
  const enormous = `${'9'.repeat(400)}.0.0`;
  t.mock.method(global, 'fetch', async () => new Response(JSON.stringify({ versions: { '1.0.0': {}, '01.0.0': {}, [enormous]: {} } })));
  const versions = await registry.listPublishedRuntimeVersions();
  assert.equal(versions.length, 3);
  assert(versions.includes(enormous));
  const provider = process.env.AIX_NPM_REGISTRY;
  t.after(() => { if (provider === undefined) delete process.env.AIX_NPM_REGISTRY; else process.env.AIX_NPM_REGISTRY = provider; });
  delete process.env.AIX_NPM_REGISTRY;
  await assert.rejects(registry.getRemoteCurrentRuntimeVersion(), /failed to resolve current runtime version/);
});

test('terminal loading always stops on success and failure', async (t) => {
  const lines = [];
  property(t, process.stderr, 'isTTY', true);
  t.mock.method(process.stderr, 'write', (value) => { lines.push(value); return true; });
  const { withLoading, formatError } = source('ui/status.ts');
  assert.equal(await withLoading('loading', async () => 42), 42);
  await assert.rejects(withLoading('loading', async () => { throw new Error('failed'); }), /failed/);
  assert.match(formatError('failed'), /\u001b\[1;31m/);
});

test('check rejects Workers that exit without a report or fail after reporting', async (t) => {
  const threads = require('node:worker_threads');
  let events;
  t.mock.method(threads, 'Worker', class extends EventEmitter {
    constructor() {
      super();
      queueMicrotask(() => { for (const [name, value] of events) this.emit(name, value); });
    }
  });
  const lines = [];
  property(t, process.stderr, 'isTTY', false);
  t.mock.method(process.stderr, 'write', (value) => { lines.push(value); return true; });
  t.mock.method(process.stdout, 'write', () => true);
  const { cmdCheck } = source('commands/check.ts');
  for (events of [
    [['exit', 0]],
    [['message', { type: 'report', report: { diagnostics: [] } }], ['exit', 1]],
    [['error', new Error('Worker failed')], ['exit', 1]],
  ]) await assert.rejects(cmdCheck('ignored', 'text'), /Check worker exited|Worker failed/);
  assert.match(lines.join(''), /Checks failed/);
  events = [['message', { type: 'progress', event: { type: 'check_finished', check: { id: 'json', label: 'JSON', status: 'warning', errors: 2, warnings: 2, detail: 'details' } } }], ['message', { type: 'report', report: { diagnostics: [] } }], ['exit', 0]];
  await cmdCheck('ignored', 'text');
  assert.match(lines.join(''), /2 errors, 2 warnings.*details/);
});

test('preview servers reject unavailable listen addresses and close rejected upgrade sockets', async (t) => {
  const http = require('node:http');
  const f = fixture(t);
  let server;
  let address;
  t.mock.method(http, 'createServer', (handler) => {
    server = new EventEmitter();
    server.on('request', handler);
    server.listen = (_port, _host, ready) => { ready(); };
    server.address = () => address;
    server.close = () => {};
    return server;
  });
  const api = source('preview/server.ts');
  for (address of [null, 'socket']) await assert.rejects(api.startStaticPreviewServer('html'), /Unable to determine preview server address/);
  address = { port: 12345 };
  const preview = await api.startDevPreviewServer(f.project(), '1.0.0', { imports: {} });
  t.after(() => preview.close());
  let status;
  const response = { writeHead(code) { status = code; return this; }, end() {} };
  server.emit('request', {}, response);
  assert.equal(status, 404);
  let destroyed = false;
  server.emit('upgrade', { url: '/private' }, { destroy() { destroyed = true; } }, Buffer.alloc(0));
  assert(destroyed);
  destroyed = false;
  server.emit('upgrade', {}, { destroy() { destroyed = true; } }, Buffer.alloc(0));
  assert(destroyed);
  preview.close();
});

test('preview validates direct command arguments and renders omitted file counts', async () => {
  await assert.rejects(source('preview/index.ts').cmdPreview([]), /preview requires an input/);
  const { renderPreviewHtml } = source('preview/html.ts');
  const html = renderPreviewHtml({ mode: 'static', sourceLabel: 'agent', inkRuntimeVersion: '1.0.0', inkImportMap: { imports: {} } });
  assert.match(html, /id="preview-files">0<\/span>/);
});

test('bootstrap handles the option terminator, missing format value, and unknown terminal width', (t) => {
  const { succeeds, fails } = require('./helpers/cli.cjs');
  const f = fixture(t);
  const input = f.project({ pages: [] });
  succeeds(f.run(['check', '--', input]));
  fails(f.run(['check', input, '--format']), /argument missing|missing argument/);
  f.configure({ tty: true, columns: 0 });
  succeeds(f.run(['check', input]));
});
