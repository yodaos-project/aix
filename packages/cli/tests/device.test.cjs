const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { fixture, succeeds, fails } = require('./helpers/cli.cjs');

const agentId = 'develop.rokid.agent.test-agent';
const widgetPath = 'widgets/card';
const moduleAt = (startIndex, extra = {}) => ({
  name: 'custom_1', startIndex, columnSize: 2, rowSize: 1, agentId, path: widgetPath, ...extra,
});
const grid = (permanentModules = [], overlayModules = []) => ({
  totalGridCount: 4, totalColumnCount: 2, permanentModules, overlayModules,
});
const snapshot = (config, extra = {}) => ({
  errorCode: '', ok: true, environment: 'dev', ready: true,
  configurationJson: config, configurationVersion: 'layout-1', environmentEpoch: 7, ...extra,
});

function setup(t, adb = {}) {
  const f = fixture(t);
  const project = f.project({ widgets: [{ path: widgetPath, family: '1x2' }] });
  f.write('agent/.aix/agent-id', 'test-agent\n');
  f.configure({ adb: { agentId, ...adb } });
  return { ...f, project };
}
const calls = (f) => f.calls().filter((call) => call.kind === 'adb');
const layouts = (f) => calls(f).filter((call) => call.method === 'push')
  .flatMap((call) => call.files).filter((file) => file.path.endsWith('widget-config.json')).map((file) => file.json);

test('first Widget launch creates the default grid when the device has no configuration', (t) => {
  const f = setup(t);
  succeeds(f.run(['launch-widget', f.project, widgetPath]));
  const config = layouts(f)[0];
  assert.equal(config.totalGridCount, 4);
  assert.equal(config.totalColumnCount, 2);
  assert.deepEqual(config.permanentModules, []);
  assert.equal(config.overlayModules[0].agentId, agentId);
});

test('install stages a real package and derived definition, validates status, and cleans temporary files', (t) => {
  const f = setup(t);
  const output = succeeds(f.run(['install', f.project, '--serial', 'test-device', '-O', '--opt-level', '1', '--engine', '^0.18.0']));
  assert.match(output, /Agent installed: Test Agent/);
  assert.match(output, /Phone upload: confirmed/);
  assert.match(output, /Cloud indexing: not verified/);
  assert.deepEqual(calls(f).map((call) => call.method), ['devices', 'prepare', 'shell', 'push', 'apply', 'status']);
  const upload = calls(f).find((call) => call.method === 'push');
  assert.equal(upload.args.at(-1), '/mock/inbox');
  assert.equal(upload.files.length, 2);
  assert(upload.files[0].bytes > 0);
  assert.equal(upload.files[1].json.agentId, agentId);
  assert.equal(upload.files[1].json.agentName, 'Test Agent');
  for (const file of upload.files) assert(!fs.existsSync(file.path), 'installation staging must be removed');
  assert.deepEqual(calls(f).at(-1).args.slice(-2), ['--arg', 'op-1']);
});

test('install rejects failed or inconsistent device responses and cleans up after apply', async (t) => {
  for (const [name, responses, error] of [
    ['developer mode disabled', { prepare: { errorCode: 'ENTRYPOINT_DISABLED', message: 'disabled' } }, /aix device set-dev/],
    ['prepare not ready', { prepare: { errorCode: '', ready: false } }, /Prepare did not report ready=true/],
    ['wrong agent', { apply: { errorCode: '', agentId: 'other' } }, /Apply agentId does not match/],
    ['unknown outcome', { apply: { errorCode: '', agentId, outcome: 'INVALID' } }, /Unexpected Apply outcome/],
    ['missing operation', { apply: { errorCode: '', agentId, outcome: 'CREATED' } }, /Apply operationId is missing/],
    ['publish failed', { status: { errorCode: '', publishState: 'FAILED_RETRYABLE' } }, /Upload stopped with publishState=FAILED_RETRYABLE/],
    ['incompatible source', { status: { errorCode: 'ENGINE_INCOMPATIBLE', message: 'newer Ink required' } }, /Retry with a compatible engine range/],
  ]) {
    await t.test(name, (t) => {
      const f = setup(t, { responses });
      fails(f.run(['install', f.project]), error);
      for (const file of calls(f).find((call) => call.method === 'push')?.files ?? []) {
        assert(!fs.existsSync(file.path));
      }
    });
  }
});

