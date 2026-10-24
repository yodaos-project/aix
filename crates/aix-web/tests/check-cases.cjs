const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const path = require('node:path');
const cliRoot = path.resolve(__dirname, '../../../packages/cli');
const cases = [];
function test(name, files, verify) {
  cases.push({ name, files: { 'app.json': '{"pages":[]}', ...files }, verify });
}

const clean = (diagnostics) => assert.deepEqual(diagnostics, []);
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
}, (diagnostics) => assert(diagnostics.some((d) => d.code === "AIX049" && d.path === "tsconfig.json")));
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

module.exports = cases;
