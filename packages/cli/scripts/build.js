const fs = require("fs");
const path = require("path");
const { execSync, spawnSync } = require("child_process");
const { buildCheck } = require('../../../crates/aix-web/scripts/build-check');

const rootDir = path.resolve(__dirname, "..");
const distDir = path.join(rootDir, "dist");
const aixWebDir = path.resolve(rootDir, "../../crates/aix-web");
const wasmOutDir = path.join(aixWebDir, "dist", "pkg-cli");
const pkgDir = path.join(distDir, "pkg");

async function main() {

  if (fs.existsSync(distDir)) {
    fs.rmSync(distDir, { recursive: true, force: true });
  }
  fs.mkdirSync(distDir, { recursive: true });

  buildNodeWasm();

  // The wasm-bindgen wrapper resolves `aix_web_bg.wasm` relative to its own
  // __dirname, so the .wasm file must sit next to the wrapper after bundling.
  fs.mkdirSync(pkgDir, { recursive: true });
  await buildCheck(path.join(pkgDir, 'check.cjs'), 'cjs');
  for (const file of ["aix_web.js", "aix_web_bg.wasm"]) {
    const src = path.join(wasmOutDir, file);
    if (!fs.existsSync(src)) {
      throw new Error(`Missing WASM artifact: ${src}`);
    }
    fs.copyFileSync(src, path.join(pkgDir, file));
  }

  // The WASM wrapper is intentionally NOT bundled: cli.ts loads it through a
  // dynamic require so its __dirname stays correct. Runtime-only node_modules
  // dependencies stay external as well.
  execSync(
    'npx esbuild src/cli.ts --bundle --platform=node --format=cjs --target=node20 --outfile=dist/cli.js --banner:js="#!/usr/bin/env node" --external:ignore --external:ws',
    { cwd: rootDir, stdio: "inherit" },
  );
  execSync(
    'npx esbuild src/commands/check-worker.ts --bundle --platform=node --format=cjs --target=node20 --outfile=dist/check-worker.js',
    { cwd: rootDir, stdio: "inherit" },
  );

  fs.chmodSync(path.join(distDir, "cli.js"), 0o755);

  execSync("npx tsc --noEmit", { cwd: rootDir, stdio: "inherit" });

  console.log("Build completed. Output in dist/");
}

main().catch((error) => { console.error(error); process.exitCode = 1; });

function buildNodeWasm() {
  const baseArgs = [
    "build",
    "--target",
    "nodejs",
    "--out-dir",
    wasmOutDir,
    "--out-name",
    "aix_web",
  ];
  const fallbackAttempt = runWasmPack(baseArgs);
  if (fallbackAttempt.status !== 0) {
    throw new Error(
      `${fallbackAttempt.stdout}\n${fallbackAttempt.stderr}`.trim(),
    );
  }
}

function runWasmPack(args) {
  const result = spawnSync("wasm-pack", args, {
    cwd: aixWebDir,
    encoding: "utf-8",
  });
  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
  return result;
}