test('archive installation reports pending upload and gives repack guidance for incompatible engines', (t) => {
  const f = setup(t);
  const archive = path.join(f.root, 'agent.aix');
  succeeds(f.run(['pack', f.project, '-o', archive]));
  const packagedId = JSON.parse(succeeds(f.run(['show', archive]))).agentId;
  f.configure({ adb: { agentId: packagedId, responses: {
    prepare: { errorCode: '', ready: true },
    status: { errorCode: '', publishState: 'PENDING' },
  } } });
  assert.match(succeeds(f.run(['install', archive])), /Phone upload: pending/);
  assert(calls(f).some((call) => call.method === 'push' && call.args.at(-1) === '/sdcard/aiui/package/.staging/adb'));
  f.configure({ adb: { agentId: packagedId, responses: { status: { errorCode: 'PROJECTION_ENGINE_INCOMPATIBLE' } } } });
  fails(f.run(['install', archive]), /already packaged[\s\S]*Repack the source project first/);
});

test('launch-page sends a declared route, card target, and safely quoted nested params', (t) => {
  const f = setup(t);
  const params = { text: "O'Reilly", nested: { value: 'a & b' }, items: [1, 2] };
  succeeds(f.run(['launch-page', f.project, '--card', '--params', JSON.stringify(params)]));
  const open = calls(f).find((call) => call.method === 'open');
  assert.deepEqual(open.request, { agentId, path: 'pages/home', target: '_current', params });
  assert(open.args.at(-1).includes("'\"'\"'"), 'apostrophes must be shell-quoted');
  const file = f.write('params.json', { input: 'from file' });
  assert.match(succeeds(f.run(['launch-page', f.project, 'pages/home', '--params-file', file])), /Page launch requested/);
  assert.deepEqual(calls(f).at(-1).request, { agentId, path: 'pages/home', target: '_blank', params: { input: 'from file' } });
});

test('launch-page rejects invalid routes and parameters before contacting ADB', async (t) => {
  const f = setup(t);
  for (const [args, error] of [
    [['missing'], /Launch path is not declared/],
    [[widgetPath], /Target blank requires a path declared in app.json pages/],
    [['--params', '[]'], /Launch params must be a JSON object/],
    [['--params', 'null'], /Launch params must be a JSON object/],
    [['--params', '{}', '--params-file', 'params.json'], /--params cannot be used with --params-file/],
  ]) await t.test(args.join(' '), () => fails(f.run(['launch-page', f.project, ...args]), error));
  f.write('agent/app.json', { pages: [] });
  fails(f.run(['launch-page', f.project]), /declares no pages/);
  assert.deepEqual(calls(f), []);
});

test('device selection rejects offline, unauthorized, ambiguous, and unknown serials', async (t) => {
  for (const [name, devices, args, error] of [
    ['offline', 'a\toffline\nb\tunauthorized\n', [], /No online, authorized ADB device found/],
    ['multiple', 'a\tdevice\nb\tdevice\n', [], /Multiple ADB devices found; pass --serial/],
    ['unknown serial', 'a\tdevice\n', ['--serial', 'b'], /ADB device is not online or authorized: b/],
  ]) await t.test(name, (t) => {
    const f = setup(t, { devices });
    fails(f.run(['device', ...args]), error);
    assert.deepEqual(calls(f).map((call) => call.method), ['devices']);
  });
});

