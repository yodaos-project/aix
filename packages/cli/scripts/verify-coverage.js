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

// Codecov treats a line with an unexecuted branch as a miss or partial even
// when C8 counts the line as executed. Enforce the floor conservatively for
// that interpretation as well, retaining every BRDA entry in the upload.
let totalLines = 0;
let completeLines = 0;
const lcovSources = [];
for (const record of fs.readFileSync(path.join(root, 'coverage/lcov.info'), 'utf8').split('end_of_record')) {
  const lines = new Map();
  const incomplete = new Set();
  for (const entry of record.split('\n')) {
    if (entry.startsWith('SF:')) lcovSources.push(path.resolve(root, entry.slice(3)));
    if (entry.startsWith('DA:')) {
      const [line, hits] = entry.slice(3).split(',').map(Number);
      assert(Number.isInteger(line) && line > 0 && Number.isFinite(hits) && hits >= 0, 'Invalid LCOV line counts');
      lines.set(line, hits);
    }
    if (entry.startsWith('BRDA:')) {
      const [line, , , hits] = entry.slice(5).split(',');
      if (hits === '-' || Number(hits) === 0) incomplete.add(Number(line));
    }
  }
  totalLines += lines.size;
  completeLines += [...lines].filter(([line, hits]) => hits > 0 && !incomplete.has(line)).length;
}
assert.deepEqual(lcovSources.sort(), sources(path.join(root, 'src')).sort(), 'LCOV must include every CLI source');
assert.equal(totalLines, report.total.lines.total, 'LCOV and C8 must agree on the line denominator');
const completePercent = 100 * completeLines / totalLines;
assert(completePercent >= 98, `CLI coverage with incomplete branches counted as misses is ${completePercent.toFixed(2)}%; expected at least 98%`);
console.log(`Coverage verified: all CLI sources, subprocesses, and check Worker; branch-complete lines ${completePercent.toFixed(2)}%`);
