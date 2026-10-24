const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

// Explicit paths work on Node 20 and newer, including Windows shells that do
// not expand globs and Node versions that no longer accept a directory here.
const directory = path.resolve(__dirname, '../tests');
const files = fs.readdirSync(directory).filter((name) => name.endsWith('.test.cjs')).sort();
const result = spawnSync(process.execPath, ['--test', '--test-concurrency=1', ...files.map((name) => path.join(directory, name))], { stdio: 'inherit' });
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
