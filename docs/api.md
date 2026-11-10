# @yodaos-pkg/aix

`@yodaos-pkg/aix` is the browser-facing WebAssembly and TypeScript package for
reading, creating, optimizing, and inspecting AIX artifacts. The high-level SDK
targets modern browsers and Web Workers with ES modules, fetch, and WebAssembly.
The npm CLI uses a separate Node.js WASM build.

## Installation

~~~bash
npm install @yodaos-pkg/aix
~~~

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

const response = await fetch('/agents/example.aix');
const aix = await AIX.From(new Uint8Array(await response.arrayBuffer()));
console.log(aix.getTitle(), aix.getPages(), aix.getTools());
~~~

`AIX.From` initializes the WASM runtime and accepts complete archive bytes or a
browser File object.

Reading loads an independent reader WASM. Packing, optimization, and source
checks (including `AIX.From(data, limits, true)`) lazily load the full tooling
module; existing methods and direct WASM paths remain available. Self-host the complete generated distribution, including
both `pkg-reader/` and `pkg/`. See [WASM loading and measurements](/wasm) for
the build features, compatibility plan, tradeoffs, and measured costs.

## API Reference

This reference covers the public TypeScript exports in v0.12.0. The seven
static methods are asynchronous; instance methods are synchronous. Source checks
use `AIX.checkFromSource()` or `AIX.checkFromFiles()`; there is no `AIX.check()`
or instance `check()` method.

### AIX.From(data, limits?, check?) {#aix-from-data-limits}

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits, check?: boolean): Promise<AIX>
~~~

Creates an instance from a complete AIX archive. The promise rejects for an
invalid ZIP, invalid entry path, duplicate normalized entry, or exceeded budget.

