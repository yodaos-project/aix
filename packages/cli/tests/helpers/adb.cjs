const fs = require('node:fs');
const path = require('node:path');
const { adb = {} } = JSON.parse(fs.readFileSync(process.env.AIX_CLI_TEST_CONFIG, 'utf8'));
const logFile = process.env.AIX_CLI_TEST_LOG;
const args = process.argv.slice(2);
const command = args[0] === '-s' ? args.slice(2) : args;
const methodIndex = command.indexOf('--method');
let method = methodIndex >= 0 ? command[methodIndex + 1] : command[0];
const entry = { kind: 'adb', args };
if (command[0] === 'shell' && command[1]?.startsWith('content call ')) {
  method = 'open';
  const quoted = command[1].slice(command[1].indexOf('--arg ') + 6);
  entry.request = JSON.parse(quoted.slice(1, -1).split("'\"'\"'").join("'"));
}
if (command[0] === 'shell' && command[1]?.startsWith('find ')) method = 'find';
if (command[0] === 'shell' && command[1]?.startsWith('if [')) method = 'manifest';
entry.method = method;
if (command[0] === 'push') {
  entry.files = command.slice(1, -1).map((file) => ({
    path: file,
    bytes: fs.statSync(file).size,
    json: path.extname(file) === '.json' ? JSON.parse(fs.readFileSync(file, 'utf8')) : undefined,
  }));
}
const history = fs.existsSync(logFile)
  ? fs.readFileSync(logFile, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse)
  : [];
fs.appendFileSync(logFile, JSON.stringify(entry) + '\n');
if (adb.fail === method) {
  process.stderr.write('simulated ADB failure');
  process.exit(1);
}
if (method === 'devices') {
  process.stdout.write(adb.devices ?? 'List of devices attached\ntest-device\tdevice\n');
} else if (method === 'find') {
  process.stdout.write(adb.files ?? '');
} else if (method === 'manifest') {
  process.stdout.write(adb.manifest ? JSON.stringify(adb.manifest) : '');
} else if (method === 'push' || method === 'shell') {
  process.stdout.write('ok');
} else {
  const defaults = {
    prepare: { errorCode: '', ready: true, inboxPath: '/mock/inbox' },
    apply: { errorCode: '', agentId: adb.agentId, outcome: 'CREATED', operationId: 'op-1' },
    status: { errorCode: '', publishState: 'UPLOADED' },
    open: { errorCode: '', ok: true },
    remove: { errorCode: '', localCompleted: true, success: true, dispatched: true },
    switch: { errorCode: '', ok: true },
    'widget-snapshot': { errorCode: '', ok: true, environment: 'dev', ready: true },
    'widget-apply': { errorCode: '', ok: true, configurationVersion: 'layout-2' },
  };
  let value = adb.responses?.[method] ?? defaults[method];
  if (Array.isArray(value)) {
    const count = history.filter((call) => call.kind === 'adb' && call.method === method).length;
    value = value[Math.min(count, value.length - 1)];
  }
  if (value === undefined) throw new Error(`Unexpected ADB method: ${method}`);
  process.stdout.write(typeof value === 'string' ? value : `Result: Bundle[{result_data=${JSON.stringify(value)}}]`);
}
