const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');

const cliRoot = path.resolve(__dirname, '../..');
const cli = path.join(cliRoot, 'dist/cli.js');
const preload = path.join(__dirname, 'preload.cjs');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aix-cli-test-'));
  const home = path.join(root, 'home');
  fs.mkdirSync(home);
  const configFile = path.join(root, 'fixture.json');
  const logFile = path.join(root, 'calls.jsonl');
  const children = new Set();
  const env = {
    ...process.env, HOME: home, USERPROFILE: home, AIX_NPM_REGISTRY: 'npm',
    AIX_CLI_TEST_CONFIG: configFile, AIX_CLI_TEST_LOG: logFile,
  };
  delete env.NODE_OPTIONS;
  // V8 coverage stays inherited by the CLI, Workers, and simulated ADB children.
  t.after(async () => {
    await Promise.all([...children].map((running) => running.stop()));
    fs.rmSync(root, { recursive: true, force: true });
  });

  const write = (relative, data) => {
    const file = path.join(root, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, typeof data === 'object' && !Buffer.isBuffer(data) ? JSON.stringify(data) : data);
    return file;
  };
  const configure = (value = {}) => fs.writeFileSync(configFile, JSON.stringify(value));
  configure();
  const calls = () => fs.existsSync(logFile)
    ? fs.readFileSync(logFile, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse)
    : [];
  const run = (args, options = {}) => {
    const { env: extraEnv, ...rest } = options;
    const result = spawnSync(process.execPath, ['--require', preload, cli, ...args], {
      cwd: root, env: { ...env, ...extraEnv }, encoding: 'utf8', timeout: 15000, ...rest,
    });
    assert.ifError(result.error);
    assert.equal(result.signal, null, `CLI terminated: ${result.stderr}`);
    return result;
  };
  const start = (args) => {
    const child = spawn(process.execPath, ['--require', preload, cli, ...args], {
      cwd: root, env, stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    const done = new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('close', (status, signal) => resolve({ status, signal, stdout, stderr }));
    });
    const running = {
      child, done,
      async output(pattern) {
        const timeout = AbortSignal.timeout(10000);
        while (!pattern.test(stdout + stderr)) {
          if (child.exitCode !== null || child.signalCode !== null) throw new Error(`CLI exited before ${pattern}: ${stdout}${stderr}`);
          await new Promise((resolve, reject) => {
            const cleanup = () => {
              child.stdout.off('data', changed);
              child.stderr.off('data', changed);
              child.off('exit', changed);
              timeout.removeEventListener('abort', aborted);
            };
            const changed = () => { cleanup(); resolve(); };
            const aborted = () => { cleanup(); reject(new Error(`Timed out waiting for ${pattern}: ${stdout}${stderr}`)); };
            child.stdout.once('data', changed);
            child.stderr.once('data', changed);
            child.once('exit', changed);
            timeout.addEventListener('abort', aborted, { once: true });
            if (timeout.aborted) aborted();
          });
        }
        return pattern.exec(stdout + stderr);
      },
      async stop() {
        if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
        const timer = setTimeout(() => child.kill('SIGKILL'), 3000);
        try { return await done; } finally { clearTimeout(timer); children.delete(running); }
      },
    };
    children.add(running);
    return running;
  };
  const project = (app = {}) => {
    write('agent/app.json', { name: 'Test Agent', pages: ['pages/home'], ...app });
    write('agent/pages/home.ink', '<script def>{"parameters":{"type":"object","properties":{}}}</script><view>Hello</view>');
    write('agent/AGENTS.md', '# Agent\n\nA useful test agent.\n');
    return path.join(root, 'agent');
  };
  return { root, home, write, project, configure, calls, run, start };
}

function succeeds(result) {
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return result.stdout;
}

function fails(result, pattern) {
  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.match(result.stderr, pattern);
}

module.exports = { fixture, succeeds, fails };
