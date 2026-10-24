# @yodaos-pkg/aix

`@yodaos-pkg/aix` is the browser-facing WebAssembly and TypeScript package for
reading, creating, optimizing, and inspecting AIX artifacts. It runs in modern
browsers and other environments with WebAssembly support.

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

## API Reference

### AIX.From(data, limits?)

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits): Promise<AIX>
~~~

Creates an instance from a complete AIX archive. The promise rejects for an
invalid ZIP, invalid entry path, duplicate normalized entry, or exceeded budget.

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
verification. `maxTotalBytes` limits the sum of declared uncompressed sizes; it
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

### AIX.pack(files, options?)

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

Packs already-normalized in-memory entries. The input must contain app.json and
must not contain VERSION or reserved META-INF/aix paths.

### AIX.packFromSource(files, options?)

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

Packs a source tree, normalizing paths, applying nested .aixignore rules, and
omitting ignore files. Use pack when those steps were already performed.

### AIX.packFromFiles(files, options?)

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

Converts browser File objects to source entries using webkitRelativePath when
available, then delegates to packFromSource.

### AIX.optimize(data, options?)

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

Writes an optimized copy of an existing package. Optimization changes bytes, so
an existing signature is not retained.

### Instance methods

~~~typescript
list(): AixEntry[]
readFile(name: string): Uint8Array
getVersion(): string | undefined
supportsEngine(currentVersion: string): boolean
getTitle(): string | undefined
getPages(): PageInfo[]
resolveMetadata(): MetadataResolution
getWidgets(locale?: string): WidgetInfo[]
getTools(): Tool[]
~~~

`list` returns normalized names and compressed/uncompressed sizes. `readFile`
returns verified uncompressed bytes. `getVersion` reads VERSION; `getTitle`
reads app.json.window.navigationBarTitleText. `supportsEngine` checks the
manifest semver range. `getPages` resolves Ink or multi-file pages. `getWidgets`
validates widget Ink entries and optionally applies locale overlays. `getTools`
derives OpenAI-compatible function records. See the [Specification](/spec) for
page, schema, layout, and tool semantics.

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
for full Ink syntax, component references, or JSON Schema validity, and does not
validate runtime behavior. Packing validation is unchanged.

Rust uses `AixReader::resolve_metadata() -> MetadataResolution`; direct WASM
consumers use `reader.resolve_metadata()` with the same report shape.

## Type Reference

~~~typescript
interface MetadataDiagnostic { path: string; message: string }
interface MetadataResolution {
  pages: PageInfo[];
  tools: Tool[];
  diagnostics: MetadataDiagnostic[];
}

interface AixInputFile { path: string; data: Uint8Array }

interface PackOptions {
  buildId?: string;
  engine?: string;
  optimize?: false | OptimizeOptions;
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
type PackProgressEvent =
  | { type: 'transferring_files_to_wasm' }
  | { type: 'collecting_source_inputs' }
  | { type: 'resolving_engine' }
  | { type: 'preparing_files' }
  | { type: 'file_finished'; report: FileOptimizeReport }
  | { type: 'finalizing_archive' };
~~~

### Package and derived models

~~~typescript
interface AixEntry { name: string; size: number; compressed_size: number }

interface PageInfo {
  name: string;
  title?: string;
  data_schema: unknown;
}

interface WidgetInfo {
  path: string;
  family: string;
  placement: 'persistent' | 'overlay';
  displayName?: string | null;
  description?: string | null;
}

interface Tool {
  type: string;
  function: { name: string; description?: string; parameters: unknown };
}
~~~

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

## Package Lab

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
