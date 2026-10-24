# Reader and tooling WASM loading

The Web SDK loads an independent reader WASM for `AIX.From()` and instance
methods. Reading archive entries, resolving page metadata/tools/Widgets, and
checking engine compatibility do not load the packer, image codecs, Oxc
parser/minifier, or JSON Schema validator.

The first `pack`, `packFromSource`, `packFromFiles`, `optimize`,
`checkFromSource`, or `checkFromFiles` call lazily initializes the full module.
Concurrent tooling calls share initialization. Reader instances keep their own
module memory and remain usable after tooling initializes. Tooling-only callers
do not fetch the reader WASM.

## Compatibility and build choices

The public TypeScript API and method signatures remain unchanged. Builds contain
two web-target artifacts:

| Artifact | Contents | Loading |
| --- | --- | --- |
| `dist/pkg-reader/aix_web.js` and adjacent `.wasm` | Reader exports only | SDK `AIX.From()` |
| `dist/pkg/aix_web.js` and adjacent `.wasm` | Existing reader and tooling exports | Lazy SDK tooling; existing direct WASM consumers |

The npm CLI continues to use its synchronous Node.js WASM wrapper with the full
export surface. It does not need dynamic loading or the additional reader file.
Self-hosted Web SDK distributions must include both web artifact directories;
their JS wrappers resolve the adjacent WASM files relative to their own URLs.
Deploy the generated distribution as a unit so an updated SDK can find the new
reader artifact. Build from source with `npm run build` in `crates/aix-web`.

`aiui-aix-web` now has a default-enabled `tooling` Cargo feature. Turning it off
removes the `aiui-aix-pack` dependency rather than merely hiding its JS entry
points:

```bash
cargo check -p aiui-aix-web --no-default-features --target wasm32-unknown-unknown
cargo tree -p aiui-aix-web --no-default-features --target wasm32-unknown-unknown -e normal
```

For direct reader-only WASM builds:

```bash
cd crates/aix-web
npm run build:wasm:reader
```

Default Cargo builds and the original `build:wasm` script retain all exports.
The archive format, manifest/signature verification, metadata semantics, and
core crate's `no_std` support are unchanged.

Feature-gating one crate avoids introducing another published Rust/npm package.
The compatibility artifact retains reader exports so direct consumers can keep
their paths. The tradeoff is that a workflow using both reading and tooling
downloads two binaries and maintains two WASM memories. The full compatibility
module still contains reader code; this change does not claim a smaller total
download or lower peak memory for mixed workflows. A third tooling-only artifact
would add another build and compatibility surface and was not selected.

## Measurements

Measured on 2026-10-25 on an Apple M2, macOS arm64, Node 25.6.1 and Chrome
156.0.8078.47. The monolithic baseline was built from `a838f8459ee3fc38ad6f08841afb79681bd98bd1`
before changing the Rust dependency/build structure. The split measurements use
the proposed implementation based on that revision; the raw reports' `revision`
records that base commit, since the candidate was measured before committing.

Both runs used seven fresh browser contexts per operation, one browser process,
a loopback HTTP server with `Cache-Control: no-store`, and uncompressed responses.
Browser/engine compilation caches may be warm. This measures local load/init and
operation costs, not cold device startup or deployed network latency. Gzip sizes
are computed separately and were not the HTTP transfer encoding.

| Metric | Monolithic baseline | Split |
| --- | ---: | ---: |
| WASM needed for reading, bytes | 5,514,021 | 355,365 |
| Same WASM, gzip bytes | 2,024,032 | 170,947 |
| Compatibility/tooling WASM, bytes | 5,514,021 | 5,510,789 |
| SDK import, reader scenario median ms | 4.80 | 4.70 |
| First `AIX.From`, median ms | 12.10 | 2.90 |
| Warm reader operation, median ms | 0.0735 | 0.0730 |
| First source check, median ms | 39.80 | 41.30 |
| Warm source check, median ms | 1.89 | 1.91 |
| First source pack, median ms | 30.60 | 32.10 |
| Warm source pack, median ms | 2.38 | 2.37 |

Reader WASM size falls by 93.6% raw and 91.6% gzip in this build. Warm operations
remain similar. First check/pack calls move module loading to that call and were
slightly slower in this run; no general speedup is claimed for those operations.

The fixture contains six source files (22,010 bytes) and produces a 2,252-byte
archive. Warm reading runs `resolveMetadata`, `getWidgets`, and `readFile` as
one operation, averaged over 200 iterations per context. Check/pack use ten warm
iterations; pack keeps default JS minification with `optimize: false`.
Raw samples and artifact sizes are tracked in `docs/benchmarks/wasm-monolithic.json`
and `docs/benchmarks/wasm-split.json`.

To repeat after building the npm CLI and Web SDK:

```bash
cd crates/aix-web
AIX_TEST_BROWSER_CHANNEL=chrome node scripts/benchmark-wasm.js /tmp/aix-wasm-measurement.json
```

Omit `AIX_TEST_BROWSER_CHANNEL` to use Playwright's installed Chromium. Set
`AIX_WEB_DIST` to benchmark another built distribution, or `AIX_BENCH_SAMPLES`
to change the sample count. Timings are evidence, not CI thresholds.

## Verification

The browser suite checks the reader dependency tree and WASM exports, blocks all
tooling requests while exercising the reader, and tests a failed tooling WASM
load followed by concurrent check/pack retry. It then verifies that the original
reader still returns the native metadata contract. Existing browser, Web Worker,
CLI, raw-WASM, resource-limit, and metadata parity tests retain their coverage.
