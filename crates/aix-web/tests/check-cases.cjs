const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const path = require('node:path');
const cliRoot = path.resolve(__dirname, '../../../packages/cli');
const cases = [];
function test(name, files, verify, verifyChecks) {
  cases.push({ name, files: { 'app.json': '{"pages":[]}', ...files }, verify, verifyChecks });
}

const clean = (diagnostics, _, report) => {
  assert.deepEqual(diagnostics, []);
  assert(!report.diagnostics.some((diagnostic) => diagnostic.severity === 'error'), JSON.stringify(report.diagnostics));
};
test("catches semantic errors across imports", {
  "value.ts": "export const value: number = 1;",
  "main.ts": "import { value } from './value';\nconst text: string = value;",
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].path, "main.ts");
  assert.equal(diagnostics[0].line, 2);
  assert.match(diagnostics[0].message, /TS2322/);
});
test("uses Ink globals without ambient Node pollution", {
  "main.ts": "console.log('Ink');\nconst id: number = setTimeout(() => {}, 10);\nfetch('https://example.com');",
}, clean);
test("resolves project declarations and imported JS types", {
  "types.d.ts": "declare const custom: number;",
  "value.js": "export const value = 'hello';",
  "main.ts": "import { value } from './value.js';\nconst text: string = value; console.log(custom);",
}, clean);
test("honors tsconfig extends and path aliases", {
  "base.json": '{"compilerOptions":{"module":"CommonJS","strict":false,"baseUrl":".","paths":{"@lib/*":["lib/*"]}}}',
  "tsconfig.json": '{"extends":"./base.json","compilerOptions":{"types":["@yodaos-pkg/ink-env"]}}',
  "lib/value.ts": "export const value: number = 1;",
  "main.ts": "import { value } from '@lib/value'; let count: number = null; console.log(value, count);",
}, clean);
test("reports invalid tsconfig", {
  "tsconfig.json": '{"compilerOptions":{"strict":"yes"}}',
  "main.ts": "export const value = 1;",
}, (diagnostics) => assert(diagnostics.some((d) => d.code === "AIX049" && d.path === "tsconfig.json")), (checks) => {
  assert.equal(checks.find((check) => check.id === 'typescript_config').status, 'failed');
  assert.equal(checks.find((check) => check.id === 'typescript').status, 'skipped');
});
test("maps inline offsets after Unicode and ignores markup/comments/def", {
  "main.ink": '<!-- 😀 <script lang="ts">fake</script> --><script def>{"title":"text"}</script>\n<page><text /></page>\n<script setup lang = "ts">const emoji = "😀"; const count: number = "bad";</script>',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].path, "main.ink");
  assert.equal(diagnostics[0].line, 3);
  assert.equal(diagnostics[0].column, 52);
});
test("isolates Ink files and resolves inline module exports", {
  "one.ink": '<script lang="ts">const count = 1; export default count;</script>',
  "two.ink": '<script lang="ts">import value from "./one.ink"; const count: number = value;</script>',
}, clean);
test("keeps adjacent inline scripts separate without semicolons", {
  "main.ink": '<script lang="ts">const value = 1</script><script lang="ts">const text: string = value</script>',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.match(diagnostics[0].message, /TS2322/);
  assert.equal(diagnostics[0].path, "main.ink");
  assert.equal(diagnostics[0].line, 1);
});
test("prefers project Ink declarations", {
  "node_modules/@yodaos-pkg/ink-env/package.json": '{"types":"index.d.ts"}',
  "node_modules/@yodaos-pkg/ink-env/index.d.ts": 'declare const projectInkVersion: "custom";',
  "main.ts": 'const version: "custom" = projectInkVersion;',
}, clean);
test("ignored files stay excluded even when imported", {
  ".aixignore": "ignored.ts\n",
  "ignored.ts": "export const value: number = 'bad';",
  "main.ts": "import { value } from './ignored'; console.log(value);",
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].path, "main.ts");
  assert.match(diagnostics[0].message, /TS2307/);
});
test("reports CLI errors as JSON and exits 1", {
  "app.json": '{"pages":["main"]}',
  "main.ink": '<script def>{"schema":{"data":{"type":"invalid"}}}</script><page><view /></page><script lang="ts">const count: number = "bad";</script>',
}, (_, root) => {
  const result = spawnSync(process.execPath, [path.join(cliRoot, "dist/cli.js"), "check", root, "--format", "json"], { encoding: "utf8" });
  assert.equal(result.status, 1, result.stderr);
  const { diagnostics } = JSON.parse(result.stdout);
  assert(diagnostics.some((d) => d.code === "AIX047"));
  assert(diagnostics.some((d) => d.code === "AIX048" && d.message.includes("TS2322")));
});
test("keeps warning-only exit status and text output", {
  "main.ts": "fetch('https://example.com');",
}, (_, root) => {
  const result = spawnSync(process.execPath, [path.join(cliRoot, "dist/cli.js"), "check", root], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /warning \[AIX033\]/);
  assert.match(result.stdout, /1 diagnostic\(s\)/);
});