test('device status joins installed archive sizes with manifest metadata', (t) => {
  const config = grid([{ name: 'clock', startIndex: 0, columnSize: 1, rowSize: 1, agentId, path: widgetPath }]);
  const f = setup(t, {
    responses: { 'widget-snapshot': snapshot(JSON.stringify(config)) },
    files: '/sdcard/aiui/package/b.aix|1048576\ninvalid\n/sdcard/aiui/package/a.aix|2048\n/sdcard/aiui/package/bad.aix|-1\n/sdcard/aiui/package/c.aix|12\n',
    manifest: { packages: [null, [], 1, { fileName: 'a.aix', agentName: 'Agent A', agentId, nativeVersion: '1.2.0', inkVersion: '^0.18.0' }] },
  });
  const output = succeeds(f.run(['device']));
  assert.match(output, /Developer Mode: enabled/);
  assert.match(output, /Widget Runtime: ready/);
  assert.match(output, /Environment Epoch: 7/);
  assert.match(output, /Permanent Widgets \(1\):[\s\S]*clock/);
  assert.match(output, /Installed Agents \(3\):/);
  assert.match(output, /Agent A[\s\S]*a.aix \(2.0 KiB\)[\s\S]*native 1.2.0, Ink \^0.18.0/);
  assert.match(output, /b.aix \(1.0 MiB\)/);
  assert.match(output, /c.aix \(12 B\)/);
  assert(output.indexOf('Agent A') < output.indexOf('b.aix'));
  for (const action of ['set-dev', 'unset-dev']) {
    assert.match(succeeds(f.run(['device', action])), /Developer Mode (enabled|disabled)/);
    assert.equal(calls(f).at(-1).args.at(-1), action === 'set-dev' ? 'dev' : 'prod');
  }
});

test('device status handles unconfigured production devices and command failures', (t) => {
  const f = setup(t, { responses: { 'widget-snapshot': snapshot('', { environment: 'prod', ready: false }) } });
  assert.match(succeeds(f.run(['device'])), /Developer Mode: disabled[\s\S]*not ready[\s\S]*not configured[\s\S]*Installed Agents \(0\)/);
  f.configure({ adb: { fail: 'switch' } });
  fails(f.run(['device', 'set-dev']), /failed: simulated ADB failure/);
});

test('uninstall validates all completion fields and parses raw and Bundle JSON responses', async (t) => {
  const f = setup(t, { responses: { remove: JSON.stringify({ errorCode: '', localCompleted: true, success: true, dispatched: true }) } });
  assert.match(succeeds(f.run(['uninstall', agentId])), /cloud removal is not verified/);
  for (const field of ['localCompleted', 'success', 'dispatched']) {
    await t.test(field, () => {
      f.configure({ adb: { responses: { remove: { errorCode: '', localCompleted: true, success: true, dispatched: true, [field]: false } } } });
      fails(f.run(['uninstall', f.project]), new RegExp(`remove did not report ${field}=true`));
    });
  }
  f.configure({ adb: { responses: { remove: { errorCode: '', localCompleted: true, success: true, dispatched: true, message: 'nested } "quoted" \\ text', details: { braces: '{}' } } } } });
  succeeds(f.run(['uninstall', f.project]));
  fails(f.run(['uninstall', 'develop.rokid.invalid']), /agentId must use the develop.rokid/);
});

test('malformed or failed device responses surface actionable errors', async (t) => {
  for (const [response, error] of [
    ['nothing', /Expected exactly one result_data JSON value/],
    ['result_data=[]', /result_data is not a JSON object/],
    ['result_data={} result_data={}', /Expected exactly one result_data/],
    ['result_data={"errorCode":""', /Incomplete result_data JSON/],
    [{ errorCode: 'DENIED', errorMessage: 'Permission denied' }, /remove failed \(DENIED\): Permission denied/],
  ]) await t.test(String(response), (t) => {
    const f = setup(t, { responses: { remove: response } });
    fails(f.run(['uninstall', agentId]), error);
  });
});

test('Widget placement preserves permanent modules and chooses a free row', (t) => {
  const permanent = { name: 'clock', startIndex: 0, columnSize: 1, rowSize: 1, agentId: 'permanent', path: 'clock' };
  const f = setup(t, { responses: { 'widget-snapshot': snapshot(grid([permanent])) } });
  assert.match(succeeds(f.run(['launch-widget', f.project, widgetPath])), /Widget layout applied[\s\S]*Widget launch requested/);
  assert.deepEqual(layouts(f), [grid([permanent], [moduleAt(2)])]);
  assert.deepEqual(calls(f).at(-1).request, { agentId, path: widgetPath, target: '_widget', params: {} });
  assert.deepEqual(calls(f).find((call) => call.method === 'push').args.slice(-1), ['/mock/inbox/widget-config.json']);
});

