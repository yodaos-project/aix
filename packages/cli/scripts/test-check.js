const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const cases = require('../../../crates/aix-web/tests/check-cases.cjs');
const cliRoot = path.resolve(__dirname, '..');
const { checkSource } = require('../dist/pkg/check.cjs');
const engine = require('../dist/pkg/aix_web.js');

for (const { name, files, verify } of cases) {
  const root = mkdtempSync(path.join(tmpdir(), 'aix-check-'));
  try {
    for (const [file, content] of Object.entries(files)) {
      const target = path.join(root, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    const inputs = Object.entries(files).map(([file, content]) => ({ path: file, data: Buffer.from(content) }));
    const progress = [];
    const report = checkSource(inputs, engine, { onProgress: (event) => progress.push(event) });
    assert.deepEqual(progress[0], { type: 'checking_sources', fileCount: inputs.length });
    assert.deepEqual(progress.at(-1), { type: 'collecting_diagnostics' });
    if (name === 'catches semantic errors across imports') {
      assert.deepEqual(progress.map((event) => event.type), ['checking_sources', 'configuring_typescript',
        'resolving_typescript_project', 'checking_typescript', 'collecting_diagnostics']);
      assert.equal(progress.find((event) => event.type === 'checking_typescript').rootCount, 2);
    }
    const diagnostics = report.diagnostics.filter((diagnostic) => ['AIX048', 'AIX049'].includes(diagnostic.code));
    verify(diagnostics, root, report);
    const cli = spawnSync(process.execPath, [path.join(cliRoot, 'dist/cli.js'), 'check', root, '--format', 'json'], { encoding: 'utf8' });
    assert.equal(cli.status, report.diagnostics.some((d) => d.severity === 'error') ? 1 : 0, cli.stderr);
    assert.equal(cli.stderr, '', 'JSON output must not include progress logs');
    assert.deepEqual(JSON.parse(cli.stdout), report);
    if (name === 'catches semantic errors across imports') {
      const text = spawnSync(process.execPath, [path.join(cliRoot, 'dist/cli.js'), 'check', root], { encoding: 'utf8' });
      assert.equal(text.status, 1, text.stderr);
      for (const stage of ['Scanning source files', 'Loading WASM engine', 'JSON Schemas (3 files)',
        'Reading TypeScript project configuration', 'Resolving TypeScript project (2 roots)',
        'Checking TypeScript types (2 project roots)', 'Collecting diagnostics']) assert(text.stderr.includes(stage), stage);
      assert.match(text.stdout, /error \[AIX048\].*TS2322/);
      assert.match(text.stdout, /1 diagnostic\(s\)/);
      const invalid = spawnSync(process.execPath, [path.join(cliRoot, 'dist/cli.js'), 'check', path.join(root, 'app.json')], { encoding: 'utf8' });
      assert.equal(invalid.status, 1);
      assert.match(invalid.stderr, /Input path is not a directory/);
      assert.equal(invalid.stdout, '');
    }
    console.log(`ok - ${name}`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
console.log(`${cases.length} shared checker / CLI parity tests passed`);
