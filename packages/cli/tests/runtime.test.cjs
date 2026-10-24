const test = require('node:test');
const assert = require('node:assert/strict');
const { fixture, succeeds, fails } = require('./helpers/cli.cjs');

test('runtime current resolves npm metadata and respects local selection without a request', (t) => {
  const f = fixture(t);
  assert.equal(succeeds(f.run(['runtime', 'current'])), '0.18.2\n');
  const requests = f.calls().filter((call) => call.kind === 'fetch');
  assert.deepEqual(requests, [{ kind: 'fetch', url: 'https://registry.npmjs.org/@yodaos-pkg%2Fink', headers: { Accept: 'application/json' } }]);
  f.write('home/.aix/runtime.json', { selectedVersion: ' 0.18.1 ' });
  assert.equal(succeeds(f.run(['runtime', 'current'])), '0.18.1\n');
  assert.equal(f.calls().filter((call) => call.kind === 'fetch').length, 1);
});

test('runtime versions uses the requested registry, sorts numeric stable versions, and labels selection', (t) => {
  const f = fixture(t);
  f.write('home/.aix/runtime.json', { selectedVersion: '0.18.1' });
  const output = succeeds(f.run(['runtime', 'versions'], { env: { AIX_NPM_REGISTRY: ' npmmirror ' } }));
  assert.match(output, /Runtime source: npmmirror/);
  assert.match(output, /Current: 0.18.1\nSelected: 0.18.1/);
  assert.equal(output.split('Versions:\n')[1], '- 1.0.0\n- 0.18.2\n- 0.18.1\n- 0.9.9\n');
  assert.equal(f.calls().filter((call) => call.kind === 'fetch').length, 2);
  assert(f.calls().every((call) => call.url === 'https://registry.npmmirror.com/@yodaos-pkg%2Fink'));
  assert.match(succeeds(f.run(['runtime'])), /Inspect available preview runtime versions/);
});

test('runtime ignores empty or unsupported local selections', async (t) => {
  const f = fixture(t);
  for (const value of [null, [], {}, { selectedVersion: 1 }, { selectedVersion: '  ' }]) {
    await t.test(JSON.stringify(value), () => {
      f.write('home/.aix/runtime.json', value);
      assert.equal(succeeds(f.run(['runtime', 'current'])), '0.18.2\n');
    });
  }
  f.write('home/.aix/runtime.json', '{broken');
  fails(f.run(['runtime', 'current']), /failed to read local runtime selection/);
  fails(f.run(['runtime', 'current'], { env: { AIX_NPM_REGISTRY: 'custom' } }), /failed to read local runtime selection/);
});

test('runtime reports configuration, HTTP, registry data, and network failures', async (t) => {
  const f = fixture(t);
  const cases = [
    ['invalid provider', {}, { AIX_NPM_REGISTRY: 'custom' }, ['runtime', 'versions'], /invalid AIX_NPM_REGISTRY value/],
    ['HTTP current', { httpStatus: 503 }, {}, ['runtime', 'current'], /current runtime version from npm registry \(HTTP 503\)/],
    ['missing latest', { registry: { versions: {} } }, {}, ['runtime', 'current'], /failed to resolve current runtime version/],
    ['no stable versions', { registry: { 'dist-tags': { latest: '0.18.0-rc.1' }, versions: { '0.18.0-rc.1': {} } } }, {}, ['runtime', 'versions'], /no stable runtime versions available/],
    ['network cause', { networkError: 'offline test' }, {}, ['runtime', 'current'], /failed to fetch current runtime version from npm registry: offline test/],
  ];
  for (const [name, config, env, args, error] of cases) {
    await t.test(name, () => { f.configure(config); fails(f.run(args, { env }), error); });
  }
});

test('runtime select persists a choice from the actual interactive prompt', { timeout: 15000 }, async (t) => {
  const f = fixture(t);
  f.configure({ tty: true });
  const command = f.start(['runtime', 'select']);
  await command.output(/Select a preview runtime version/);
  command.child.stdin.write('\r');
  const result = await command.done;
  succeeds(result);
  assert.equal(succeeds(f.run(['runtime', 'current'])), '1.0.0\n');
});
