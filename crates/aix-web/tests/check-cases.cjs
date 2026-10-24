const assert = require('node:assert/strict');
const cases = [];
function test(name, files, verify = (report) => assert.deepEqual(report.diagnostics, []), verifyChecks) {
  cases.push({name, files: {'app.json': '{"pages":[]}', ...files}, verify, verifyChecks});
}
const hasCode = (code) => (report) => assert(report.diagnostics.some((diagnostic) => diagnostic.code === code), JSON.stringify(report));

test('accepts TypeScript type mismatches without a type environment', {
  'value.ts': 'export const value: number = 1;',
  'main.ts': 'import {value} from "./value"; const text: string = value; console.log(text);',
});
test('does not resolve imports or validate TypeScript compiler options', {
  'tsconfig.json': '{"extends":"missing-config","compilerOptions":{"strict":"yes","types":["unavailable-types"]}}',
  'main.ts': 'import {missing} from "unavailable-package"; const value: number = "bad";',
});
test('checks JavaScript syntax', {
  'broken.js': 'const value = ;',
}, hasCode('AIX029'));
test('checks TypeScript syntax', {
  'broken.ts': 'const value: number = ;',
}, hasCode('AIX029'));
test('maps inline syntax errors after Unicode and ignores comments and def blocks', {
  'main.ink': '<!-- 😀 <script lang="ts">fake = ;</script> --><script def>{"title":"text"}</script>\n<page><text /></page>\n<script setup lang = "ts">const emoji = "😀"; const count: number = ;</script>',
}, (report) => {
  assert.equal(report.diagnostics.length, 1);
  const diagnostic = report.diagnostics[0];
  assert.equal(diagnostic.code, 'AIX029');
  assert.equal(diagnostic.path, 'main.ink');
  assert.equal(diagnostic.line, 3);
});
test('checks adjacent Ink scripts independently', {
  'main.ink': '<script lang="ts">const count = 1</script><script lang="ts">const count: string = "text"</script>',
});
test('checks CommonJS and TypeScript declaration syntax', {
  'helper.cjs': 'module.exports = 1;',
  'main.cts': 'import value = require("helper"); export = value;',
  'types.d.cts': 'declare const value: number; export = value;',
});
test('honors nested ignore rules', {
  '.aixignore': 'ignored.ts\nnode_modules/',
  'ignored.ts': 'const broken = ;',
  'node_modules/example/index.d.ts': 'invalid syntax #',
  'nested/.aixignore': 'broken.js',
  'nested/broken.js': 'const broken = ;',
  'main.ts': 'import value from "./ignored"; void value;',
});
test('excludes local cache directories in every host', {
  '.aix/cache.ts': 'const broken = ;',
  '.aix/broken.json': '{invalid',
  'main.ts': 'const count: number = 1;',
});
test('validates JSON syntax', {
  'broken.json': '{invalid',
}, hasCode('AIX001'));
test('checks Page entry structure', {
  'app.json': '{"pages":["missing"]}',
}, hasCode('AIX016'));
test('shares schema permission and syntax diagnostics', {
  'app.json': '{"pages":["pages/home"]}',
  'pages/home.ink': '<script def>{"schema":{"data":{"properties":{"count":{"type":"invalid"}}}}}</script><page><view /></page><script lang="ts">const count: number = "bad"; navigator.geolocation.getCurrentPosition(() => {});</script>',
  'broken.js': 'const value = ;',
}, (report) => {
  for (const code of ['AIX027', 'AIX029', 'AIX047']) hasCode(code)(report);
  assert(!report.diagnostics.some((diagnostic) => ['AIX048', 'AIX049'].includes(diagnostic.code)));
});
test('validates nested schema keywords patterns and local references', {
  'app.json': '{"pages":["home"]}',
  'home.ink': '<script def>{"schema":{"data":{"type":"object","properties":{"count":{"$ref":"#/$defs/count"}},"$defs":{"count":{"type":"integer","minimum":0}}}}}</script><page><view /></page>',
});
test('rejects unresolved external schema references without fetching', {
  'app.json': '{"pages":["home"]}',
  'home.ink': '<script def>{"schema":{"data":{"$ref":"https://example.invalid/schema"}}}</script><page><view /></page>',
}, hasCode('AIX047'));
test('keeps warning-only reports successful', {
  'main.ts': 'fetch("https://example.com");',
}, (report) => {
  assert.equal(report.diagnostics.length, 1);
  assert.equal(report.diagnostics[0].code, 'AIX033');
  assert.equal(report.diagnostics[0].severity, 'warning');
});
test('lists each completed check with its actual work count', {
  'app.json': '{"pages":["home"]}',
  'home.ink': '<script def>{"schema":{"data":{"type":"object"}}}</script><page><view /></page><script lang="ts">const count: number = "unchecked";</script>',
}, undefined, (checks) => {
  assert.equal(checks.length, 5);
  assert(checks.every((check) => check.status === 'passed'), JSON.stringify(checks));
  assert.match(checks.find((check) => check.id === 'syntax').label, /1 script/);
  assert.match(checks.find((check) => check.id === 'schemas').label, /1 schema/);
});
test('skips checks without scripts or schemas', {}, undefined, (checks) => {
  for (const id of ['syntax', 'schemas']) assert.equal(checks.find((check) => check.id === id).status, 'skipped', id);
  for (const id of ['json', 'structure', 'permissions']) assert.equal(checks.find((check) => check.id === id).status, 'passed', id);
});
test('does not pass source checks blocked by invalid app metadata', {
  'app.json': 'null',
  'main.ts': 'const broken = ;',
}, hasCode('AIX004'), (checks) => {
  assert.equal(checks.find((check) => check.id === 'structure').status, 'failed');
  for (const id of ['syntax', 'permissions', 'schemas']) assert.equal(checks.find((check) => check.id === id).status, 'skipped', id);
});
test('keeps successful checks alongside schema and permission issues', {
  'app.json': '{"pages":["home"]}',
  'home.ink': '<script def>{"schema":{"data":{"type":"invalid"}}}</script><page><view /></page><script lang="ts">const count: number = "unchecked"; fetch("https://example.com");</script>',
}, hasCode('AIX047'), (checks) => {
  for (const id of ['json', 'structure', 'syntax']) assert.equal(checks.find((check) => check.id === id).status, 'passed', id);
  assert.equal(checks.find((check) => check.id === 'schemas').status, 'failed');
  assert.equal(checks.find((check) => check.id === 'permissions').status, 'warning');
});
test('does not pass permission analysis blocked by syntax errors', {
  'broken.js': 'const count = ;',
}, hasCode('AIX029'), (checks) => {
  assert.equal(checks.find((check) => check.id === 'syntax').status, 'failed');
  assert.equal(checks.find((check) => check.id === 'permissions').status, 'skipped');
});

module.exports = cases;
