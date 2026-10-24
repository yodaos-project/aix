const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { stripVTControlCharacters } = require('node:util');
const cases = require('../../../crates/aix-web/tests/check-cases.cjs');
const cliRoot = path.resolve(__dirname, '..');
const { checkSource } = require('../dist/pkg/check.cjs');
const engine = require('../dist/pkg/aix_web.js');
// A developer's selected global types must not affect deterministic fixtures.
const testHome = mkdtempSync(path.join(tmpdir(), 'aix-check-home-'));
process.env.HOME = testHome;
process.env.USERPROFILE = testHome;
process.on('exit', () => rmSync(testHome, {recursive: true, force: true}));
assert(!require('node:fs').readFileSync(path.join(cliRoot, 'dist/pkg/check.cjs'), 'utf8').includes('/__aix__/node_modules/'),
  'The checker must not bundle an Ink declaration snapshot');

// Observe command-module loading rather than relying on machine-specific timing.
// Even a cold/slow command bundle must be preceded by visible startup progress.
const startupRoot = mkdtempSync(path.join(tmpdir(), 'aix-check-startup-'));
try {
  const input = path.join(startupRoot, 'agent');
  mkdirSync(input);
  writeFileSync(path.join(input, 'app.json'), '{"pages":[]}');
  const probe = path.join(startupRoot, 'probe.cjs');
  const marker = 'command bundle loading\n';
  writeFileSync(probe, `const Module = require('node:module');
    const load = Module._load;
    Module._load = function(request, ...args) {
      if (request === ${JSON.stringify(path.join(cliRoot, 'dist/cli-main.js'))}) {
        process.stderr.write(${JSON.stringify(marker)});
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
      }
      return load.call(this, request, ...args);
    };`);
  for (const options of [[], ['--format=text'], ['--format', 'json', '--format', 'text']]) {
    const result = spawnSync(process.execPath, ['--require', probe, path.join(cliRoot, 'dist/cli.js'), 'check', input, ...options], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert(result.stderr.startsWith(`Starting source checks\n${marker}`), result.stderr);
    assert.match(result.stderr, /\[passed\] JSON syntax and text encoding \(1 file\)/);
    assert.match(result.stderr, /\[skipped\] TypeScript project types and imports/);
    assert.match(result.stdout, /0 diagnostic\(s\)/);
  }
  for (const options of [['--format', 'json'], ['--format=json'], ['--format=text', '--format=json']]) {
    const result = spawnSync(process.execPath, ['--require', probe, path.join(cliRoot, 'dist/cli.js'), 'check', input, ...options], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stderr, marker, 'JSON checks must not print startup progress');
    assert.deepEqual(JSON.parse(result.stdout), { diagnostics: [] });
  }
  for (const help of ['--help', '-h']) {
    const result = spawnSync(process.execPath, ['--require', probe, path.join(cliRoot, 'dist/cli.js'), 'check', help], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stderr, marker, 'Help must not print startup progress');
    assert.match(result.stdout, /Statically check an AIX source directory/);
  }
  const terminalProbe = path.join(startupRoot, 'terminal.cjs');
  writeFileSync(terminalProbe, `const readline = require('node:readline');
    Object.defineProperty(process.stderr, 'isTTY', {value: true});
    process.stderr.columns = 80;
    for (const method of ['cursorTo', 'clearLine', 'moveCursor']) {
      process.stderr[method] = (...args) => readline[method](process.stderr, ...args);
    }`);
  const terminalEnv = { ...process.env, TERM: 'xterm-256color' };
  delete terminalEnv.CI;
  const terminal = spawnSync(process.execPath, ['--require', terminalProbe, path.join(cliRoot, 'dist/cli.js'), 'check', input], { encoding: 'utf8', env: terminalEnv });
  assert.equal(terminal.status, 0, terminal.stderr);
  assert(terminal.stderr.startsWith('Starting source checks\r'), JSON.stringify(terminal.stderr));
  assert.equal(terminal.stderr.split('\n').length, 8, 'Each of the seven final check items should persist on the terminal');
  assert.match(stripVTControlCharacters(terminal.stderr), /✔ JSON syntax and text encoding \(1 file\)/);
  assert.match(stripVTControlCharacters(terminal.stderr), /- TypeScript project types and imports .*skipped: no TypeScript project/);
  assert.match(terminal.stderr, /Scanning source files/);
  assert.match(terminal.stdout, /0 diagnostic\(s\)/);
  for (const options of [['--format', 'json'], ['--help']]) {
    const quiet = spawnSync(process.execPath, ['--require', terminalProbe, path.join(cliRoot, 'dist/cli.js'), 'check', input, ...options], { encoding: 'utf8', env: terminalEnv });
    assert.equal(quiet.status, 0, quiet.stderr);
    assert.equal(quiet.stderr, '', 'Terminal JSON/help output must not include progress');
  }
  writeFileSync(path.join(input, 'main.ts'), 'const count: number = "bad";');
  const failed = spawnSync(process.execPath, ['--require', terminalProbe, path.join(cliRoot, 'dist/cli.js'), 'check', input], { encoding: 'utf8', env: terminalEnv });
  assert.equal(failed.status, 1, failed.stderr);
  assert.match(stripVTControlCharacters(failed.stderr), /✖ TypeScript project types and imports \(1 root\): 1 error\n$/);
  assert.match(stripVTControlCharacters(failed.stderr), /✔ JS\/TS syntax \(1 script\)/);
  assert(!failed.stderr.includes('Checks passed'), failed.stderr);
  writeFileSync(path.join(input, 'main.ts'), 'declare function fetch(url: string): Promise<unknown>; fetch("https://example.com");');
  const warned = spawnSync(process.execPath, ['--require', terminalProbe, path.join(cliRoot, 'dist/cli.js'), 'check', input], { encoding: 'utf8', env: terminalEnv });
  assert.equal(warned.status, 0, warned.stderr);
  assert.match(stripVTControlCharacters(warned.stderr), /⚠ Permission declarations and direct API use: 1 warning/);
  assert.match(stripVTControlCharacters(warned.stderr), /✔ TypeScript project types and imports \(1 root\)/);
  const broken = spawnSync(process.execPath, ['--require', terminalProbe, path.join(cliRoot, 'dist/cli.js'), 'check', path.join(input, 'app.json')], { encoding: 'utf8', env: terminalEnv });
  assert.equal(broken.status, 1);
  assert.match(broken.stderr, /Checks failed/);
  assert.match(broken.stderr, /Input path is not a directory/);
  assert(!broken.stderr.includes('Checks passed'), broken.stderr);
  console.log('ok - startup progress precedes command loading; JSON and help stay quiet');
  console.log('ok - terminal progress replaces the startup status on the same line');
  console.log('ok - specific passed/warning/failed/skipped check items persist with the correct exit code');
} finally {
  rmSync(startupRoot, { recursive: true, force: true });
}

for (const { name, files, verify, verifyChecks } of cases) {
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
    const checks = progress.filter((event) => event.type === 'check_finished').map((event) => event.check);
    assert.equal(checks.length, 7, name);
    assert.equal(checks.reduce((count, check) => count + check.errors + check.warnings, 0), report.diagnostics.length, name);
    verifyChecks?.(checks);
    if (name === 'catches semantic errors across imports') {
      assert.deepEqual(progress.filter((event) => event.type !== 'check_finished').map((event) => event.type), ['checking_sources', 'configuring_typescript',
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

for (const { name, files, options, verify } of require('../../../crates/aix-web/tests/type-options-cases.cjs')) {
  const inputs = Object.entries(files).map(([path, source]) => ({path, data: Buffer.from(source)}));
  verify(checkSource(inputs, engine, options));
  console.log(`ok - ${name}`);
}
for (const path of ['../escape.d.ts', '/absolute.d.ts', 'C:/escape.d.ts', 'node_modules/../escape.d.ts', 'main.ts']) {
  assert.throws(() => checkSource([
    {path: 'app.json', data: Buffer.from('{"pages":[]}')},
    {path: 'main.ts', data: Buffer.from('export {};')},
  ], engine, {typeFiles: [{path, source: ''}]}), /Invalid type file path/);
}
