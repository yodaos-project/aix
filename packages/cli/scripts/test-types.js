const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { spawn } = require('node:child_process');
const { createHash } = require('node:crypto');
const tar = require('tar');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aix-types-'));
const home = path.join(root, 'home');
const cli = path.resolve(__dirname, '../dist/cli.js');
const packageName = '@yodaos-pkg/ink-env';

async function main() {
  const archives = {};
  const versions = {};
  for (const version of ['1.0.0', '2.0.0', '3.0.0']) {
    const cwd = path.join(root, version);
    fs.mkdirSync(path.join(cwd, 'package'), {recursive: true});
    fs.writeFileSync(path.join(cwd, 'package/package.json'), JSON.stringify({name: packageName, version, types: 'index.d.ts', scripts: {postinstall: 'exit 99'}}));
    fs.writeFileSync(path.join(cwd, 'package/index.d.ts'), `declare const selectedVersion: "${version}";`);
    const chunks = [];
    for await (const chunk of tar.c({gzip: true, cwd}, ['package'])) chunks.push(chunk);
    const archive = archives[version] = Buffer.concat(chunks);
    versions[version] = {version, dist: {tarball: `https://registry.npmjs.org/test/${version}.tgz`,
      integrity: 'sha512-' + (version === '3.0.0' ? 'invalid' : createHash('sha512').update(archive).digest('base64'))}};
  }
  let requests = 0;
  const server = http.createServer((request, response) => {
    requests++;
    const archive = archives[request.url.slice(1).replace(/\.tgz$/, '')];
    response.end(archive ?? JSON.stringify({'dist-tags': {latest: '2.0.0'}, versions}));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const probe = path.join(root, 'registry.cjs');
  fs.writeFileSync(probe, `const fetch = global.fetch;
    global.fetch = (input, options) => {
      if (process.env.TEST_TYPES_OFFLINE) throw new Error('test: offline');
      const url = new URL(input);
      const name = url.pathname.endsWith('.tgz') ? url.pathname.split('/').pop() : 'registry';
      return fetch('http://127.0.0.1:${server.address().port}/' + name, options);
    };`);
  const run = (args, offline = false) => new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ['--require', probe, cli, ...args], {
      env: {...process.env, HOME: home, USERPROFILE: home, AIX_NPM_REGISTRY: 'npm', TEST_TYPES_OFFLINE: offline ? '1' : ''},
    });
    let stdout = '', stderr = '';
    child.stdout.on('data', (data) => { stdout += data; });
    child.stderr.on('data', (data) => { stderr += data; });
    child.on('error', reject);
    child.on('close', (status) => resolve({status, stdout, stderr}));
  });
  const check = async (input, args = [], offline = false) => {
    const result = await run(['check', input, '--format', 'json', ...args], offline);
    assert.equal(result.stderr, '', result.stderr);
    return {...result, report: JSON.parse(result.stdout)};
  };
  const project = path.join(root, 'agent');
  fs.mkdirSync(project);
  fs.writeFileSync(path.join(project, 'app.json'), '{"pages":[]}');
  fs.writeFileSync(path.join(project, 'main.ts'), 'const version: "1.0.0" = selectedVersion;');
  fs.mkdirSync(path.join(home, '.aix'), {recursive: true});
  const runtimeSelection = '{"selectedVersion":"9.9.9"}\n';
  fs.writeFileSync(path.join(home, '.aix/runtime.json'), runtimeSelection);
  try {
    assert.match((await run(['types', 'current'], true)).stdout, /No global type version selected/);
    const absent = await check(project, [], true);
    assert.equal(absent.status, 1);
    assert(absent.report.diagnostics.some((d) => /TS2304/.test(d.message)));
    assert.equal(requests, 0, 'No selection must not trigger a download');
    assert.match((await run(['types', 'versions'])).stdout, /2\.0\.0 \(latest\)/);
    const selected = await run(['types', 'select', '1.0.0']);
    assert.equal(selected.status, 0, selected.stderr);
    assert.equal(selected.stdout.trim(), '1.0.0');
    assert.equal((await run(['types', 'current'], true)).stdout.trim(), '1.0.0');
    assert.equal(fs.readFileSync(path.join(home, '.aix/runtime.json'), 'utf8'), runtimeSelection);
    const beforeOffline = requests;
    assert.equal((await check(project, [], true)).status, 0, 'Selected cached declarations work offline');
    assert.equal(requests, beforeOffline);
    const installed = path.join(project, 'node_modules/@yodaos-pkg/ink-env');
    fs.mkdirSync(installed, {recursive: true});
    fs.writeFileSync(path.join(installed, 'package.json'), '{"types":"index.d.ts"}');
    fs.writeFileSync(path.join(installed, 'index.d.ts'), 'declare const selectedVersion: "project";');
    fs.writeFileSync(path.join(project, 'main.ts'), 'const version: "project" = selectedVersion;');
    assert.equal((await check(project, [], true)).status, 0, 'Project version overrides global selection');
    const overridden = await check(project, ['--types-version', '1.0.0'], true);
    assert.equal(overridden.status, 1);
    assert.equal(overridden.report.diagnostics.length, 1);
    assert.match(overridden.report.diagnostics[0].message, /TS2322/);
    fs.writeFileSync(path.join(project, 'main.ts'), 'const version: "2.0.0" = selectedVersion;');
    assert.equal((await check(project, ['--types-version', 'latest'])).status, 0, 'Explicit flag overrides installed types');
    assert.equal((await run(['types', 'current'], true)).stdout.trim(), '1.0.0', 'Per-check override must not change global selection');
    const tag = await run(['types', 'select', 'latest']);
    assert.equal(tag.status, 0, tag.stderr);
    assert.equal((await run(['types', 'current'], true)).stdout.trim(), '2.0.0', 'Persist exact version after choosing a tag');
    const invalid = await run(['types', 'select', '3.0.0']);
    assert.equal(invalid.status, 1);
    assert.match(invalid.stderr, /integrity verification failed/);
    assert.equal((await run(['types', 'current'], true)).stdout.trim(), '2.0.0', 'Failed selection preserves previous version');
    assert(!fs.existsSync(path.join(home, '.aix/types/ink-env/3.0.0.json')));
    assert.equal((await run(['types', 'reset'], true)).status, 0);
    assert.match((await run(['types', 'current'], true)).stdout, /No global type version selected/);
    console.log('ok - independent type selection, project/flag precedence, integrity, offline cache, exact tag pinning, and reset');
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}
main().catch((error) => { console.error(error); process.exitCode = 1; })
  .finally(() => fs.rmSync(root, {recursive: true, force: true}));