test('unchanged Widget placement is restored once when the device loses it', (t) => {
  const config = grid([], [moduleAt(0)]);
  const f = setup(t, { responses: {
    'widget-snapshot': snapshot(config),
    open: [{ errorCode: 'WIDGET_PLACEMENT_MISSING', ok: false }, { errorCode: '', ok: true }],
  } });
  assert.match(succeeds(f.run(['launch-widget', f.project, widgetPath])), /Widget placement restored/);
  assert.deepEqual(layouts(f), [config]);
  assert.equal(calls(f).filter((call) => call.method === 'open').length, 2);
  assert(calls(f).findIndex((call) => call.method === 'open') < calls(f).findIndex((call) => call.method === 'prepare'));
});

test('requested Widget position replaces overlapping overlays but never permanent modules', (t) => {
  const other = moduleAt(0, { agentId: 'other', columnSize: 1 });
  const f = setup(t, { responses: { 'widget-snapshot': snapshot(grid([], [other])) } });
  succeeds(f.run(['launch-widget', f.project, widgetPath, '--position', '0']));
  assert.deepEqual(layouts(f), [grid([], [moduleAt(0)])]);
  f.configure({ adb: { responses: { 'widget-snapshot': snapshot(grid([other])) } } });
  fails(f.run(['launch-widget', f.project, widgetPath, '--position', '0']), /conflicts with a permanent Widget/);
  assert.equal(layouts(f).length, 1);
});

test('Widget launch validates declarations, mode, grid, and position', async (t) => {
  for (const [name, app, config, options, error] of [
    ['undeclared', {}, undefined, ['missing'], /Widget path or family is not declared/],
    ['bad family', { widgets: [{ path: widgetPath, family: 'large' }] }, undefined, [widgetPath], /Widget family must use/],
    ['zero family', { widgets: [{ path: widgetPath, family: '0x2' }] }, undefined, [widgetPath], /dimensions must be greater than zero/],
    ['production mode', {}, snapshot(undefined, { environment: 'prod' }), [widgetPath], /requires Developer Mode/],
    ['bad config', {}, snapshot('[]'), [widgetPath], /configurationJson must contain a JSON object/],
    ['bad grid', {}, snapshot({ ...grid(), totalColumnCount: 0 }), [widgetPath], /grid dimensions must be positive integers/],
    ['negative position', {}, snapshot(grid()), [widgetPath, '--position', '-1'], /--position must be a non-negative integer/],
    ['full permanent grid', {}, snapshot(grid([moduleAt(0), moduleAt(2)])), [widgetPath], /already configured as permanent/],
  ]) await t.test(name, (t) => {
    const f = setup(t, config ? { responses: { 'widget-snapshot': config } } : {});
    if (Object.keys(app).length) f.write('agent/app.json', app);
    fails(f.run(['launch-widget', f.project, ...options]), error);
    assert.deepEqual(layouts(f), []);
  });
});

test('widget-layout shows and clears placements while preserving grid settings', (t) => {
  const config = { ...grid([], [moduleAt(0)]), custom: 'preserve' };
  const f = setup(t, { responses: { 'widget-snapshot': snapshot(config) } });
  assert.match(succeeds(f.run(['widget-layout', 'show'])), /Grid: 2 columns, 4 cells[\s\S]*Dynamic Widget Placements \(1\)/);
  assert.match(succeeds(f.run(['widget-layout', '--clear', '--yes'])), /Widget layout cleared/);
  assert.deepEqual(layouts(f), [{ ...config, overlayModules: [] }]);
  f.configure({ adb: { responses: { 'widget-snapshot': snapshot(undefined) } } });
  assert.match(succeeds(f.run(['widget-layout'])), /Layout: not configured/);
  assert.match(succeeds(f.run(['widget-layout', '--clear', '--yes'])), /already empty/);
});

test('widget-layout refuses destructive or inconsistent arguments without device calls', async (t) => {
  const f = setup(t);
  for (const [args, error] of [
    [['--clear'], /requires confirmation; rerun with --yes/],
    [['--yes'], /--yes requires --clear/],
    [['show', '--clear', '--yes'], /show cannot be used with --clear/],
  ]) await t.test(args.join(' '), () => fails(f.run(['widget-layout', ...args]), error));
  assert.deepEqual(calls(f), []);
});
