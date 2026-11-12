const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { fixture, succeeds, fails } = require('./helpers/cli.cjs');

const source = '<page><text>杭州</text></page>\n<script setup lang="ts">export default { data: { count: 7 } };</script>\n';
const calls = (f) => f.calls().filter((call) => call.kind === 'adb');

test('launch-page uploads a standalone Ink snapshot and opens it full screen without installing an Agent', (t) => {
  const f = fixture(t);
  const input = f.write('counter.ink', source);
  f.configure({ adb: { responses: { prepare: { errorCode: '', ready: true, inboxPath: '/custom/inbox/' } } } });
  const output = succeeds(f.run(['launch-page', input]));
  assert.match(output, /Page launch requested/);
  assert(output.includes(`Source: ${input}`));
  assert.match(output, /request accepted; rendering is handled asynchronously/);
  assert.doesNotMatch(output, /Agent:|Path:|undefined/);
  assert.deepEqual(calls(f).map((call) => call.method), ['devices', 'prepare', 'push', 'open']);
  const upload = calls(f).find((call) => call.method === 'push');
  const open = calls(f).at(-1).request;
  assert.match(open.sourceFile, /^aix-debug-[a-f0-9]{64}\.ink$/);
  assert.deepEqual(open, { sourceFile: open.sourceFile, sourceName: open.sourceFile, target: '_blank', params: {} });
  assert.equal(upload.args.at(-1), `/custom/inbox/${open.sourceFile}`);
  assert.equal(upload.files.length, 1);
  assert.equal(upload.files[0].source, Buffer.from(source).toString('base64'));
  assert(!fs.existsSync(upload.files[0].path), 'local upload snapshot must be removed');
  assert.equal(fs.readFileSync(input, 'utf8'), source);
  assert(!fs.existsSync(path.join(f.root, '.aix')), 'single-file launch must not create an Agent identity');
  assert(!f.calls().some((call) => call.kind === 'fetch'));
});

test('single-file launch preserves card, JSON parameters, parameter files, and device selection', (t) => {
  const f = fixture(t);
  const input = f.write("demo O'Reilly.INK", source);
  const params = { city: '杭州', text: "O'Reilly $(echo test)", nested: { items: [1, 2] } };
  f.configure({ adb: { devices: 'first\tdevice\nsecond\tdevice\n' } });
  succeeds(f.run(['launch-page', input, '--card', '--params', JSON.stringify(params), '--serial', 'second']));
  const open = calls(f).at(-1);
  assert.equal(open.request.target, '_current');
  assert.deepEqual(open.request.params, params);
  assert(open.args.at(-1).includes("'\"'\"'"), 'Open JSON must preserve apostrophes through the remote shell');
  assert(calls(f).slice(1).every((call) => call.args[0] === '-s' && call.args[1] === 'second'));
  const paramsFile = f.write('params.json', { city: '上海' });
  succeeds(f.run(['launch-page', input, '--params-file', paramsFile, '-s', 'first']));
  assert.equal(calls(f).at(-1).args[1], 'first');
  assert.equal(calls(f).at(-1).request.target, '_blank');
  assert.deepEqual(calls(f).at(-1).request.params, { city: '上海' });
});

test('single-file identity survives edits and relative paths while isolating identical basenames', (t) => {
  const f = fixture(t);
  const input = f.write('one/counter.ink', source);
  succeeds(f.run(['launch-page', input]));
  const initial = calls(f).at(-1).request;
  const updated = '<page><text>Updated</text></page>';
  f.write('one/counter.ink', updated);
  succeeds(f.run(['launch-page', path.relative(f.root, input)]));
  assert.deepEqual(calls(f).at(-1).request, initial);
  assert.equal(calls(f).filter((call) => call.method === 'push').at(-1).files[0].source, Buffer.from(updated).toString('base64'));
  const other = f.write('two/counter.ink', source);
  succeeds(f.run(['launch-page', other]));
  assert.notEqual(calls(f).at(-1).request.sourceName, initial.sourceName);
  assert.notEqual(calls(f).at(-1).request.sourceFile, initial.sourceFile);
});