The optional third parameter `check` defaults to `false`. When `true`, the
package contents go through the same source checks as `checkFromSource()` before
the instance is returned. Warnings alone pass. Any diagnostic with
`severity: 'error'` rejects with [AixCheckError](#aix-check-error), whose `report`
contains the complete `CheckReport` and whose `diagnostics` includes all errors
and warnings. Archive, extraction, and checker loading/collection failures retain
their original operation errors; those do not necessarily have a check report.

~~~typescript
import { AIX, AixCheckError } from '@yodaos-pkg/aix';

async function openCheckedPackage(file: File) {
  try {
    return await AIX.From(file, undefined, true);
  } catch (error) {
    if (error instanceof AixCheckError) console.table(error.diagnostics);
    throw error;
  }
}
~~~

Checking does not execute project code or verify runtime behavior. It adds work
to opening the package and retains extracted check inputs; archive limits still
apply and do not bound the checker's total memory use.

~~~typescript
const aix = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
~~~

These example budgets are inclusive and caller-selected. All central-directory
entries count, including directories and signature metadata. Metadata is checked
before extraction, and `readFile` bounds output while retaining CRC and size
verification. Unknown limit fields are rejected. `maxTotalBytes` limits the sum
of declared uncompressed sizes; it
does not count repeated reads. Each option must be a non-negative safe integer;
zero allows no entries or bytes. Omitted fields, `{}`, and the existing one-argument
call retain the unlimited compatibility policy. Set all three fields for
untrusted packages. These reader limits do not cover the input archive buffer,
parsed objects, caller-retained copies, or `AIX.optimize`.

Rust callers use `AixReader::new_with_limits(data, AixReadLimits { ... })`, with
`Option<u64>` fields `max_entries`, `max_entry_bytes`, and `max_total_bytes`.
`AixReadLimits::default()` and `AixReader::new(data)` remain unlimited. Direct
WASM consumers use `AixReaderWasm.with_limits(data, limits)` with the same camelCase
fields as the TypeScript SDK; `new AixReaderWasm(data)` keeps its existing policy.

### AIX.checkFromSource(files, options?)

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

Checks an in-memory source tree without creating an archive, emitting files, or
running project code. Paths are relative to the application root, with `app.json`
at that root. Collection normalizes paths, applies root/nested `.aixignore`
rules, and excludes rule files and `.aix/` caches at every depth.

Checks cover JSON/text validity, App/Page/Widget structure, component references
and cycles, JS/TS syntax (including `.mts`, `.cts`, and Ink `lang="ts"` scripts),
statically recognizable permission use, and Page `schema.data` declarations.
Schema checks use `$schema` (2020-12 by default), support local references, and
do not fetch external resources. They validate schema declarations, not runtime
data. TypeScript type checking, import resolution, compiler options, dynamic API
usage, and runtime behavior are outside this check.

~~~typescript
import { AIX, type AixInputFile, type CheckReport } from '@yodaos-pkg/aix';

const encode = (text: string) => new TextEncoder().encode(text);
const files: AixInputFile[] = [
  { path: 'app.json', data: encode('{"pages":[]}') },
  { path: 'main.ts', data: encode('const count: number = ;') },
];
const report: CheckReport = await AIX.checkFromSource(files, {
  onProgress(event) {
    if (event.type === 'check_finished') {
      console.log(event.check.id, event.check.status, event.check.detail);
    }
  },
});
for (const diagnostic of report.diagnostics) {
  console.log(diagnostic.path, diagnostic.line, diagnostic.column,
    diagnostic.severity, diagnostic.code, diagnostic.message);
}
const hasErrors = report.diagnostics.some(item => item.severity === 'error');
~~~

Source errors and warnings resolve to `CheckReport.diagnostics`; they do not
reject the promise. Check for `severity === 'error'` to decide whether to
continue. Collection failures such as invalid paths or malformed ignore rules,
WASM loading failures, and exceptions from `onProgress` reject the promise.
`CheckReport` has no success flag or checklist array. Per-category results are
available through `check_finished` progress events.

### AIX.checkFromFiles(files, options?)

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

Checks browser `File` objects, using `webkitRelativePath` or falling back to
`name`. When all relative paths share the selected directory prefix, that outer
directory is removed before delegating to `checkFromSource`. Read failures
reject the promise. For a directory picker:

~~~html
<input id="source" type="file" webkitdirectory multiple>
~~~

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector<HTMLInputElement>('#source')!;
input.addEventListener('change', async () => {
  const report = await AIX.checkFromFiles(Array.from(input.files ?? []));
  console.log(report.diagnostics);
});
~~~

### Check progress and browser responsiveness

`CheckOptions.onProgress` receives `loading_checker`, `checking_sources` (input
file count before collection), five `check_finished` events, and finally
`collecting_diagnostics` on successful completion. The five IDs are `json`,
`structure`, `syntax`, `permissions`, and `schemas`. Each result has a status,
error/warning counts, and optional `detail`; checks without applicable work or
blocked by invalid input may be `skipped`.

These events describe stages, not percentage completion. After initialization,
the browser check runs synchronously. Use a Web Worker to keep a progress UI
responsive. The npm CLI uses the same checker in a Node.js Worker; see
[`aix check`](/cli#aix-check-input-dir).

### AIX.pack(files, options?)

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

Packs already-normalized in-memory entries. The input must contain app.json and
must not contain VERSION or reserved META-INF/aix paths. This lower-level
entry point does not collect a source tree or apply `.aixignore` rules.

`buildId` defaults to a generated UUID v4 and becomes `VERSION`. `engine` uses
the explicit option, then `app.json.engine`, then `*`. All `.json` files are
validated and supported text is normalized to UTF-8. Standalone `.js`/`.ts`
files are minified even when `optimize` is omitted or `false`; this does not
transpile TypeScript to JavaScript.

Optional JSON/image optimization is disabled when `optimize` is omitted or
`false`. Passing `{}` enables defaults: `level: 2`, `json: true`, `png: true`,
and `jpeg: true`. Packing does not automatically run the full source checker.
`warnings` are nonfatal packaging diagnostics; use the check APIs for the full
static report.

Set `options.agentId` to the same normalized project ID across builds to write
manifest `agent_id`. The SDK does not create a filesystem identity file or
generate this ID automatically. Use a nonempty string of lowercase ASCII
letters, digits, `_`, and `-`, without a leading or trailing `-`.
It is independent of `buildId` / `VERSION` and is preserved
by `optimize`. `getAgentId()` returns this ID, returns `undefined` for legacy
packages without it, and throws for malformed manifest identities. Direct WASM
pack exports accept the ID as an optional final argument; reader exports expose
`get_agent_id()`. Rust uses `PackOptions.agent_id` and `AixReader::get_agent_id()`.

### AIX.packFromSource(files, options?)

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

Packs a source tree, normalizing paths, applying nested .aixignore rules, and
omitting ignore files and `.aix/` caches at any depth. Duplicate normalized
paths and invalid collection rules reject the promise. Use `pack` when those
steps were already performed. `onProgress` is available here and in
`packFromFiles`, not in `pack` or `optimize`; callback exceptions reject the call.

### AIX.packFromFiles(files, options?)

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

Converts browser File objects to source entries using webkitRelativePath when
available (otherwise `name`), then delegates to `packFromSource`.

Unlike `checkFromFiles`, this method preserves the outer directory in
`webkitRelativePath`. For an input selected with `webkitdirectory`, remove that
prefix yourself so that `app.json` is at the archive root, and call
`packFromSource`:

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector<HTMLInputElement>('#source')!;
const files = await Promise.all(Array.from(input.files ?? []).map(async file => ({
  path: file.webkitRelativePath.split('/').slice(1).join('/'),
  data: new Uint8Array(await file.arrayBuffer()),
})));
const packed = await AIX.packFromSource(files, { agentId: 'my-agent' });
~~~

### Progressive Assets {#progressive-assets}

Declare media in an ECMAScript module or an Ink script. The imported value is an
opaque `AssetRef` that you can pass to an Ink video or audio consumer.

~~~javascript
import clip from './assets/clip.mp4' with { type: 'video', progressive: 'prefer' };
~~~

Use `type: 'audio'` for audio. With the supported profile, AIX can remux ordinary
H.264 MP4 and AAC-LC M4A into fragmented MP4 without re-encoding. It also accepts
compatible prefragmented files. The default is to embed the source media.

To deliver fragments remotely, provide an `onProgressiveAsset` callback. The
callback decides whether to embed each candidate and calls `prepare()` to get
the initialization metadata and remote objects. Your uploader stores the
objects and returns a receipt.

~~~typescript
const packed = await AIX.packFromSource(files, {
  signal: controller.signal,
  async onProgressiveAsset(candidate, { prepare, signal }) {
    if (!candidate.progressiveSupport.supported) {
      return { delivery: 'embedded' };
    }

    const plan = await prepare();
    const receipt = await uploadToStorage(plan, signal);
    return { delivery: 'progressive', planId: plan.id, receipt };
  },
});
~~~

`uploadToStorage` is supplied by the caller. The receipt must cover every
prepared object with the matching ID, byte length, SHA-256, and a stable HTTPS
URL without credentials, a query string, or a fragment. AIX checks the
receipt before writing the package. An upload error, invalid receipt, or
abort rejects the build. Use `packed.progressiveAssets` to see the selected
delivery mode and any retained source bytes.

To try the callback, open [Build in Play](/play?mode=build), choose a project
directory containing a progressive media declaration, and click **Build and
download**. Build prepares fragments for supported media and simulates the
upload receipt in the browser. It embeds media whose profile is unsupported.
The downloaded package uses placeholder URLs for simulated uploads, which you
can view in Inspect.

### AIX.optimize(data, options?)

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

Writes an optimized copy of an existing package. Optimization changes bytes, so
an existing signature is not retained. `VERSION` and manifest `agent_id` are
preserved. Omitted options use the optimization defaults above. Report byte
totals describe uncompressed entry data, not the ZIP download size.

### Instance methods

~~~typescript
list(): AixEntry[]
readFile(name: string): Uint8Array
getVersion(): string | undefined
getAgentId(): string | undefined
supportsEngine(currentVersion: string): boolean
getTitle(): string | undefined
getPermissions(): string[]
getPages(): PageInfo[]
resolveMetadata(): MetadataResolution
getWidgets(locale?: string): WidgetInfo[]
getTools(): Tool[]
~~~

| Method | Behavior |
| --- | --- |
| `list()` | Lists normalized entry names and compressed/uncompressed byte sizes. |
| `readFile(name)` | Reads an exact entry name and verifies its CRC and size; throws for missing/corrupt entries or exceeded budgets. |
| `getVersion()` | Reads `VERSION`; missing, unreadable, or non-UTF-8 data returns `undefined`. |
| `getAgentId()` | Reads manifest `agent_id`; absent identity returns `undefined`, malformed manifest/identity throws. |
| `supportsEngine(version)` | Tests the manifest semver range; throws when the manifest/range is missing or invalid, or the supplied version is invalid. |
| `getTitle()` | Reads `app.json.window.navigationBarTitleText`; missing or invalid configuration returns `undefined`. |
| `getPermissions()` | Reads `app.json.permissions` as a string array, preserving order, duplicates, and custom names. An omitted or empty list returns `[]`; missing/unreadable/invalid `app.json` or a non-string-array value throws. |
| `getPages()` | Resolves page metadata with the tolerant behavior described below. |
| `resolveMetadata()` | Returns successfully resolved pages/tools and resource diagnostics. |
| `getWidgets(locale?)` | Validates Widget entries and optionally overlays `displayName`/`description` from `app.<locale>.json`; invalid configuration, locale files, or missing entries throw. |
| `getTools()` | Derives OpenAI-compatible function records from the tolerant page list. |

`getPermissions()` returns declarations, not runtime or device authorization state,
and does not prompt for access. For example:

~~~typescript
const permissions = aix.getPermissions(); // e.g. ['GEOLOCATION', 'CAMERA']
~~~

These synchronous methods can surface errors when reading package resources.
By default, `AIX.From()` validates the archive structure without eagerly reading
every entry. The optional source check does not verify a publisher's cryptographic
signature. Signature verification is available through the Rust reader; it is not a public TypeScript method.
See the [Specification](/spec) for page, layout, Widget, and tool semantics.

### AixCheckError {#aix-check-error}

An exported `Error` subclass thrown when the optional `From` check reports errors.
Import it as a value for `instanceof` checks, not with `import type`.

| Property | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | `"AixCheckError"`. |
| `message` | `string` | A brief check-failure summary. |
| `report` | `CheckReport` | The complete check report. |
| `diagnostics` | `CheckDiagnostic[]` | The same array as `report.diagnostics`, including warnings and source locations. |

Warnings alone do not throw. To retrieve a report independently, call
`checkFromSource()` or `checkFromFiles()`.

### Metadata resolution and diagnostics

`getPages()` and `getTools()` retain their tolerant behavior: invalid page lists
return empty arrays, and missing resources or malformed page metadata can produce
default page/tool records. Page, Widget, and title fields in `app.json` are parsed
independently, so an invalid Widget field does not hide valid pages. `getWidgets()`
continues to throw for invalid Widget configuration or missing Widget entries.

Use `resolveMetadata()` when callers need to distinguish successful resolution
from defaults:

~~~typescript
const result = aix.resolveMetadata();
for (const diagnostic of result.diagnostics) {
  console.error(diagnostic.path, diagnostic.message);
}
// Only successfully resolved pages contribute callable tools.
const tools = result.tools;
// To require complete success, decline the result if diagnostics is nonempty.
~~~

The synchronous report contains `pages`, `tools`, and `diagnostics`. Failures are
returned as diagnostics rather than thrown. A valid `pages: []` has no diagnostics;
missing, unreadable, or invalid `app.json.pages` produces an `app.json` diagnostic.
Each failed page contributes its first resource/metadata error and is excluded
from both lists, while other pages retain declaration order. Tool targeting uses
the same rules as `getTools()`, applied to the successfully resolved page list.

Resolution requires a readable `.ink` entry, or both `.json` and `.wxml` for a
traditional page. Present resources must pass ZIP CRC/size checks and UTF-8
decoding; page JSON and complete Ink `<script def>` blocks must deserialize.
A broken `.ink` entry does not fall back to multi-file pages. Optional definitions,
schemas, and styles may be absent; `.wcss` takes precedence over `.wxss`. Widget
and title fields are outside this report. This API does not replace source checks
for Ink structure, component references, or JSON Schema validity, and does not
validate runtime behavior. Packing validation is unchanged.

Rust uses `AixReader::resolve_metadata() -> MetadataResolution`; direct WASM
consumers use `reader.resolve_metadata()` with the same report shape.

## Type Reference

~~~typescript
interface AixInputFile {
    path: string;
    data: Uint8Array;
}

interface AixReadLimits {
    maxEntries?: number;
    maxEntryBytes?: number;
    maxTotalBytes?: number;
}

interface PackOptions {
    buildId?: string;
    agentId?: string;
    engine?: string;
    optimize?: false | OptimizeOptions;
    signal?: AbortSignal;
    onProgressiveAsset?: (
        candidate: ProgressiveAssetCandidate,
        context: ProgressiveAssetPackContext,
    ) => ProgressiveAssetDecision | Promise<ProgressiveAssetDecision>;
}

interface OptimizeOptions {
    level?: 1 | 2 | 3;
    json?: boolean;
    png?: boolean;
    jpeg?: boolean;
}

interface PackFromSourceOptions extends PackOptions {
    onProgress?: (event: PackProgressEvent) => void;
}

interface PackResult {
    data: Uint8Array;
    report: OptimizeReport;
    warnings: string[];
    progressiveAssets: ProgressiveAssetReport[];
}

interface OptimizeReport {
    files: FileOptimizeReport[];
    original_size: number;
    output_size: number;
    saved_bytes: number;
}

interface FileOptimizeReport {
    path: string;
    status: 'optimized' | 'unchanged' | 'skipped';
    original_size: number;
    output_size: number;
    saved_bytes: number;
    converted_to_utf8: boolean;
}
~~~

### PackProgressEvent

~~~typescript
type PackProgressEvent = {
    type: 'transferring_files_to_wasm';
} | {
    type: 'collecting_source_inputs';
} | {
    type: 'resolving_engine';
} | {
    type: 'preparing_files';
} | {
    type: 'file_finished';
    report: FileOptimizeReport;
} | {
    type: 'finalizing_archive';
};
~~~

Pack callbacks report transfer, source collection, engine resolution, file
preparation, each completed file, and archive finalization. `file_finished`
carries that entry's `FileOptimizeReport`, including the generated `VERSION`
entry. These are synchronous stage notifications, not percentage completion.

### Source check types

~~~typescript
interface CheckDiagnostic {
    code: string;
    severity: 'error' | 'warning';
    path: string;
    line: number;
    column: number;
    message: string;
}

interface CheckReport {
    diagnostics: CheckDiagnostic[];
}

interface CheckOptions {
    onProgress?: (event: CheckProgressEvent) => void;
}

interface CheckResult {
    id: 'json' | 'structure' | 'syntax' | 'permissions' | 'schemas';
    label: string;
    status: 'passed' | 'warning' | 'failed' | 'skipped';
    errors: number;
    warnings: number;
    detail?: string;
}

type CheckProgressEvent = {
    type: 'loading_checker';
} | {
    type: 'checking_sources';
    fileCount: number;
} | {
    type: 'check_finished';
    check: CheckResult;
} | {
    type: 'collecting_diagnostics';
};
~~~

### Package and derived models

~~~typescript
interface AixEntry {
    name: string;
    size: number;
    compressed_size: number;
}

type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;

interface JsonObject {
    [key: string]: JsonValue;
}

type JsonSchema = JsonValue;

interface PageConstraint {
    width: number;
    height: number;
}

interface PageInfo {
    name: string;
    title: string | null;
    description: string | null;
    data_schema: JsonSchema;
    size: PageConstraint;
}

interface MetadataDiagnostic {
    path: string;
    message: string;
}

interface MetadataResolution {
    pages: PageInfo[];
    tools: Tool[];
    diagnostics: MetadataDiagnostic[];
}

type WidgetPlacement = 'persistent' | 'overlay';

interface WidgetInfo {
    path: string;
    family: string;
    placement: WidgetPlacement;
    displayName: string | null;
    description: string | null;
}

type ToolTarget = '_current' | '_blank';

interface FunctionDefinition {
    name: string;
    description: string | null;
    parameters: JsonSchema;
}

interface Tool {
    type: string;
    target: ToolTarget;
    layout: PageConstraint;
    function: FunctionDefinition;
}
~~~

Page `size` and tool `layout` contain numeric width/height constraints. Tools
currently emit `type: "function"` and `_current`/`_blank` targets; unprefixed
names are deserialization aliases only. `JsonSchema` retains arbitrary JSON,
including boolean schemas and unvalidated legacy values; use source checking
for schema validation and narrow `JsonValue` before reading object properties.

All metadata fields above are required. Missing strings and nested JSON nulls
are serialized as `null` in raw WASM, SDK getters, and `resolveMetadata()`, as in
native Rust JSON. Earlier WASM builds used `undefined` for these values; change
explicit metadata `=== undefined` checks to `=== null` when migrating. Standalone
`getTitle()` and `getVersion()` continue to return `string | undefined`.

## Browser Example

~~~typescript
const input = document.querySelector<HTMLInputElement>('#package')!;
input.addEventListener('change', async () => {
  const file = input.files?.[0];
  if (!file) return;
  const aix = await AIX.From(file);
  console.log(aix.getVersion());
  console.table(aix.list());
  console.log(aix.getPages(), aix.getTools());
});
~~~

## Play

The official browser lab is available at [/play](/play). It displays an
artifact's title, version, pages, generated tools, archive entries, and raw
entry contents.

## Build From Source

~~~bash
cd crates/aix-web
npm install
npm run build
~~~

The generated distribution is written to crates/aix-web/dist. Applications
should consume the published package rather than generated internals.

## License

MIT
