const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const report = JSON.parse(fs.readFileSync(path.join(root, 'coverage/coverage-summary.json'), 'utf8'));

function sources(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return sources(file);
    return file.endsWith('.ts') && !file.endsWith('.d.ts') ? [file] : [];
  });
}

// Catch missing --all coverage and accidental inclusion of bundled dependencies.
assert.deepEqual(
  Object.keys(report).filter((file) => file !== 'total').sort(),
  sources(path.join(root, 'src')).sort(),
  'Coverage must include every CLI source file and only CLI sources',
);

// c8 filters multi-source maps before resolving relative paths or file: URLs.
// A bad filter can silently report 100% lines with no recorded functions.
// These existing tests execute functions in both a CLI child and its Worker.
for (const source of ['cli.ts', 'commands/check.ts', 'commands/check-worker.ts', 'walk.ts']) {
  assert.ok(
    report[path.join(root, 'src', source)].functions.covered > 0,
    `Missing executed functions in ${source}; check source maps and c8 filters`,
  );
}

console.log('Coverage verified: all CLI sources, subprocesses, and check Worker');