test('shares schema, permission, syntax, and semantic diagnostics', {
  'app.json': '{"pages":["pages/home"]}',
  'pages/home.ink': '<script def>{"schema":{"data":{"properties":{"count":{"type":"invalid"}}}}}</script><page><view /></page><script lang="ts">const count: number = "bad"; navigator.geolocation.getCurrentPosition(() => {});</script>',
  'broken.js': 'const value = ;',
}, (diagnostics, _, report) => {
  for (const code of ['AIX027', 'AIX029', 'AIX047', 'AIX048']) {
    assert(report.diagnostics.some((d) => d.code === code), code);
  }
});
test('resolves ignored dependency types and package configs in memory', {
  '.aixignore': 'node_modules/',
  'node_modules/@demo/config/package.json': '{"name":"@demo/config"}',
  'node_modules/@demo/config/base.json': '{"compilerOptions":{"strict":false}}',
  'node_modules/@demo/types/package.json': '{"types":"index.d.ts"}',
  'node_modules/@demo/types/index.d.ts': 'export const value: number;',
  'tsconfig.json': '{"extends":"@demo/config/base.json"}',
  'main.ts': 'import {value} from "@demo/types"; const count: number = null; const text: string = value;',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.match(diagnostics[0].message, /TS2322/);
});
test('excludes local .aix cache directories in every host', {
  '.aix/cache.ts': 'const value: number = "bad";',
  '.aix/broken.json': '{invalid',
  'main.ts': 'const count: number = 1;',
}, (diagnostics, _, report) => assert.deepEqual(report.diagnostics, []));

test('checks named exports across the project module graph', {
  'value.ts': 'export const value = 1;',
  'barrel.ts': 'export { value } from "./value";',
  'main.ts': 'import { missing } from "./barrel"; void missing;',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.match(diagnostics[0].message, /TS2305/);
});
test('honors tsconfig files instead of treating every file as a root', {
  'tsconfig.json': '{"files":["main.ts"]}',
  'main.ts': 'export const value = 1;',
  'other.ts': 'const wrong: string = 1;',
}, clean);
test('honors include and exclude but still follows imports into excluded files', {
  'tsconfig.json': '{"include":["src/**/*.ts"],"exclude":["src/dependency.ts","src/unreferenced.ts"]}',
  'src/main.ts': 'import {value} from "./dependency"; void value;',
  'src/dependency.ts': 'export const value: string = 1;',
  'src/unreferenced.ts': 'const other: string = 2;',
  'outside.ts': 'const outside: string = 3;',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].path, 'src/dependency.ts');
  assert.match(diagnostics[0].message, /TS2322/);
});
test('includes Ink globs as isolated modules in the configured project', {
  'tsconfig.json': '{"include":["pages/**/*.ink"]}',
  'pages/one.ink': '<script lang="ts">const count: number = "bad";</script>',
  'pages/two.ink': '<script lang="ts">const count: number = 1;</script>',
  'other.ts': 'const wrong: string = 2;',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].path, 'pages/one.ink');
  assert.match(diagnostics[0].message, /TS2322/);
});
test('reports missing root files from the project config', {
  'tsconfig.json': '{"files":["missing.ts"]}',
  'other.js': 'const value = 1;',
}, (diagnostics) => assert(diagnostics.some((d) => d.message.includes('TS6053'))));
test('enforces NodeNext ESM import extensions', {
  'package.json': '{"type":"module"}',
  'tsconfig.json': '{"compilerOptions":{"module":"NodeNext"}}',
  'value.ts': 'export const value = 1;',
  'main.ts': 'import { value } from "./value"; void value;',
}, (diagnostics) => {
  assert.equal(diagnostics.length, 1);
  assert.match(diagnostics[0].message, /TS2835/);
});
test('allows NodeNext ESM imports using the emitted js extension', {
  'package.json': '{"type":"module"}',
  'tsconfig.json': '{"compilerOptions":{"module":"NodeNext"}}',
  'value.ts': 'export const value = 1;',
  'main.ts': 'import { value } from "./value.js"; void value;',
}, clean);
test('resolves import and require exports per usage in the same project', {
  '.aixignore': 'node_modules/',
  'tsconfig.json': '{"compilerOptions":{"module":"NodeNext"}}',
  'node_modules/conditional/package.json': '{"exports":{".":{"import":{"types":"./import.d.mts"},"require":{"types":"./require.d.cts"}}}}',
  'node_modules/conditional/import.d.mts': 'export const value: string;',
  'node_modules/conditional/require.d.cts': 'export const value: number;',
  'esm.mts': 'import {value} from "conditional"; import cjs = require("conditional"); import type {value as required} from "conditional" with {"resolution-mode":"require"}; const text: string = value; const count: number = cjs.value; const typed: typeof required = 1;',
  'common.cts': 'import {value} from "conditional"; const count: number = value;',
}, clean);
test('uses each files nearest package type to choose ESM or CJS', {
  '.aixignore': 'node_modules/',
  'package.json': '{"type":"module"}',
  'tsconfig.json': '{"compilerOptions":{"module":"NodeNext"}}',
  'node_modules/conditional/package.json': '{"exports":{".":{"import":{"types":"./import.d.mts"},"require":{"types":"./require.d.cts"}}}}',
  'node_modules/conditional/import.d.mts': 'export const value: string;',
  'node_modules/conditional/require.d.cts': 'export const value: number;',
  'esm.ts': 'import {value} from "conditional"; const text: string = value;',
  'cjs/package.json': '{"type":"commonjs"}',
  'cjs/main.ts': 'import {value} from "conditional"; const count: number = value;',
}, clean);
test('preserves explicit resolution mode on type reference directives', {
  '.aixignore': 'node_modules/',
  'tsconfig.json': '{"compilerOptions":{"module":"NodeNext"}}',
  'node_modules/conditional/package.json': '{"exports":{".":{"import":{"types":"./import.d.mts"},"require":{"types":"./require.d.cts"}}}}',
  'node_modules/conditional/import.d.mts': 'declare global { const condition: "import"; } export {};',
  'node_modules/conditional/require.d.cts': 'declare global { const condition: "require"; } export {};',
  'main.cts': '/// <reference types="conditional" resolution-mode="import" />\nconst mode: "import" = condition;',
}, clean);
test('reports syntax errors in ignored project configs with source locations', {
  '.aixignore': 'tsconfig.json',
  'tsconfig.json': '{\n"compilerOptions": {"strict":true,,}\n}',
  'main.ts': 'export {};',
}, (diagnostics) => assert(diagnostics.some((d) => d.code === 'AIX049'
  && d.path === 'tsconfig.json' && d.line === 2 && d.message.includes('TS1136'))));

