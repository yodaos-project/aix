const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const ts = require('typescript');

exports.checkDeclarations = function checkDeclarations(dist) {
  const native = spawnSync('cargo', ['test', '--quiet', '-p', 'aiui-aix-web', '--lib',
    'tooling::tests::metadata_contract_fixture', '--', '--exact', '--nocapture'], {
    cwd: path.resolve(__dirname, '../../..'), encoding: 'utf8',
  });
  if (native.error) throw native.error;
  assert.equal(native.status, 0, native.stderr);
  const marker = 'AIX_METADATA_CONTRACT=';
  const output = native.stdout.split('\n').find((line) => line.startsWith(marker));
  assert(output, 'Native metadata contract test must emit its serialized models');
  const fixture = JSON.parse(output.slice(marker.length));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aix-metadata-contract-'));
  try {
    const generated = path.join(root, 'native-output.ts');
    fs.writeFileSync(generated, `import type { PageInfo, Tool, WidgetInfo, MetadataResolution, ToolTarget, WidgetPlacement } from ${JSON.stringify(path.join(dist, 'index.js'))};
      const output = ${JSON.stringify(fixture.expected, null, 2)} satisfies {
        pages: PageInfo[]; tools: Tool[]; widgets: WidgetInfo[]; metadata: MetadataResolution;
      };
      const targets = ${JSON.stringify(fixture.targets)} as const satisfies readonly ToolTarget[];
      const placements = ${JSON.stringify(fixture.placements)} as const satisfies readonly WidgetPlacement[];
      type Assert<T extends true> = T;
      type AllTargets = Assert<ToolTarget extends typeof targets[number] ? true : false>;
      type AllPlacements = Assert<WidgetPlacement extends typeof placements[number] ? true : false>;
    `);
    const consumer = path.join(root, 'consumer.ts');
    fs.writeFileSync(consumer, fs.readFileSync(path.join(__dirname, 'metadata-types.ts'), 'utf8')
      .replace("'../dist/index.js'", JSON.stringify(path.join(dist, 'index.js'))));
    const program = ts.createProgram([generated, consumer], {
      noEmit: true, strict: true, exactOptionalPropertyTypes: true, skipLibCheck: false,
      target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      lib: ['lib.es2022.d.ts', 'lib.dom.d.ts', 'lib.esnext.disposable.d.ts'],
    });
    const diagnostics = ts.getPreEmitDiagnostics(program);
    assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCanonicalFileName: (file) => file, getCurrentDirectory: () => process.cwd(),
      getNewLine: () => '\n',
    }));
    console.log('ok - published SDK declarations match native Rust metadata and reject invalid consumers');
    return fixture;
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
};

exports.checkBrowser = async function checkBrowser(page, fixture) {
  const result = await page.evaluate(async ({ files }) => {
    const { AixReaderWasm } = await import('./pkg/aix_web.js');
    const { data } = await window.AIX.pack(files.map(({ path, data }) => ({ path, data: new Uint8Array(data) })), {
      buildId: 'metadata-contract', optimize: false,
    });
    const sdk = await window.AIX.From(data);
    const raw = new AixReaderWasm(data);
    try {
      return {
        sdk: { pages: sdk.getPages(), tools: sdk.getTools(), widgets: sdk.getWidgets(), metadata: sdk.resolveMetadata() },
        raw: { pages: raw.get_pages(), tools: raw.get_tools(), widgets: raw.get_widgets(), metadata: raw.resolve_metadata() },
        titleMissing: sdk.getTitle() === undefined,
      };
    } finally {
      raw.free();
    }
  }, fixture);
  assert.deepEqual(result.sdk, fixture.expected, 'SDK metadata must match native JSON, including nested nulls');
  assert.deepEqual(result.raw, fixture.expected, 'Raw WASM metadata must match native JSON');
  assert.equal(result.titleMissing, true, 'The standalone optional-string getter retains undefined');
  console.log('ok - SDK and raw WASM preserve native metadata fields, enums, and nested JSON nulls');
};
