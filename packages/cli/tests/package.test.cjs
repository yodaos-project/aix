const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { fixture, succeeds, fails } = require('./helpers/cli.cjs');
const { AixReaderWasm, pack_aix } = require('../dist/pkg/aix_web.js');

function reader(file) {
  return new AixReaderWasm(new Uint8Array(fs.readFileSync(file)));
}

test('pack, list/ls, and optimize round-trip source files and engine metadata', (t) => {
  const f = fixture(t);
  const project = f.project({ engine: '^0.17.0' });
  f.write('agent/.aixignore', 'private.txt\n');
  f.write('agent/private.txt', 'must not ship');
  f.write('agent/.aix/cache.txt', 'local state');
  f.write('agent/app.js', '// remove this comment\nconst longName = 1 + 2;\n');
  f.write('agent/data.json', Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from('{"value":"测试"}', 'utf16le')]));
  const output = path.join(f.root, 'agent.aix');
  const logs = succeeds(f.run(['pack', project, '-o', output, '-O', '--opt-level', '3', '--engine', '^0.18.0', '--log-time']));
  assert.match(logs, /\[\d{2}:\d{2}:\d{2}\.\d{3}\] Scanning source files/);
  assert.match(logs, /Converted data.json to UTF-8/);
  assert.match(logs, /Optimization Summary: Total saved/);
  const archive = reader(output);
  t.after(() => archive.free());
  const names = archive.list().map((entry) => entry.name);
  assert(names.includes('VERSION'));
  assert(names.includes('pages/home.ink'));
  assert(!names.includes('private.txt'));
  assert(!names.some((name) => name.startsWith('.aix/')));
  assert.match(archive.get_version(), /^[0-9a-f-]{36}$/);
  assert.equal(archive.supports_engine('0.18.2'), true);
  assert.equal(archive.supports_engine('1.0.0'), false);
  assert.deepEqual(JSON.parse(Buffer.from(archive.read_file('data.json')).toString()), { value: '测试' });
  assert(!Buffer.from(archive.read_file('app.js')).toString().includes('remove this comment'));
  for (const command of ['list', 'ls']) {
    const listing = succeeds(f.run([command, output]));
    assert.match(listing, /Contents of .*agent.aix:/);
    for (const name of names) assert(listing.includes(`${name}:`), name);
    assert.match(listing, /compressed:/);
  }
  const optimized = path.join(f.root, 'optimized.aix');
  assert.match(succeeds(f.run(['optimize', output, '-o', optimized, '--level', '1'])), /Optimized .*saved/);
  const optimizedReader = reader(optimized);
  t.after(() => optimizedReader.free());
  assert.equal(optimizedReader.get_version(), archive.get_version());
  assert.equal(optimizedReader.get_agent_id(), archive.get_agent_id());
  assert.deepEqual(Buffer.from(optimizedReader.read_file('data.json')), Buffer.from(archive.read_file('data.json')));
});

test('pack uses its default output and prints warnings without failing', (t) => {
  const f = fixture(t);
  const project = f.project({ pages: undefined });
  const result = f.run(['pack', project]);
  assert.match(succeeds(result), /Package created: bundle.aix/);
  assert.match(result.stderr, /warning:/);
  assert(fs.existsSync(path.join(f.root, 'bundle.aix')));
});

test('package commands reject invalid options and source data without output artifacts', async (t) => {
  const f = fixture(t);
  const project = f.project();
  const output = path.join(f.root, 'rejected.aix');
  for (const [name, args, error] of [
    ['missing pack input', ['pack'], /missing required argument/],
    ['invalid pack level', ['pack', project, '-o', output, '--opt-level', '1.5'], /--opt-level must be an integer between 1 and 3/],
    ['invalid optimize level', ['optimize', 'missing.aix', '-o', output, '--level', '4'], /--level must be an integer between 1 and 3/],
    ['missing optimize output', ['optimize', 'missing.aix'], /required option/],
    ['file instead of source', ['pack', f.write('input.txt', 'text'), '-o', output], /Input path is not a directory/],
  ]) {
    await t.test(name, () => { fails(f.run(args), error); assert(!fs.existsSync(output)); });
  }
  f.write('agent/broken.json', '{');
  fails(f.run(['pack', project, '-o', output]), /broken.json/);
  assert(!fs.existsSync(output));
  fs.unlinkSync(path.join(project, 'broken.json'));
  f.write('agent/VERSION', 'reserved');
  fails(f.run(['pack', project, '-o', output]), /VERSION is generated/);
  assert(!fs.existsSync(output));
});

