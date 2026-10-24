const fs = require('node:fs');

const [kind, reportPath] = process.argv.slice(2);
if (!['rust', 'cli'].includes(kind) || !reportPath) {
  throw new Error('Usage: node scripts/coverage-summary.cjs <rust|cli> <report.json>');
}

const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));
const totals = kind === 'rust' ? report.data[0].totals : report.total;
const metrics = kind === 'rust'
  ? ['lines', 'functions', 'regions']
  : ['lines', 'statements', 'functions', 'branches'];
const rows = metrics.map((metric) => {
  const value = totals[metric];
  const total = kind === 'rust' ? value.count : value.total;
  const covered = value.covered;
  if (!Number.isFinite(total) || !Number.isFinite(covered)) {
    throw new Error(`Invalid ${metric} coverage in ${reportPath}`);
  }
  const percent = total === 0 ? 'N/A' : `${(100 * covered / total).toFixed(2)}%`;
  return `| ${metric} | ${covered} / ${total} | ${percent} |`;
});

const scope = kind === 'rust'
  ? 'Native tests across all three crates plus instrumented WASM binding tests in Node.js. LLVM combines execution counts across targets; doc-test coverage is not included. Regions are reported rather than branches.'
  : 'CLI TypeScript sources, including subprocesses and the check Worker. Unexecuted source files count as uncovered; dependencies, shared SDK code, and WASM internals are excluded. The 98% floor also applies conservatively with branch-incomplete lines counted as misses.';
const markdown = [
  `## ${kind === 'rust' ? 'Rust' : 'CLI'} test coverage`,
  '',
  '| Metric | Covered / total | Coverage |',
  '| --- | ---: | ---: |',
  ...rows,
  '',
  scope,
  '',
  `Download the ${kind}-coverage artifact for HTML, LCOV, and JSON reports. A minimum of 98% line coverage is enforced.`,
  '',
].join('\n');

process.stdout.write(markdown);
if (process.env.GITHUB_STEP_SUMMARY) {
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, markdown + '\n');
}
