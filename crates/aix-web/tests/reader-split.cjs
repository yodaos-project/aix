const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

module.exports = async function testReaderSplit(page, fixture, dist, cli) {
  const tree = spawnSync('cargo', ['tree', '-p', 'aiui-aix-web', '--no-default-features',
    '--target', 'wasm32-unknown-unknown', '-e', 'normal'], {
    cwd: path.resolve(__dirname, '../../..'), encoding: 'utf8',
  });
  if (tree.error) throw tree.error;
  assert.equal(tree.status, 0, tree.stderr);
  assert(!/aiui-aix-pack|jsonschema|oxc_|\bimage v/.test(tree.stdout), tree.stdout);
  const binary = new WebAssembly.Module(fs.readFileSync(path.join(dist, 'pkg-reader/aix_web_bg.wasm')));
  const names = WebAssembly.Module.exports(binary).map((entry) => entry.name);
  assert(names.includes('aixreaderwasm_new'));
  assert(!names.some((name) => /aixpackresult|aixsourcepackbuilder|pack_aix|optimize_aix|check_aix|prepare_aix_check|collect_aix_source/.test(name)), names.join('\n'));

  const engine = require(path.join(path.dirname(cli), 'pkg/aix_web.js'));
  const packed = engine.pack_aix(fixture.files, 'reader-split', undefined, null);
  const bytes = [...packed.data];
  packed.free();
  const requests = [];
  page.on('request', (request) => requests.push(new URL(request.url()).pathname));
  await page.route('**/pkg/**', (route) => route.abort());
  const readerOnly = await page.evaluate(async (bytes) => {
    const [reader, bounded] = await Promise.all([
      window.AIX.From(new Uint8Array(bytes)),
      window.AIX.From(new Uint8Array(bytes), {}),
    ]);
    window.splitReader = reader;
    if (JSON.stringify(reader.getPages()) !== JSON.stringify(bounded.getPages())) throw new Error('Concurrent reader results differ');
    if (reader.getVersion() !== 'reader-split' || reader.getTitle() !== undefined
      || !reader.supportsEngine('0.18.1') || !reader.list().some((entry) => entry.name === 'app.json')
      || !Array.isArray(JSON.parse(new TextDecoder().decode(reader.readFile('app.json'))).pages)) {
      throw new Error('Reader instance methods failed with tooling blocked');
    }
    return { pages: reader.getPages(), tools: reader.getTools(), widgets: reader.getWidgets(), metadata: reader.resolveMetadata() };
  }, bytes);
  assert.deepEqual(readerOnly, fixture.expected);
  assert(requests.includes('/pkg-reader/aix_web_bg.wasm'), requests.join('\n'));
  assert.equal(requests.filter((name) => name === '/pkg-reader/aix_web_bg.wasm').length, 1, 'Concurrent reader calls share initialization');
  assert(!requests.some((name) => name.startsWith('/pkg/') || name.startsWith('/check/')), requests.join('\n'));
  await page.unroute('**/pkg/**');

  let toolingFetches = 0;
  await page.route('**/pkg/aix_web_bg.wasm', (route) => ++toolingFetches === 1 ? route.abort() : route.continue());
  const recovered = await page.evaluate(async () => {
    const files = [
      { path: 'app.json', data: new TextEncoder().encode('{"pages":[]}') },
      { path: 'main.ts', data: new TextEncoder().encode('export const count: number = 1 + 2;') },
    ];
    let failed = false;
    try { await window.AIX.pack(files, { buildId: 'failed-init' }); }
    catch { failed = true; }
    const [packed, checked] = await Promise.all([
      window.AIX.packFromSource(files, { buildId: 'retried-init', onProgress() {} }),
      window.AIX.checkFromSource(files),
    ]);
    const optimized = await window.AIX.optimize(packed.data);
    const reader = await window.AIX.From(optimized.data);
    // The compatibility artifact still supports raw reader and tooling calls.
    const legacy = await import('./pkg/aix_web.js');
    const raw = new legacy.AixReaderWasm(optimized.data);
    try {
      return { failed, diagnostics: checked.diagnostics, version: reader.getVersion(),
        script: new TextDecoder().decode(reader.readFile('main.ts')),
        original: { pages: window.splitReader.getPages(), tools: window.splitReader.getTools(),
          widgets: window.splitReader.getWidgets(), metadata: window.splitReader.resolveMetadata() },
        legacyVersion: raw.get_version(), legacyPages: raw.get_pages() };
    } finally { raw.free(); }
  });
  assert.equal(recovered.failed, true);
  assert.equal(toolingFetches, 2, 'One failed load, then a single shared initialization for concurrent pack/check');
  assert.deepEqual(recovered.diagnostics, []);
  assert.equal(recovered.version, 'retried-init');
  assert.equal(recovered.legacyVersion, 'retried-init');
  assert.deepEqual(recovered.legacyPages, []);
  assert.equal(recovered.script, 'export const count:number=3;');
  assert.deepEqual(recovered.original, fixture.expected, 'Tooling initialization must not invalidate the existing reader memory');
  await page.unroute('**/pkg/aix_web_bg.wasm');
  console.log('ok - reader has no tooling dependencies/exports, loads with tooling blocked, and survives lazy tooling retry/concurrency');
};
