const assert = require('node:assert/strict');
const prefix = 'node_modules/@yodaos-pkg/ink-env/';
const environment = (version) => [
  {path: prefix + 'package.json', source: JSON.stringify({name: '@yodaos-pkg/ink-env', version, types: 'index.d.ts'})},
  {path: prefix + 'index.d.ts', source: `declare const selectedVersion: "${version}";`},
];
const files = (source, extra = {}) => ({'app.json': '{"pages":[]}', 'main.ts': source, ...extra});
module.exports = [
  ...['1.0.0', '2.0.0'].map((version) => ({
    name: `uses developer-supplied type version ${version}`,
    files: files(`const version: "${version}" = selectedVersion;`),
    options: {typeFiles: environment(version)},
    verify(report) { assert.deepEqual(report.diagnostics, []); },
  })),
  {
    name: 'reports API mismatches against the selected version',
    files: files('const version: "2.0.0" = selectedVersion;'),
    options: {typeFiles: environment('1.0.0')},
    verify(report) { assert.equal(report.diagnostics.length, 1); assert.match(report.diagnostics[0].message, /TS2322/); },
  },
  {
    name: 'replaces the whole installed package when supplying another version',
    files: files('import { removed } from "@yodaos-pkg/ink-env/removed"; const version: "2.0.0" = selectedVersion;', {
      '.aixignore': 'node_modules/',
      [prefix + 'package.json']: '{"types":"index.d.ts"}',
      [prefix + 'index.d.ts']: 'declare const selectedVersion: "1.0.0";',
      [prefix + 'removed.d.ts']: 'export const removed: number;',
    }),
    options: {typeFiles: environment('2.0.0')},
    verify(report) { assert.equal(report.diagnostics.length, 1); assert.match(report.diagnostics[0].message, /TS2307.*removed/); },
  },
  {
    name: 'allows the host to override tsconfig type packages explicitly',
    files: files('const version: "2.0.0" = selectedVersion;', {
      'tsconfig.json': '{"compilerOptions":{"types":["unavailable-package"]}}',
    }),
    options: {typeFiles: environment('2.0.0'), types: ['@yodaos-pkg/ink-env']},
    verify(report) { assert.deepEqual(report.diagnostics, []); },
  },
  {
    name: 'allows the host to disable automatic Ink globals',
    files: files('void selectedVersion;'),
    options: {typeFiles: environment('2.0.0'), types: []},
    verify(report) { assert.equal(report.diagnostics.length, 1); assert.match(report.diagnostics[0].message, /TS2304/); },
  },
];
