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
    const report = checkSource(inputs, engine);
    const diagnostics = report.diagnostics.filter((diagnostic) => ['AIX048', 'AIX049'].includes(diagnostic.code));
    verify(diagnostics, root, report);
    const cli = spawnSync(process.execPath, [path.join(cliRoot, 'dist/cli.js'), 'check', root, '--format', 'json'], { encoding: 'utf8' });
    assert.equal(cli.status, report.diagnostics.some((d) => d.severity === 'error') ? 1 : 0, cli.stderr);
    assert.deepEqual(JSON.parse(cli.stdout), report);
    console.log(`ok - ${name}`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
console.log(`${cases.length} shared checker / CLI parity tests passed`);
