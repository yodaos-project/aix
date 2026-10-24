// Substitute only external boundaries; every test still executes dist/cli.js.
const fs = require('node:fs');
const path = require('node:path');
const childProcess = require('node:child_process');
const config = JSON.parse(fs.readFileSync(process.env.AIX_CLI_TEST_CONFIG, 'utf8'));
const record = (value) => fs.appendFileSync(process.env.AIX_CLI_TEST_LOG, JSON.stringify(value) + '\n');

globalThis.fetch = async (url, options) => {
  record({ kind: 'fetch', url: String(url), headers: options?.headers });
  if (!/^https:\/\/registry\.(npmjs\.org|npmmirror\.com)\//.test(String(url))) {
    throw new Error(`Unexpected network request in CLI test: ${url}`);
  }
  if (config.networkError) {
    throw new Error('fetch failed', { cause: new Error(config.networkError) });
  }
  const metadata = config.registry ?? {
    'dist-tags': { latest: '0.18.2' },
    versions: { '0.18.2': {}, '0.9.9': {}, '1.0.0': {}, '0.18.1': {}, '0.19.0-rc.1': {} },
  };
  return new Response(JSON.stringify(metadata), { status: config.httpStatus ?? 200 });
};

const spawn = childProcess.spawn;
childProcess.spawn = (command, args, options) => {
  if (command === 'adb') {
    return spawn(process.execPath, [path.join(__dirname, 'adb.cjs'), ...args], options);
  }
  if (['open', 'xdg-open', 'cmd'].includes(command)) {
    record({ kind: 'browser', command, args });
    return spawn(process.execPath, ['-e', ''], options);
  }
  throw new Error(`Unexpected subprocess in CLI test: ${command}`);
};

if (config.tty) {
  for (const stream of [process.stdin, process.stdout, process.stderr]) {
    Object.defineProperty(stream, 'isTTY', { value: true });
  }
  process.stderr.columns = process.stdout.columns = 100;
}
