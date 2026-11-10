const assert = require('node:assert/strict');
const fs = require('node:fs');
const https = require('node:https');
const os = require('node:os');
const { execFileSync } = require('node:child_process');
const { localProgressiveUpload } = require('./local-progressive-upload');
const { testInkProgressive } = require('./test-ink-progressive');

// The same Studio callback is exercised independently and by the Ink client.
async function studioUpload(plan, signal) {
  const send = async (url, init) => {
    const response = await fetch(url, { ...init, signal });
    if (!response.ok) throw new Error(await response.text());
    if (response.status !== 204) return response.json();
  };
  const post = (url, body) => send(url, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const objects = plan.remoteObjects.map(({ id, size, sha256 }) => ({ id, size, sha256 }));
  const { uploadId } = await post('/studio/uploads', { assetVersion: plan.assetVersion, objects });
  const session = '/studio/uploads/' + uploadId;
  for (const { id, bytes } of plan.remoteObjects) {
    await send(session + '/' + id, { method: 'PUT', body: bytes });
  }
  return post(session + '/complete');
}
const path = require('node:path');
const { chromium } = require('playwright');

async function main() {
  const dist = path.resolve(__dirname, '../dist');
  const inkDist = process.env.INK_WEB_DIST && path.resolve(process.env.INK_WEB_DIST);
  const requests = [];
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'aix-studio-upload-'));
  const key = path.join(temporary, 'key.pem');
  const cert = path.join(temporary, 'cert.pem');
  execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
    '-subj', '/CN=localhost', '-keyout', key, '-out', cert], { stdio: 'ignore' });
  const upload = localProgressiveUpload(temporary);
  const server = https.createServer({ key: fs.readFileSync(key), cert: fs.readFileSync(cert) },
    async (request, response) => {
    const origin = `https://127.0.0.1:${server.address().port}`;
    const pathname = new URL(request.url, origin).pathname;
    if (await upload.handle(request, response, pathname, origin)) return;

    requests.push(pathname);
    if (pathname === '/') {
      response.setHeader('Content-Type', 'text/html');
      response.end('<!doctype html><title>AIX Progressive Asset test</title>');
      return;
    }
    const ink = inkDist && pathname.startsWith('/ink/');
    const root = ink ? inkDist : dist;
    const file = path.resolve(root, '.' + (ink ? pathname.slice(4) : pathname));
    if (!file.startsWith(root + path.sep) || !fs.existsSync(file)) {
      response.writeHead(404);
      response.end();
      return;
    }
    response.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
    response.end(fs.readFileSync(file));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true, channel: process.env.AIX_TEST_BROWSER_CHANNEL || undefined });
    const page = await browser.newPage({ ignoreHTTPSErrors: true });
    if (process.env.INK_TEST_DEBUG) {
      page.on('console', message => console.log(message.text()));
      page.on('pageerror', error => console.error(error));
    }
    const origin = `https://127.0.0.1:${server.address().port}/`;
    await page.goto(origin);
    const tone = Array.from(fs.readFileSync(path.resolve(__dirname, '../../aix-pack/tests/data/tone-flat.m4a')));
    const video = Array.from(fs.readFileSync(path.resolve(__dirname, '../../aix-pack/tests/data/pattern-tone-flat.mp4')));
    const result = await page.evaluate(async ({ tone, video, studioUploadSource }) => {
      const { AIX } = await import('./index.js');
      const inputs = () => [
        ...Object.entries({
          'app.json': '{"pages":["pages/index"]}',
          'pages/index.ink': `<script setup>
import clip from '../assets/clip.mp4' with { type: 'video', progressive: 'prefer' };
export default { data: { source: clip } };
</script>
<page><video src="{{source}}" style="width: 96px; height: 96px;" /></page>`,
        }).map(([path, code]) => ({ path, data: new TextEncoder().encode(code) })),
        { path: 'assets/clip.mp4', data: Uint8Array.from(video) },
      ];
      const files = inputs();
      let calls = 0;
      const studioUpload = Function('return (' + studioUploadSource + ')')();
      const packed = await AIX.packFromSource(files, {
        async onProgressiveAsset(candidate, context) {
          calls += 1;
          if (candidate.kind !== 'video' || candidate.references.length !== 1) throw new Error('Unexpected candidate');
          files[2].data.fill(0);
          const [first, second] = await Promise.all([context.prepare(), context.prepare()]);
          if (first.id !== second.id || first.layout.fragments.length < 2) throw new Error('Unexpected plan');
          const originalByte = first.remoteObjects[0].bytes[0];
          first.remoteObjects[0].bytes[0] ^= 0xff;
          if (second.remoteObjects[0].bytes[0] !== originalByte) throw new Error('Plan bytes are shared with upload caller');
          return { delivery: 'progressive', planId: second.id, receipt: await studioUpload(second) };
        },
      });
      const archive = await AIX.From(packed.data);
      const paths = archive.list().map(({name}) => name);
      let badReceiptRejected = false;
      try {
        await AIX.pack(inputs(), { async onProgressiveAsset(_, context) {
          const plan = await context.prepare();
          const invalid = await studioUpload(plan);
          invalid.objects[0].sha256 = '0'.repeat(64);
          return { delivery: 'progressive', planId: plan.id, receipt: invalid };
        } });
      } catch (error) {
        badReceiptRejected = String(error).includes('length or hash mismatch');
      }
      const controller = new AbortController();
      let uploadAborted = false;
      try {
        await AIX.pack(inputs(), { signal: controller.signal, async onProgressiveAsset(_, context) {
          await context.prepare();
          controller.abort(new Error('Upload aborted by test'));
          return new Promise(() => {});
        } });
      } catch (error) {
        uploadAborted = String(error).includes('Upload aborted by test');
      }
      const media = await AIX.pack([
        { path: 'app.json', data: new TextEncoder().encode('{"pages":[]}') },
        { path: 'app.js', data: new TextEncoder().encode(`
import tone from './tone.m4a' with { type: 'audio', progressive: 'prefer' };
import movie from './movie.mp4' with { type: 'video', progressive: 'prefer' };
export default { tone, movie };
`) },
        { path: 'tone.m4a', data: Uint8Array.from(tone) },
        { path: 'movie.mp4', data: Uint8Array.from(video) },
      ], {
        async onProgressiveAsset(candidate, context) {
          if (!candidate.progressiveSupport.supported) throw new Error(candidate.progressiveSupport.reason);
          const plan = await context.prepare();
          if (!plan.layout.tracks['1'] || plan.layout.fragments.length < 2) throw new Error('Unexpected media timeline');
          const receipt = await studioUpload(plan);
          for (const object of receipt.objects) {
            const response = await fetch(object.url, { cache: 'no-store' });
            const bytes = await response.arrayBuffer();
            const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
            const hex = Array.from(hash, byte => byte.toString(16).padStart(2, '0')).join('');
            if (!response.ok || bytes.byteLength !== object.size || hex !== object.sha256) {
              throw new Error('Studio receipt does not match stored object');
            }
          }
          return { delivery: 'progressive', planId: plan.id, receipt };
        },
      });
      const embedded = await AIX.pack(inputs());
      const fallback = await AIX.From(embedded.data);
      return {
        calls, paths, report: packed.progressiveAssets,
        embedded: fallback.list().some(({name}) => name === 'assets/clip.mp4'),
        badReceiptRejected, uploadAborted, media: media.progressiveAssets.map(({ delivery }) => delivery),
      };
    }, { tone, video, studioUploadSource: studioUpload.toString() });
    assert.equal(result.calls, 1);
    assert.deepEqual(result.media, ['progressive', 'progressive']);
    assert.equal(result.report[0].delivery, 'progressive');
    assert.equal(result.report[0].retainedSourceBytes, 0);
    assert(result.paths.includes('META-INF/assets/progressive_assets/index.json'));
    assert(!result.paths.includes('assets/clip.mp4'));
    assert(result.embedded && result.badReceiptRejected && result.uploadAborted);
    assert(requests.includes('/workers/progressive-worker.js'));
    assert(upload.requests.some(([method, pathname]) => method === 'PUT' && pathname.startsWith('/studio/uploads/')));
    assert(upload.requests.some(([method, pathname]) => method === 'GET' && pathname.startsWith('/objects/')));
    console.log('Progressive Asset browser worker, Studio HTTPS upload, flat MP4/M4A, abort, and embedded fallback passed');
    if (inkDist) await testInkProgressive(page, origin, upload, studioUpload.toString(), { tone, video });
  } finally {
    await browser?.close();
    await new Promise((resolve) => server.close(resolve));
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
