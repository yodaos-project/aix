const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const os = require('node:os');
const { spawnSync } = require('node:child_process');
const { chromium } = require('playwright');
const cases = require('../tests/check-cases.cjs');

const dist = process.env.AIX_WEB_DIST || path.resolve(__dirname, '../dist');
const cli = process.env.AIX_CLI_PATH || path.resolve(__dirname, '../../../packages/cli/dist/cli.js');

async function main() {
  const cliHome = fs.mkdtempSync(path.join(os.tmpdir(), 'aix-web-cli-home-'));
  const cliEnv = {...process.env, HOME: cliHome, USERPROFILE: cliHome};
  const requests = [];
  const server = http.createServer((request, response) => {
    const pathname = new URL(request.url, 'http://localhost').pathname;
    requests.push(pathname);
    if (pathname === '/') {
      response.setHeader('Content-Type', 'text/html');
      response.end('<!doctype html><title>AIX shared checker test</title>');
    } else if (pathname === '/worker.js') {
      response.setHeader('Content-Type', 'text/javascript');
      response.end(`import { AIX } from './index.js';
        onmessage = async ({data}) => {
          const checks = [];
          try { postMessage({report: await AIX.checkFromSource(data, {
            onProgress(event) { if (event.type === 'check_finished') checks.push(event.check); }
          }), checks}); }
          catch (error) { postMessage({error: String(error)}); }
        };`);
    } else {
      const file = path.resolve(dist, '.' + pathname);
      if (!file.startsWith(dist + path.sep) || !fs.existsSync(file)) {
        response.writeHead(404); response.end(); return;
      }
      response.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
      response.end(fs.readFileSync(file));
    }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true, channel: process.env.AIX_TEST_BROWSER_CHANNEL || undefined });
    const page = await browser.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(String(error)));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.evaluate(async () => { window.AIX = (await import('./index.js')).AIX; });
    assert(!requests.includes('/check/index.js'), 'The check adapter must remain lazy before the first check');
    for (const { name, files, verify, verifyChecks } of cases) {
      const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aix-web-check-'));
      try {
        for (const [file, content] of Object.entries(files)) {
          const target = path.join(root, file);
          fs.mkdirSync(path.dirname(target), { recursive: true });
          fs.writeFileSync(target, content);
        }
        const result = spawnSync(process.execPath, [cli, 'check', root, '--format', 'json'], { encoding: 'utf8', env: cliEnv });
        assert([0, 1].includes(result.status), result.stderr);
        const expected = JSON.parse(result.stdout);
        const { reports, progress, fileProgress, workerChecks } = await page.evaluate(async (files) => {
          const sources = Object.entries(files).map(([path, source]) => ({path, data: new TextEncoder().encode(source)}));
          const browserFiles = Object.entries(files).map(([path, source]) => {
            const file = new File([source], path.split('/').pop());
            Object.defineProperty(file, 'webkitRelativePath', {value: path});
            return file;
          });
          const selectedDirectory = Object.entries(files).map(([path, source]) => {
            const file = new File([source], path.split('/').pop());
            Object.defineProperty(file, 'webkitRelativePath', {value: 'selected-agent/' + path});
            return file;
          });
          const worker = window.checkWorker ??= new Worker('/worker.js', {type: 'module'});
          const workerReport = await new Promise((resolve, reject) => {
            worker.onmessage = ({data}) => data.error ? reject(new Error(data.error)) : resolve(data);
            worker.onerror = (event) => reject(new Error(event.message));
            worker.postMessage(sources);
          });
          const progress = [];
          const fileProgress = [];
          const reports = [await window.AIX.checkFromSource(sources, {onProgress: (event) => progress.push(event)}),
            await window.AIX.checkFromFiles(browserFiles),
            await window.AIX.checkFromFiles(selectedDirectory, {onProgress: (event) => fileProgress.push(event)}), workerReport.report];
          return { reports, progress, fileProgress, workerChecks: workerReport.checks };
        }, files);
        for (const events of [progress, fileProgress]) {
          assert.deepEqual(events[0], {type: 'loading_checker'});
          assert.deepEqual(events[1], {type: 'checking_sources', fileCount: Object.keys(files).length});
          assert.deepEqual(events.at(-1), {type: 'collecting_diagnostics'});
          const checks = events.filter((event) => event.type === 'check_finished').map((event) => event.check);
          assert.deepEqual(checks, workerChecks, name);
          verifyChecks?.(checks);
        }
        for (const report of reports) {
          assert.deepEqual(report, expected, name);
          verify(report);
        }
        console.log(`ok - browser/CLI parity: ${name}`);
      } finally {
        fs.rmSync(root, { recursive: true, force: true });
      }
    }
    await page.evaluate(() => window.checkWorker?.terminate());
    assert.deepEqual(pageErrors, []);
    assert(requests.every((request) => !request.includes('node_modules')), 'Checks must not download dependencies');
    console.log(`${cases.length} browser SDK / Web Worker / CLI parity tests passed`);
  } finally {
    await browser?.close();
    await new Promise((resolve) => server.close(resolve));
    fs.rmSync(cliHome, {recursive: true, force: true});
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