test('single-file validation rejects invalid input before contacting ADB', async (t) => {
  for (const [name, data, args, error] of [
    ['extra route', source, ['pages/home'], /single .ink file does not accept a page path/],
    ['empty', '', [], /Ink source must not be empty/],
    ['whitespace', ' \n\t', [], /Ink source must not be empty/],
    ['invalid UTF-8', Buffer.from([0xc3, 0x28]), [], /Ink source must be valid UTF-8/],
    ['oversized', Buffer.alloc(128 * 1024 + 1, 'a'), [], /Ink source is larger than 128 KiB/],
    ['array params', source, ['--params', '[]'], /Launch params must be a JSON object/],
    ['null params', source, ['--params', 'null'], /Launch params must be a JSON object/],
    ['conflicting params', source, ['--params', '{}', '--params-file', 'params.json'], /--params cannot be used with --params-file/],
  ]) await t.test(name, (t) => {
    const f = fixture(t);
    const input = f.write('counter.ink', data);
    fails(f.run(['launch-page', input, ...args]), error);
    assert.deepEqual(calls(f), []);
  });
});

test('single-file launch accepts the exact 128 KiB boundary', (t) => {
  const f = fixture(t);
  const input = f.write('limit.ink', source + ' '.repeat(128 * 1024 - Buffer.byteLength(source)));
  succeeds(f.run(['launch-page', input]));
  assert.equal(calls(f).find((call) => call.method === 'push').files[0].bytes, 128 * 1024);
});

test('launch-page rejects source symlinks while preserving symlinked project directories', { skip: process.platform === 'win32' }, (t) => {
  const f = fixture(t);
  const original = f.write('original.ink', source);
  const link = path.join(f.root, 'linked.ink');
  fs.symlinkSync(original, link);
  fails(f.run(['launch-page', link]), /Ink source must be a regular file/);
  assert.deepEqual(calls(f), []);
  const projectLink = path.join(f.root, 'project.ink');
  fs.symlinkSync(f.project(), projectLink, 'dir');
  succeeds(f.run(['launch-page', projectLink]));
  assert.deepEqual(calls(f).map((call) => call.method), ['devices', 'open']);
});

test('a project directory ending in .ink still opens its installed Agent', (t) => {
  const f = fixture(t);
  const project = path.join(f.root, 'project.ink');
  fs.renameSync(f.project(), project);
  succeeds(f.run(['launch-page', project]));
  assert.deepEqual(calls(f).map((call) => call.method), ['devices', 'open']);
  assert.equal(calls(f).at(-1).request.path, 'pages/home');
  assert.equal(calls(f).at(-1).request.target, '_blank');
  assert.match(calls(f).at(-1).request.agentId, /^develop\.rokid\.agent\./);
});

test('single-file launch stops on device errors and removes local upload snapshots', async (t) => {
  for (const [name, adb, error, expected] of [
    ['dev disabled', { responses: { prepare: { errorCode: 'ENTRYPOINT_DISABLED' } } }, /aix device set-dev/, ['devices', 'prepare']],
    ['prepare not ready', { responses: { prepare: { errorCode: '', ready: false } } }, /Prepare did not report ready=true/, ['devices', 'prepare']],
    ['prepare missing code', { responses: { prepare: { ready: true } } }, /prepare failed \(unknown error\)/, ['devices', 'prepare']],
    ['push failure', { fail: 'push' }, /simulated ADB failure/, ['devices', 'prepare', 'push']],
    ['mode changed', { responses: { open: { errorCode: 'ENTRYPOINT_DISABLED' } } }, /aix device set-dev/, ['devices', 'prepare', 'push', 'open']],
    ['Open failed', { responses: { open: { errorCode: 'OPEN_FAILED', message: 'no display' } } }, /OPEN_FAILED.*no display/, ['devices', 'prepare', 'push', 'open']],
    ['Open not accepted', { responses: { open: { errorCode: '', ok: false } } }, /open did not report ok=true/, ['devices', 'prepare', 'push', 'open']],
    ['Open missing code', { responses: { open: { ok: true } } }, /open failed \(unknown error\)/, ['devices', 'prepare', 'push', 'open']],
  ]) await t.test(name, (t) => {
    const f = fixture(t);
    const input = f.write('counter.ink', source);
    f.configure({ adb });
    fails(f.run(['launch-page', input]), error);
    assert.deepEqual(calls(f).map((call) => call.method), expected);
    for (const upload of calls(f).filter((call) => call.method === 'push')) {
      assert(!fs.existsSync(upload.files[0].path), 'failed launches must remove their local upload snapshot');
    }
  });
});
