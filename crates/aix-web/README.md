# @yodaos-pkg/aix

`@yodaos-pkg/aix` is a library for reading and managing **.aix** (AI eXecutable) packages in the web environment using WASM. The `.aix` format is an extension of the [Open Agent Format (OAF)](https://openagentformat.com/spec.html), designed to package AI agents with rich UI/UX capabilities powered by Ink Mini Programs.

## Installation

```bash
npm install @yodaos-pkg/aix
```

## Quick Start

Here is a quick example of how to load an `.aix` package:

```typescript
import { AIX } from '@yodaos-pkg/aix';

// Option 1: Load from a URL
async function fromUrl() {
  const response = await fetch('https://example.com/my-agent.aix');
  const buffer = await response.arrayBuffer();
  const aix = await AIX.From(new Uint8Array(buffer));
  console.log('App Title:', aix.getTitle());
}

// Option 2: Load from a File object (e.g., from an <input type="file">)
async function fromFile(file: File) {
  const aix = await AIX.From(file);
  console.log('App Title:', aix.getTitle());
}
```

## WASM Loading

By default, `AIX.From()` and all instance methods initialize a separate reader-only WASM.
They do not load image codecs, script minification, or source/schema checking.
The optional `AIX.From(data, limits, true)` check also loads the full tooling module.
The first pack, optimize, or source-check call lazily loads the full module;
concurrent tooling calls share initialization. Existing reader instances remain
valid after tooling loads, and tooling-only calls do not fetch reader WASM.

The generated distribution includes `pkg-reader/` and `pkg/`; self-host both
with their adjacent JS/WASM files. The existing raw `pkg/aix_web.js` exports
remain complete and the npm CLI keeps its synchronous full Node.js bundle.
Rust builds default to the `tooling` feature; `--no-default-features` produces
reader exports without the packer dependency. `npm run build` builds both web
artifacts; `npm run build:wasm:reader` builds only the reader artifact.

Local measurements reduced reader WASM from 5,514,021 to 355,365 bytes. Mixed
reader/tooling workflows load both binaries and retain two module memories.
See the repository's `docs/wasm.md` for baseline samples, timing limits, the
compatibility plan, and reproduction commands.

## Archive Resource Limits

Pass caller-selected budgets when reading untrusted packages:

```typescript
const aix = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
```

All limits are inclusive non-negative safe integers. Directories and signature
metadata count too. Metadata is rejected before extraction if over budget;
extraction also bounds output and retains CRC and size checks. `maxTotalBytes`
is the sum of declared uncompressed sizes, not a cumulative read budget.
Zero allows no entries or bytes. Omitted fields and the original `AIX.From(data)`
call remain unlimited for compatibility; set all three limits for untrusted
packages. The input archive buffer, parsed objects, and retained copies need
separate budgets. This policy applies to readers, not `AIX.optimize`.

Direct WASM consumers can call `AixReaderWasm.with_limits(data, limits)` using
the same fields. The existing `new AixReaderWasm(data)` constructor is unchanged.

## Open and Check

Pass `true` as the third `AIX.From` argument to run the source checker before
receiving the package. The second argument remains the optional read limits;
pass `undefined` there when no limits are needed.

```typescript
import { AIX, AixCheckError } from '@yodaos-pkg/aix';

async function openCheckedPackage(file: File) {
  try {
    return await AIX.From(file, undefined, true);
  } catch (error) {
    if (error instanceof AixCheckError) console.table(error.diagnostics);
    throw error;
  }
}
```

Warnings alone pass. Error diagnostics reject with the exported `AixCheckError`
(`name: 'AixCheckError'`), an `Error` subclass with the complete `CheckReport` in
`report` and the same diagnostic array in `diagnostics`, including warnings.
The check uses `checkFromSource` on the package's source files without running
project code. It adds work and retains extracted check inputs; read limits still
apply but do not bound total checker memory. Archive, extraction, loading, and
collection failures retain their original operation errors rather than a check
report. Omitting the third argument or passing `false` preserves the existing
reader-only behavior.

## Check Source Files

The Web SDK and npm CLI use the same complete in-memory check engine:

```typescript
import { AIX, type CheckReport } from '@yodaos-pkg/aix';

const encode = (source: string) => new TextEncoder().encode(source);
const report: CheckReport = await AIX.checkFromSource([
  { path: 'app.json', data: encode('{"pages":[]}') },
  { path: 'main.ts', data: encode('const count: number = ;') },
], { onProgress: (event) => console.log(event.type) });
console.log(report.diagnostics); // AIX029: invalid TypeScript syntax at main.ts:1

// Files from an <input type="file" webkitdirectory> selection also work.
const fileReport = await AIX.checkFromFiles(Array.from(input.files ?? []));
```

Checks cover JSON/config validity, Page/Widget structure, component references
and cycles, JS/TS syntax, statically recognizable permissions, and Page JSON Schemas. Each diagnostic has `code`, `severity`, `path`,
`line`, `column`, and `message`. Source errors are returned in the report;
invalid input paths or collection failures reject the promise.

`checkFromSource` takes paths relative to the application root. `checkFromFiles`
uses `webkitRelativePath`, removing the selected directory's outer folder.
Both apply nested `.aixignore` rules and exclude local `.aix` caches. The shared
check adapter loads only when checking is first requested. It uses the WASM
engine without a Node filesystem, Node polyfills, or dependency downloads.
The same API also works in a Web Worker.

Both methods accept `CheckOptions` with an optional `onProgress` callback.
`CheckProgressEvent` reports checker loading, source/schema validation (input
file count), and diagnostic collection. These are stage notifications, not a
percentage estimate. Browser checks are synchronous after loading; run the API
in a Web Worker to keep a browser progress display responsive.

`check_finished` events carry an exported `CheckResult` with `id`, `label`,
`status` (`passed`, `warning`, `failed`, or `skipped`), `errors`, `warnings`, and
an optional `detail` explaining incomplete/skipped checks. The CLI uses the same
results for its final checklist. The returned `CheckReport` remains a diagnostic
report, so JSON callers do not need to parse terminal output.

JavaScript and TypeScript are syntax-checked, including `.ts`, `.mts`, `.cts`,
and Ink `<script lang="ts">` blocks. Inline diagnostics retain their original
Ink file locations. TypeScript semantic, import-resolution, and compiler-option
checks are currently excluded. No type environment is loaded, and checks do not
emit or run code.

Page `schema.data` is validated against `$schema` (2020-12 by default), including
nested keywords, patterns, and local references. Missing/null schemas remain
accepted. External schema resources are not fetched. These checks validate
schema declarations, not runtime data; packing and reader semantics are unchanged.

## Using Play

The official browser lab now lives in the docs site at `/play`.

If you are developing inside this repository, install dependencies in `crates/aix-web` first:

```bash
npm install
```

1. Build the local `aix-web` package:

```bash
npm run build
```

This step builds the `aix-web` WebAssembly bundle and TypeScript output into `dist/`, which is what the docs-integrated Package Lab loads.

2. Install the docs dependencies:

```bash
cd ../../docs
npm install
```

3. Start the docs site:

```bash
npm run dev
```

4. Open the local VitePress URL shown in the terminal, then navigate to `/play`, usually:

```text
http://localhost:5173/aix/play
```

5. Upload an `.aix` file in the page to inspect:

- package title and version
- parsed `pages`
- generated OpenAI-compatible `tools`
- package file list and raw file contents

This is useful for verifying whether `app.json`, page schemas, and `getTools()` output match expectations while developing or debugging an AIX package inside the official docs experience.

### Pack and Optimize

The Web package includes the same in-memory packer and pure Rust resource
optimizer used by the npm CLI:

```ts
const packed = await AIX.pack(
  [{ path: "app.json", data: new TextEncoder().encode('{"pages":[]}') }],
  { optimize: { level: 2 } },
);

const optimized = await AIX.optimize(packed.data, { level: 3 });
```

Both methods return the generated `Uint8Array` and a structured per-file report.

Use `packFromSource` when the input represents an unpacked application tree.
It applies the same path normalization and nested `.aixignore` behavior as the
native and npm CLIs, and can report structured progress:

```ts
const packed = await AIX.packFromSource(
  [
    { path: ".aixignore", data: new TextEncoder().encode("*.tmp\n") },
    { path: "app.json", data: new TextEncoder().encode('{"pages":[]}') },
    { path: "scratch.tmp", data: new Uint8Array() },
  ],
  {
    optimize: { level: 2 },
    onProgress(event) {
      console.log(event.type);
    },
  },
);
```

Use `pack` only when the caller has already normalized and filtered its input
files and wants to invoke the low-level packer directly.

## API Reference

### `AIX` Class

The main class to interact with an AIX package.

#### `static async From(data: Uint8Array | File, limits?: AixReadLimits, check?: boolean): Promise<AIX>`
Initializes the WASM module and creates an AIX instance from the given `.aix` file content. It supports both `Uint8Array` and the standard Web `File` object.
`check` defaults to `false`. Set it to `true` to check before returning and reject
with `AixCheckError` when errors are reported; see [Open and Check](#open-and-check).
```typescript
// From Uint8Array
const aix = await AIX.From(new Uint8Array(buffer));

// From File (Web API)
const aix = await AIX.From(file);
```

#### `list(): AixEntry[]`
Lists all files in the AIX package.
```typescript
const files = aix.list();
// [{ name: "app.json", size: 123, compressed_size: 45 }, ...]
```

#### `readFile(name: string): Uint8Array`
Reads the raw content of a specific file from the package.
```typescript
const content = aix.readFile('app.json');
const text = new TextDecoder().decode(content);
```

#### `getVersion(): string | undefined`
Returns the version metadata from the `VERSION` file in the package.
```typescript
const version = aix.getVersion();
```

#### `getTitle(): string | undefined`
Extracts the navigation bar title from the `app.json`.
```typescript
const title = aix.getTitle();
```

#### `getPermissions(): string[]`
Returns the permission names declared in `app.json.permissions`, preserving their
order, duplicates, and custom names. An omitted or empty list returns `[]`.
Throws if `app.json` is missing, unreadable, invalid, or `permissions` is not an
array of strings. This reads declarations without querying authorization or
prompting for access.
```typescript
const permissions = aix.getPermissions();
// e.g. ['GEOLOCATION', 'CAMERA']
```

#### `getPages(): PageInfo[]`
Returns information about all pages defined in the package, including their paths, titles, and data schemas.
```typescript
const pages = aix.getPages();
/*
[{
  name: "pages/index/index",
  title: "Home",
  description: null,
  data_schema: { ... },
  size: { width: 480, height: 168 }
}]
*/
```

#### `getWidgets(locale?): WidgetInfo[]`
Returns widget declarations after verifying that every path resolves to an `.ink` entry in the package. When `locale` is provided, merges `displayName` and `description` from the matching `app.<locale>.json` overlay. Throws when a declared entry is missing or the selected locale file is invalid.
```typescript
const widgets = aix.getWidgets('en-US');
// [{ path: "widgets/clock/index", family: "1x1", placement: "persistent", displayName: "Clock", description: "Shows the current time." }]
```

#### `resolveMetadata(): MetadataResolution`

Returns `{ pages, tools, diagnostics }` synchronously. Missing/unreadable resources,
UTF-8 errors, and malformed page metadata produce `{ path, message }` diagnostics,
one per failed page. Both result lists put the default page first when it resolves
and keep the other pages in declaration order. A valid empty page list has no diagnostics;
invalid/unreadable `app.json.pages` produces an `app.json` diagnostic.

```typescript
const result = aix.resolveMetadata();
console.log(result.diagnostics);
const tools = result.tools; // only successfully resolved pages
// For all-or-nothing acceptance, require result.diagnostics.length === 0.
```

This API requires readable `.ink`, or both `.json` and `.wxml` for traditional
pages, and checks ZIP integrity and UTF-8 for resources it reads. It deserializes
page JSON and complete Ink `<script def>` blocks. Optional definitions, schemas,
and styles may be absent; present styles must be readable (`.wcss` before `.wxss`).
A broken Ink entry never falls back to traditional files. Use source checking
for full syntax, component, and JSON Schema validation. Runtime execution and
packing validation are unchanged.

`getPages()`/`getTools()` remain tolerant and can return default metadata/tools
even for missing resources. Page, Widget, and title fields are parsed separately;
invalid Widget fields no longer hide valid pages. `getWidgets()` still reports
Widget errors. Direct WASM consumers use `reader.resolve_metadata()`.

Exported `MetadataResolution` has `pages: PageInfo[]`, `tools: Tool[]`, and
`diagnostics: MetadataDiagnostic[]`; `MetadataDiagnostic` has `path` and `message`
string fields.

#### `getTools(): Tool[]`
Generates OpenAI-compatible tools from page metadata. The default page (`entryPagePath` or `pages[0]`) comes first with `_blank`. Other pages can declare `target` in page JSON or `<script def>`. Tools emit `_blank` or `_inline` and accept `_current` as an input alias for `_inline`.
```typescript
const tools = aix.getTools();
/*
[{
  type: "function",
  target: "_blank",
  layout: { width: 480, height: 168 },
  function: {
    name: "pages/index/index",
    description: "Home",
    parameters: { ... }
  }
}]
*/
```

## Data Types

### `AixEntry`
```typescript
interface AixEntry {
  name: string;
  size: number;
  compressed_size: number;
}
```

### JSON and layout types
```typescript
type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;
interface JsonObject { [key: string]: JsonValue }
type JsonSchema = JsonValue;
interface PageConstraint { width: number; height: number }
type ToolTarget = '_inline' | '_blank';
type WidgetPlacement = 'persistent' | 'overlay';
```

`JsonSchema` retains arbitrary JSON, including boolean schemas and unvalidated
legacy values. Use `AIX.checkFromSource()` for schema validation. Page `size`
and tool `layout` both contain numeric width/height constraints.

### `PageInfo`
```typescript
interface PageInfo {
  name: string;
  title: string | null;
  description: string | null;
  data_schema: JsonSchema;
  size: PageConstraint;
}
```

### `WidgetInfo`
```typescript
interface WidgetInfo {
  path: string;
  family: string;
  placement: WidgetPlacement;
  displayName: string | null;
  description: string | null;
}
```

### `Tool`
OpenAI compatible tool format.
```typescript
interface Tool {
  type: string;
  target: ToolTarget;
  layout: PageConstraint;
  function: FunctionDefinition;
}
interface FunctionDefinition {
  name: string;
  description: string | null;
  parameters: JsonSchema;
}
```

Metadata fields above are always present. Missing strings and nested JSON null
values now remain `null` through raw WASM, SDK getters, and `resolveMetadata()`,
matching native Rust JSON. Earlier WASM builds returned `undefined` for these
values; update explicit `=== undefined` checks to `=== null` when migrating
metadata consumers. Standalone `getTitle()` and `getVersion()` retain their
`string | undefined` return type. Tools currently emit `type: "function"` and
underscore-prefixed targets; `current`/`blank` are input aliases only.

## Build

To build the library for distribution:

```bash
npm run build
```

The output will be generated in the `dist` directory, including WASM binaries and TypeScript definitions.

## License

MIT

## Progressive Assets

Declare the asset in an ESM module or Ink script.

```js
import video from './assets/clip.mp4' with {
  type: 'video',
  progressive: 'prefer',
};
```

The imported value is an opaque `AssetRef`. Pass it to Ink's video or audio consumer. The CLI and Web SDK embed complete assets by default. A Web
pack caller can choose remote delivery after it stores all prepared objects.

```ts
const packed = await AIX.packFromSource(files, {
  signal: controller.signal,
  async onProgressiveAsset(candidate, context) {
    if (!candidate.progressiveSupport.supported || !shouldUpload(candidate)) {
      return { delivery: 'embedded' };
    }

    const plan = await context.prepare();
    const receipt = await uploadVerifiedObjects(plan.remoteObjects, {
      assetVersion: plan.assetVersion,
      signal: context.signal,
    });
    return { delivery: 'progressive', planId: plan.id, receipt };
  },
});
```

The caller owns upload policy, credentials, and cleanup of uploads left
unreferenced by a failed pack. Return a receipt only after durable storage has
verified every object. Receipt URLs must be final, immutable HTTPS locators
without credentials, query strings, or fragments. A rejected callback, invalid
receipt, or abort fails the pack. `packed.progressiveAssets` reports the
selected delivery mode and any source bytes retained for ordinary imports.

The media profile accepts ordinary MP4/M4A and prepared fMP4 with
finite H.264 video, AAC-LC audio, or both. The packer copies encoded samples
into keyframe-aligned fragments and preserves a single continuous rate-one
edit per track. Incompatible codecs or timelines can be converted upstream
or embedded. The Ink runtime must support the selected profile.

The browser integration test runs a local HTTPS Studio stand-in. It receives
object bytes over HTTP PUT, verifies length and SHA-256 on disk, and issues
receipts only after the entire inventory is present. Set `INK_WEB_DIST` to a
built Ink Web SDK to also test browser playback and offline reopening through the same object server.

In [Play's Build view](https://yodaos-project.github.io/aix/play?mode=build),
select a project directory and choose **Build and download**. Play calls
`onProgressiveAsset` for each declared asset and simulates the upload for
supported media. The downloaded package contains placeholder URLs you can
view in Inspect.

```sh
npm run build
AIX_TEST_BROWSER_CHANNEL=chrome node scripts/test-progressive.js
```
