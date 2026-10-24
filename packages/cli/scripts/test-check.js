const { buildSync } = require("esbuild");
const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const path = require("node:path");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const ts = require("typescript");

const cliRoot = path.resolve(__dirname, "..");
const compiled = buildSync({
  entryPoints: [path.join(cliRoot, "src/check-typescript.ts")],
  bundle: true, platform: "node", format: "cjs", write: false, external: ["typescript", "@yodaos-pkg/ink-env"],
}).outputFiles[0].text;
const checker = { exports: {} };
new Function("require", "module", "exports", compiled)(require, checker, checker.exports);
const { checkTypeScript } = checker.exports;
const engine = require(path.join(cliRoot, "dist/pkg/aix_web.js"));

let passed = 0;
function test(name, files, verify) {
  const root = mkdtempSync(path.join(tmpdir(), "aix-check-"));
  try {
    files = { "app.json": '{"pages":[]}', ...files };
    for (const [file, content] of Object.entries(files)) {
      const target = path.join(root, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    const inputs = Object.entries(files).map(([file, content]) => ({ path: file, data: Buffer.from(content) }));
    const scripts = engine.collect_check_scripts(inputs);
    const diagnostics = checkTypeScript(root, scripts);
    verify(diagnostics, root);
    passed++;
    console.log(`ok - ${name}`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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
console.log(`${passed} check tests passed (TypeScript ${ts.version})`);
