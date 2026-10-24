// Local, reproducible SDK measurements; requires built Web SDK and npm CLI.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { gzipSync } = require('node:zlib');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');

const dist = path.resolve(process.env.AIX_WEB_DIST || path.join(__dirname, '../dist'));
const samples = Number(process.env.AIX_BENCH_SAMPLES || 7);
const output = process.argv[2];
const sources = [
  { path: 'app.json', data: JSON.stringify({ pages: ['home', 'details'], widgets: [{ path: 'card', family: '1x1' }] }) },
  { path: 'home.ink', data: '<script def>{"schema":{"data":{"type":"object","properties":{}}}}</script><page><view>Hello</view></page>' },
  { path: 'details.ink', data: '<script def>{"schema":{"data":{"type":"object","properties":{"count":{"type":"integer"}},"required":["count"]}}}</script><page><view style="width:320px;height:96px">{{count}}</view></page>' },
  { path: 'card.ink', data: '<script def>{"widget":{"family":"1x1"}}</script><widget><view>Clock</view></widget>' },
  { path: 'utils.js', data: Array.from({ length: 100 }, (_, i) => `export function add${i}(value) { return value + ${i}; }`).join('\n') },
  { path: 'assets/data.bin', data: 'x'.repeat(16384) },
].map(({ path, data }) => ({ path, data: [...Buffer.from(data)] }));

function summarize(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return { median: sorted[Math.floor(sorted.length / 2)], samples: values };
}

async function main() {
  assert(Number.isInteger(samples) && samples > 0);
  const engine = require('../../../packages/cli/dist/pkg/aix_web.js');
  const packed = engine.pack_aix(sources, 'wasm-benchmark', undefined, null);
  const packageBytes = [...packed.data];
  packed.free();
  const artifacts = {};
  for (const folder of ['pkg', 'pkg-reader']) {
    const filename = `${folder}/aix_web_bg.wasm`;
    if (fs.existsSync(path.join(dist, filename))) {
      const bytes = fs.readFileSync(path.join(dist, filename));
      artifacts[filename] = { bytes: bytes.length, gzipBytes: gzipSync(bytes).length };
    }
  }
  const server = http.createServer((req, res) => {
    const pathname = new URL(req.url, 'http://localhost').pathname;
    res.setHeader('Cache-Control', 'no-store');
    if (pathname === '/') { res.setHeader('Content-Type', 'text/html'); res.end('<!doctype html>'); return; }
    const filename = path.resolve(dist, '.' + pathname);
    if (!filename.startsWith(dist + path.sep) || !fs.existsSync(filename)) { res.writeHead(404).end(); return; }
    res.setHeader('Content-Type', filename.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
    res.end(fs.readFileSync(filename));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true, channel: process.env.AIX_TEST_BROWSER_CHANNEL || undefined });
    const operations = {};
    for (const operation of ['reader', 'check', 'pack']) {
      const runs = [];
      for (let i = 0; i < samples; i++) {
        const context = await browser.newContext();
        try {
          const page = await context.newPage();
          await page.goto(`http://127.0.0.1:${server.address().port}/`);
          runs.push(await page.evaluate(async ({ operation, sources, packageBytes }) => {
            const start = performance.now();
            const { AIX } = await import('./index.js');
            const importMs = performance.now() - start;
            const files = sources.map(({ path, data }) => ({ path, data: new Uint8Array(data) }));
            let reader;
            const firstStart = performance.now();
            if (operation === 'reader') reader = await AIX.From(new Uint8Array(packageBytes));
            else if (operation === 'check') {
              const report = await AIX.checkFromSource(files);
              if (report.diagnostics.length) throw new Error(JSON.stringify(report));
            } else await AIX.packFromSource(files, { buildId: 'benchmark', optimize: false });
            const firstMs = performance.now() - firstStart;
            const iterations = operation === 'reader' ? 200 : 10;
            const warmStart = performance.now();
            for (let j = 0; j < iterations; j++) {
              if (reader) { reader.resolveMetadata(); reader.getWidgets(); reader.readFile('utils.js'); }
              else if (operation === 'check') await AIX.checkFromSource(files);
              else await AIX.packFromSource(files, { buildId: 'benchmark', optimize: false });
            }
            return { importMs, firstMs, warmMs: (performance.now() - warmStart) / iterations,
              wasmRequests: performance.getEntriesByType('resource').map((entry) => new URL(entry.name).pathname).filter((name) => name.endsWith('.wasm')) };
          }, { operation, sources, packageBytes }));
        } finally { await context.close(); }
      }
      operations[operation] = {
        importMs: summarize(runs.map((run) => run.importMs)),
        firstMs: summarize(runs.map((run) => run.firstMs)),
        warmMs: summarize(runs.map((run) => run.warmMs)),
        wasmRequests: runs[0].wasmRequests,
      };
    }
    const report = { revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
      environment: { platform: process.platform, arch: process.arch, cpu: os.cpus()[0].model, node: process.version, browser: browser.version() },
      method: { samples, server: 'loopback HTTP, no-store, uncompressed', contexts: 'fresh context per sample; browser process and engine caches may be warm',
        readerWarmOperation: 'resolveMetadata + getWidgets + readFile(utils.js)', checkWarmIterations: 10, packWarmIterations: 10,
        pack: 'packFromSource, optimize:false, default JS minification', sourceFiles: sources.length, sourceBytes: sources.reduce((n, f) => n + f.data.length, 0), packageBytes: packageBytes.length },
      artifacts, operations };
    if (output) { fs.mkdirSync(path.dirname(path.resolve(output)), { recursive: true }); fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); }
    console.log(JSON.stringify(report, null, 2));
  } finally {
    await browser?.close();
    await new Promise((resolve) => server.close(resolve));
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