test('show derives stable project identity and applies definition overrides', (t) => {
  const f = fixture(t);
  const project = f.project({ permissions: ['INTERNET'] });
  const definition = JSON.parse(succeeds(f.run(['show', project])));
  assert.match(definition.agentId, /^develop\.rokid\.agent\.[0-9a-f-]+$/);
  assert.equal(definition.agentName, 'Test Agent');
  assert.equal(definition.agentDesc, 'A useful test agent.');
  assert.equal(definition.inkVersion, '>=0.17.0');
  assert.deepEqual(definition.permissions, ['INTERNET']);
  const compact = succeeds(f.run(['show', project, '--compact']));
  assert.equal(compact.trim().split('\n').length, 1);
  assert.deepEqual(JSON.parse(compact), definition);
  assert.equal(fs.readFileSync(path.join(project, '.aix/agent-id'), 'utf8').trim(), definition.agentId.slice('develop.rokid.agent.'.length));
  f.write('agent/agent.json', { agentId: 'develop.rokid.other.ignored', agentName: 'Local override' });
  assert.equal(JSON.parse(succeeds(f.run(['show', project]))).agentName, 'Local override');
  const override = f.write('override.json', { agentId: 'ignored', agentName: 'Explicit override', custom: true });
  const output = path.join(f.root, 'definition.json');
  assert.equal(succeeds(f.run(['show', project, '--definition', override, '-o', output])), '');
  assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), { agentId: definition.agentId, agentName: 'Explicit override', custom: true });
});

test('show reads packaged metadata and rejects invalid input or persisted identity', (t) => {
  const f = fixture(t);
  const project = f.project({ agentName: 'Named', agentDesc: 'Description', nativeVersion: '2.0.0', inkVersion: '^0.18.0' });
  const output = path.join(f.root, 'agent.aix');
  succeeds(f.run(['pack', project, '-o', output]));
  const archive = reader(output);
  t.after(() => archive.free());
  const definition = JSON.parse(succeeds(f.run(['show', output, '--compact'])));
  assert.equal(definition.agentId, `develop.rokid.agent.${archive.get_agent_id()}`);
  assert.equal(definition.agentDesc, 'Description');
  assert.equal(definition.nativeVersion, '2.0.0');
  assert.equal(definition.inkVersion, '^0.18.0');
  fails(f.run(['show', f.write('input.txt', 'not an archive')]), /Input must be a project directory or .aix file/);
  f.write('agent/.aix/agent-id', ' \n');
  fails(f.run(['show', project]), /agent-id is empty/);
  f.write('agent/.aix/agent-id', '!!!');
  fails(f.run(['show', project]), /cannot be normalized/);
});

test('repacking preserves project identity in the manifest while changing VERSION', (t) => {
  const f = fixture(t);
  const project = f.project();
  const first = path.join(f.root, 'first.aix');
  const second = path.join(f.root, 'renamed.aix');
  succeeds(f.run(['pack', project, '-o', first]));
  const id = fs.readFileSync(path.join(project, '.aix/agent-id'), 'utf8').trim();
  succeeds(f.run(['pack', project, '-o', second]));
  const one = reader(first);
  const two = reader(second);
  t.after(() => { one.free(); two.free(); });
  assert.notEqual(one.get_version(), two.get_version());
  assert.equal(one.get_agent_id(), id);
  assert.equal(two.get_agent_id(), id);
  const manifest = JSON.parse(Buffer.from(two.read_file('META-INF/aix/manifest.json')));
  assert.equal(manifest.agent_id, id);
  assert.equal(manifest.version, two.get_version());
  assert(!two.list().some(({ name }) => name.startsWith('.aix/')));
  for (const input of [project, first, second]) {
    assert.equal(JSON.parse(succeeds(f.run(['show', input]))).agentId, `develop.rokid.agent.${id}`);
  }
});

test('legacy artifacts keep VERSION identity after optimization', (t) => {
  const f = fixture(t);
  const result = pack_aix([{ path: 'app.json', data: Buffer.from('{"pages":[]}') }], 'legacy-build', undefined, null);
  const archive = f.write('legacy.aix', Buffer.from(result.data));
  result.free();
  const optimized = path.join(f.root, 'optimized.aix');
  succeeds(f.run(['optimize', archive, '-o', optimized]));
  for (const input of [archive, optimized]) {
    assert.equal(JSON.parse(succeeds(f.run(['show', input]))).agentId, 'develop.rokid.agent.legacy-build');
    const legacy = reader(input);
    try { assert.equal(legacy.get_agent_id(), undefined); } finally { legacy.free(); }
  }
});

test('pack reuses normalized persisted identity and rejects empty or invalid state', (t) => {
  const f = fixture(t);
  const project = f.project();
  const output = path.join(f.root, 'agent.aix');
  f.write('agent/.aix/agent-id', ' Project ID! \n');
  succeeds(f.run(['pack', project, '-o', output]));
  assert.equal(JSON.parse(succeeds(f.run(['show', output]))).agentId, 'develop.rokid.agent.project-id');
  fs.unlinkSync(output);
  for (const [id, message] of [[' \n', /agent-id is empty/], ['!!!', /cannot be normalized/]]) {
    f.write('agent/.aix/agent-id', id);
    fails(f.run(['pack', project, '-o', output]), message);
    assert(!fs.existsSync(output));
  }
});