test('lists each completed check with its actual work count', {
  'app.json': '{"pages":["home"]}',
  'home.ink': '<script def>{"schema":{"data":{"type":"object"}}}</script><page><view /></page><script lang="ts">const count: number = 1;</script>',
}, clean, (checks) => {
  assert.equal(checks.length, 7);
  assert(checks.every((check) => check.status === 'passed'), JSON.stringify(checks));
  assert.match(checks.find((check) => check.id === 'syntax').label, /1 script/);
  assert.match(checks.find((check) => check.id === 'schemas').label, /1 schema/);
  assert.match(checks.find((check) => check.id === 'typescript').label, /1 root/);
});

test('skips checks without scripts schemas or a TypeScript project', {}, clean, (checks) => {
  for (const id of ['syntax', 'schemas', 'typescript_config', 'typescript']) {
    assert.equal(checks.find((check) => check.id === id).status, 'skipped', id);
  }
  for (const id of ['json', 'structure', 'permissions']) {
    assert.equal(checks.find((check) => check.id === id).status, 'passed', id);
  }
});

test('does not pass source checks blocked by invalid app metadata', {
  'app.json': 'null',
  'main.ts': 'export const count: number = 1;',
}, (_, __, report) => assert(report.diagnostics.some((diagnostic) => diagnostic.code === 'AIX004')), (checks) => {
  assert.equal(checks.find((check) => check.id === 'structure').status, 'failed');
  for (const id of ['syntax', 'permissions', 'schemas']) assert.equal(checks.find((check) => check.id === id).status, 'skipped', id);
  assert.equal(checks.find((check) => check.id === 'typescript').status, 'passed');
});

test('keeps successful checks alongside schema type and permission issues', {
  'app.json': '{"pages":["home"]}',
  'home.ink': '<script def>{"schema":{"data":{"type":"invalid"}}}</script><page><view /></page><script lang="ts">const count: number = "bad"; fetch("https://example.com");</script>',
}, (diagnostics, _, report) => {
  assert(diagnostics.some((diagnostic) => diagnostic.code === 'AIX048'));
  assert(report.diagnostics.some((diagnostic) => diagnostic.code === 'AIX047'));
}, (checks) => {
  for (const id of ['json', 'structure', 'syntax', 'typescript_config']) assert.equal(checks.find((check) => check.id === id).status, 'passed', id);
  assert.equal(checks.find((check) => check.id === 'schemas').status, 'failed');
  assert.equal(checks.find((check) => check.id === 'typescript').status, 'failed');
  assert.equal(checks.find((check) => check.id === 'permissions').status, 'warning');
});

test('does not pass permission analysis blocked by script syntax errors', {
  'broken.js': 'const count = ;',
}, (_, __, report) => assert(report.diagnostics.some((diagnostic) => diagnostic.code === 'AIX029')), (checks) => {
  assert.equal(checks.find((check) => check.id === 'syntax').status, 'failed');
  assert.equal(checks.find((check) => check.id === 'permissions').status, 'skipped');
});

module.exports = cases;
