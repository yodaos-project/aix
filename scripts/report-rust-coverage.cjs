const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const [toolchain, nativeTarget, wasmTarget] = process.argv.slice(2);
assert(toolchain && nativeTarget && wasmTarget, 'Expected toolchain, native target, and WASM target');
const reportRoot = path.resolve('coverage/rust');
const libdir = execFileSync('rustc', [`+${toolchain}`, '--print', 'target-libdir'], { encoding: 'utf8' }).trim();
const llvm = (name) => path.resolve(libdir, '../bin', name);
const binaries = (directory, pattern) => fs.readdirSync(directory)
  .filter((name) => pattern.test(name)).map((name) => path.join(directory, name));
const native = binaries(path.join(nativeTarget, 'debug/deps'), /^aiui_aix(?:_pack|_web)?-[a-f0-9]+$/);
const wasm = binaries(path.join(wasmTarget, 'wasm32-unknown-unknown/debug/deps'), /^aiui_aix_web-[a-f0-9]+\.wasm$/);
assert.equal(native.length, 3, 'Expected exactly one fresh native test executable per crate');
assert.equal(wasm.length, 1, 'Expected one fresh WASM test executable');

const read = (name) => JSON.parse(fs.readFileSync(path.join(reportRoot, name), 'utf8'));
const nativeReport = read('native-summary.json');
const wasmReport = read('wasm-summary.json');
assert(wasmReport.data[0].totals.lines.covered > 0, 'WASM tests must execute instrumented Rust');
const sources = [...new Set([...nativeReport.data[0].files, ...wasmReport.data[0].files].map((file) => file.filename))].sort();
const rustSources = (directory) => fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
  const file = path.resolve(directory, entry.name);
  if (entry.isDirectory()) return rustSources(file);
  // Match cargo-llvm-cov's existing convention for separate test modules.
  return file.endsWith('.rs') && !/[_-]tests\.rs$/.test(file) ? [file] : [];
});
assert.deepEqual(sources, ['aix', 'aix-pack', 'aix-web'].flatMap((name) => rustSources(`crates/${name}/src`)).sort(), 'Coverage must include every Rust source file');

const profile = path.join(reportRoot, 'combined.profdata');
execFileSync(llvm('llvm-profdata'), ['merge', '-sparse', path.join(nativeTarget, 'aix.profdata'), path.join(wasmTarget, 'aix.profdata'), '-o', profile], { stdio: 'inherit' });
const common = [`-instr-profile=${profile}`, ...[...native, ...wasm].flatMap((file) => ['-object', file])];
function writeReport(filename, args) {
  const output = fs.openSync(path.join(reportRoot, filename), 'w');
  try { execFileSync(llvm('llvm-cov'), [...args, ...common, '--sources', ...sources], { stdio: ['ignore', output, 'inherit'] }); }
  finally { fs.closeSync(output); }
}
// LLVM combines functions/regions across targets and counts a source line once.
// Reuse those real counters for all formats instead of averaging percentages.
writeReport('coverage-summary.json', ['export', '-format=text', '-summary-only']);
writeReport('lcov.info', ['export', '-format=lcov']);
execFileSync(llvm('llvm-cov'), ['show', '-format=html', '-show-instantiations=false', `-output-dir=${path.join(reportRoot, 'html')}`, ...common, '--sources', ...sources], { stdio: 'inherit' });
const lines = read('coverage-summary.json').data[0].totals.lines;
assert(lines.count > 0 && 100 * lines.covered / lines.count >= 98, `Rust line coverage is ${lines.percent.toFixed(2)}%; expected at least 98%`);
console.log(`Rust line coverage: ${lines.percent.toFixed(2)}% (native + WASM)`);
