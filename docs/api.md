---
outline: [2, 4]
---

# AIX Web SDK {#yodaos-pkg-aix}

Use `@yodaos-pkg/aix` in a web app to check an AI agent project, build a
`.aix` file, or read a package someone already built. An AIX package is a ZIP
archive that holds the agent's files and metadata.

You can try these operations without code in [Play](/play). For terminal
commands, see the [CLI guide](/cli). For method signatures and types, jump to
the [API reference](#api-reference).

## Quick start {#quick-start}

### Install the SDK {#installation}

Run this in your frontend project.

~~~bash
npm install @yodaos-pkg/aix
~~~

### Build and open your first package {#build-and-open-your-first-package}

Paste this JavaScript into your app's module script. It creates a minimal
project, checks it, builds a package, and opens that package again.

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const files = [{
  path: 'app.json',
  data: new TextEncoder().encode(JSON.stringify({
    pages: [],
    window: { navigationBarTitleText: 'My first agent' },
  })),
}];

const report = await AIX.checkFromSource(files);
if (report.diagnostics.some(item => item.severity === 'error')) {
  console.table(report.diagnostics);
  throw new Error('Fix the source errors before packing.');
}

const result = await AIX.packFromSource(files);
const pkg = await AIX.From(result.data);

console.log(pkg.getTitle()); // My first agent
console.table(pkg.list());
~~~

Each source file has a `path` relative to the project root and `data` as
`Uint8Array` bytes. `TextEncoder` turns text into bytes. The package bytes are
in `result.data`. Once you open them with `AIX.From()`, methods such as
`pkg.getTitle()` and `pkg.list()` return immediately.

## Check and build a project {#check-and-build-a-project}

### Check source files {#aix-checkfromsource-files-options}

Call `AIX.checkFromSource(files)` when your code already has the project's
paths and bytes. Put `app.json` at the root of the file list.

The returned `diagnostics` array tells you what to fix. Each item includes a
`severity` of `error` or `warning`, a message, and the file path, line, and
column. An empty array means the check found no issues. Source problems appear
in the report. A failure to read or collect the files, load the checker, or
run a progress callback rejects the promise.

::: details What does the checker cover?
It checks JSON and text encoding, app, page, and widget structure, component
references, JavaScript and TypeScript syntax, recognizable permission use, and
page data schema declarations. It supports TypeScript in `.mts` and `.cts`
files and in `.ink` script blocks marked `lang="ts"`.

Schema checks default to JSON Schema 2020-12 and resolve local references.
Run a separate TypeScript type check and test the agent in AIUI to verify types
and runtime behavior. See [check progress](#check-progress-and-browser-responsiveness)
for stage updates.
:::

### Check a folder selected in the browser {#aix-checkfromfiles-files-options}

Use a directory picker when the user supplies the project.

~~~html
<input id="project-folder" type="file" webkitdirectory multiple>
~~~

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector('#project-folder');
input.addEventListener('change', async () => {
  if (!input.files.length) return;
  const report = await AIX.checkFromFiles(Array.from(input.files));
  console.table(report.diagnostics);
});
~~~

Choose the folder that contains `app.json`. `checkFromFiles()` reads the
browser's relative paths and removes the shared outer folder. It uses each
file's name when the browser provides no relative path.

### Build from source {#aix-packfromsource-files-options}

Use `AIX.packFromSource(files)` for a project. It cleans up paths, applies
root and nested `.aixignore` rules, and leaves ignore files and `.aix/` cache
folders out of the package. Invalid paths, duplicate normalized paths, or
invalid ignore rules stop the build.

Packing checks the files it needs to build the archive. Call
`checkFromSource()` first for a full source report, as in the quick start.

| Result field | What you get |
| --- | --- |
| `data` | The bytes of the new `.aix` file. |
| `warnings` | Issues that did not stop the build. |
| `report` | File sizes before and after processing, measured before ZIP compression. |
| `progressiveAssets` | How each processed media asset was delivered and how many source bytes stayed in the package. |

To offer the result as a browser download, call this function with
`downloadPackage(result.data, 'my-agent.aix')`.

~~~javascript
function downloadPackage(data, filename) {
  const url = URL.createObjectURL(new Blob([data], { type: 'application/zip' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
~~~

### Build from browser files {#aix-packfromfiles-files-options}

Use `AIX.packFromFiles(files)` when the browser file paths already start at
the project root. It uses `webkitRelativePath` or falls back to `name`.

A directory picker usually gives paths such as `my-agent/app.json`.
`packFromFiles()` keeps that folder prefix. For this input, convert the
selection to project paths and pass it to `packFromSource()`.

~~~javascript
// Call after the user selects the project folder.
const files = await Promise.all(Array.from(input.files, async file => ({
  path: file.webkitRelativePath.split('/').slice(1).join('/'),
  data: new Uint8Array(await file.arrayBuffer()),
})));
const result = await AIX.packFromSource(files);
downloadPackage(result.data, 'my-agent.aix');
~~~

### Pack a file list you prepared {#aix-pack-files-options}

Use `AIX.pack(files)` when your code already selected the files and normalized
their paths. It packs that list directly. Use `packFromSource()` if AIX
should apply the project's ignore rules.

All pack methods require a root `app.json`. AIX generates `VERSION` and
the metadata under `META-INF/aix/`. Leave those entries out of your inputs.

### Set build options {#set-build-options}

Pass options as the second argument to a pack method. For example, use
`AIX.packFromSource(files, { agentId: 'my-agent', optimize: {} })` to set a
project ID and turn on optimization.

| Option | What it does |
| --- | --- |
| `agentId` | Sets a stable project ID in the manifest. Reuse it across builds of the same project. |
| `buildId` | Sets the build's `VERSION`. The default is a new UUID. |
| `engine` | Sets the compatible AIUI version range. The SDK falls back to `app.json.engine`, then to `*` for any version. |
| `optimize` | Pass `{}` to enable JSON, PNG, and JPEG optimization at level 2. Set `level` to 1, 2, or 3, or turn off individual formats. |
| `onProgress` | Reports build stages in `packFromSource()` and `packFromFiles()`. |
| `onProgressiveAsset` | Chooses how to deliver each [Progressive Asset](#progressive-assets). |
| `signal` | Cancels a build through an `AbortSignal`. |

An `agentId` must contain lowercase ASCII letters, digits, `_`, or `-`.
It must be nonempty and cannot start or end with `-`. Supply it yourself
when you want a persistent identity.

Optional JSON and image optimization is off when `optimize` is omitted or
`false`. Packing still validates JSON, converts supported text to UTF-8, and
minifies standalone JavaScript and TypeScript files. TypeScript stays
TypeScript.

## Open an existing package {#aix-from-data-limits}

`AIX.From()` accepts a browser `File` or all the package bytes as a
`Uint8Array`. The method name starts with a capital **F**.

### Open a local file {#browser-example}

~~~html
<input id="aix-file" type="file" accept=".aix">
~~~

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector('#aix-file');
input.addEventListener('change', async () => {
  const file = input.files[0];
  if (!file) return;

  const pkg = await AIX.From(file);
  console.log(pkg.getTitle(), pkg.getVersion());
  console.table(pkg.list());
});
~~~

### Open a package from a URL {#open-a-package-from-a-url}

~~~javascript
const response = await fetch('/agents/example.aix');
if (!response.ok) throw new Error(`Download failed (${response.status})`);

const pkg = await AIX.From(new Uint8Array(await response.arrayBuffer()));
const text = new TextDecoder().decode(pkg.readFile('app.json'));
console.log(JSON.parse(text));
~~~

`pkg.readFile()` returns the bytes of one entry. Decode them as text for
JSON or scripts. For an image or another binary file, use the bytes directly.

### Check while opening {#aix-check-error}

Pass `true` as the third argument to run source checks before opening the
package. Warnings allow the package to open. Source errors produce an
`AixCheckError` with the full `report` and its `diagnostics` array.

~~~javascript
import { AIX, AixCheckError } from '@yodaos-pkg/aix';

async function openCheckedPackage(file) {
  try {
    return await AIX.From(file, undefined, true);
  } catch (error) {
    if (error instanceof AixCheckError) console.table(error.diagnostics);
    throw error;
  }
}
~~~

The default `AIX.From(file)` checks archive structure and reads individual
files when requested. Invalid archives and extraction failures raise their
own errors. The Rust reader offers publisher signature verification.

### Get page and tool details {#metadata-resolution-and-diagnostics}

Call `pkg.getPages()` for page metadata or `pkg.getTools()` for
OpenAI-compatible tool definitions derived from those pages. These methods
can return default metadata for incomplete pages.

Use `pkg.resolveMetadata()` when you want to see exactly which pages could
be read. It returns `pages`, `tools`, and `diagnostics`. A failed page
adds a diagnostic and is left out of both lists.

::: details How page resolution works
A page needs a readable `.ink` file or both `.json` and `.wxml` files.
A broken `.ink` file produces an error for that page. Optional definitions,
schemas, and styles may be absent. If both stylesheets exist, `.wcss` takes
precedence over `.wxss`.

A missing or invalid `app.json.pages` field produces a diagnostic for
`app.json`. An empty page list is valid. The default page comes first when
it resolves, and the other pages keep their declaration order. Widget and title
metadata are read separately.
:::

## Progressive Assets {#progressive-assets}

A Progressive Asset lets AIUI play media in smaller pieces. For remote
delivery, the package keeps the information needed to start playback and
AIUI downloads the pieces it needs.

Declare the media in a JavaScript module or a script in an `.ink` file.
The import returns an `AssetRef` such as `aix-asset:<id>`. Pass it to an
AIUI video or audio component.

~~~javascript
import clip from './assets/clip.mp4' with { type: 'video', progressive: 'prefer' };
~~~

Use `type: 'audio'` for audio. The current profile supports H.264 MP4 and
AAC-LC M4A. For compatible ordinary files, AIX can split the media into
fragmented MP4 without re-encoding it.

### Choose where the pieces are stored {#choose-where-the-pieces-are-stored}

The default is to embed the source media. For remote delivery, give
`packFromSource()` an `onProgressiveAsset` callback.

1. Return `{ delivery: 'embedded' }` to keep the media in the package.
2. To deliver it remotely, call `prepare()` and upload `plan.remoteObjects`
   using your own storage service.
3. Return the plan ID and upload receipt. AIX checks the receipt before
   it writes the package.

~~~javascript
const result = await AIX.packFromSource(files, {
  async onProgressiveAsset(candidate, { prepare, signal }) {
    if (!candidate.progressiveSupport.supported) {
      return { delivery: 'embedded' };
    }

    const plan = await prepare();
    // Implement this function with your storage service.
    const receipt = await uploadToStorage(plan, signal);
    return { delivery: 'progressive', planId: plan.id, receipt };
  },
});
~~~

`uploadToStorage(plan, signal)` is a function you provide. After all uploads
succeed, it returns a `ProgressiveAssetUploadReceipt`. For example, implement
`uploadObject` for your storage service so it uploads the supplied bytes and
resolves to their accessible HTTPS URL.

~~~javascript
async function uploadToStorage(plan, signal) {
  const objects = await Promise.all(plan.remoteObjects.map(async object => {
    // Implement uploadObject to upload the bytes and return their HTTPS URL.
    const url = await uploadObject(object.bytes, { id: object.id, signal });
    return { id: object.id, size: object.size, sha256: object.sha256, url };
  }));

  return {
    uploadId: crypto.randomUUID(),
    assetVersion: plan.assetVersion,
    objects,
  };
}
~~~

| Receipt field | Value |
| --- | --- |
| `uploadId` | A nonempty identifier for this upload. The example generates one locally; your service can provide it instead. |
| `assetVersion` | Copy `plan.assetVersion` from `prepare()`. |
| `objects` | One entry for every object in `plan.remoteObjects`, with no duplicates. |
| `objects[].id`, `size`, `sha256` | Copy these values from the corresponding prepared object. `size` is in bytes. |
| `objects[].url` | The complete, stable HTTPS URL returned for that uploaded object. Credentials, query strings, and URL fragments are not accepted. |

Pass `signal` to the upload function so it can respond to cancellation.
AIX checks the receipt's identifiers, sizes, hashes, and URL format before
writing the package. An upload error, invalid receipt, or cancellation stops
the build.

### Find the URLs in the package {#find-the-urls-in-the-package}

For remote delivery, AIX stores the media index at
`META-INF/assets/progressive_assets/index.json`. The index maps the ID in
`aix-asset:<id>` to an `asset.json` description. That description stores
the complete HTTPS address of each piece in `locators`.

At playback, AIUI takes the object ID from `layout.fragments[n].object` and
reads `locators[objectId].url`. It fetches that address when the piece is
needed and verifies its size and SHA-256. The initialization file
`init.mp4` is read from the package. See the
[package format](/spec#progressive-media-metadata).

### Try the build in Play {#play}

Use `AIX.listProgressiveAssets(files)` to see declared media paths, types,
source sizes, and format support before building. Pass an already selected
file list, as you would to `AIX.pack()`.

In [Build in Play](/play?mode=build), choose a project folder to see its media
list. **Build and download** creates the media pieces for supported files and
simulates the upload. Open the downloaded package in **Inspect** to view the
stored placeholder URLs.

## Optimize an existing package {#aix-optimize-data-options}

Pass a `File` or package bytes to `AIX.optimize()` to create an optimized
copy. You can reuse the download function from the build example.

~~~javascript
const result = await AIX.optimize(file, { level: 2 });
downloadPackage(result.data, 'my-agent-optimized.aix');
~~~

Omitting options enables the default JSON, PNG, and JPEG optimization.
The copy keeps `VERSION` and `agent_id`. AIX removes the existing signature
because the archive contents can change.

## API Reference {#api-reference}

[Static methods](#static-methods) · [Instance methods](#instance-methods) ·
[Options and callbacks](#options-and-callbacks) · [Types](#type-reference)

A `?` marks an optional parameter. Empty parentheses mean the method takes no
arguments. Use `await` for methods that return a `Promise`.

### Static methods {#static-methods}

Call these methods on the imported `AIX` class.

| Method | Resolves to |
| --- | --- |
| [`AIX.From(data, limits?, check?)`](#aix-from) | An open `AIX` package |
| [`AIX.checkFromSource(files, options?)`](#aix-check-from-source) | `CheckReport` |
| [`AIX.checkFromFiles(files, options?)`](#aix-check-from-files) | `CheckReport` |
| [`AIX.packFromSource(files, options?)`](#aix-pack-from-source) | `PackResult` |
| [`AIX.packFromFiles(files, options?)`](#aix-pack-from-files) | `PackResult` |
| [`AIX.pack(files, options?)`](#aix-pack) | `PackResult` |
| [`AIX.listProgressiveAssets(files, options?)`](#aix-list-progressive-assets) | `ProgressiveAssetCandidate[]` |
| [`AIX.optimize(data, options?)`](#aix-optimize) | `PackResult` |

#### AIX.From(data, limits?, check?) {#aix-from}

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits, check?: boolean): Promise<AIX>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | Complete `.aix` bytes or a browser file. To open a URL, fetch its bytes first. |
| `limits` | `AixReadLimits` | Optional [archive read limits](#limit-archive-reads). |
| `check` | `boolean` | Defaults to `false`. Pass `true` to check the extracted source files before returning. |

**Returns** a promise that resolves to an open package with [instance methods](#instance-methods).

**Errors** include invalid archives, exceeded limits, and read failures.
When you pass `true` for `check`, error diagnostics reject it with
[`AixCheckError`](#reference-aix-check-error). Warnings allow the package to open.

#### AIX.checkFromSource(files, options?) {#aix-check-from-source}

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `files` | `AixInputFile[]` | Files with a project-relative `path` and `Uint8Array` `data`. Include `app.json` at the root. |
| `options` | `CheckOptions` | Optional settings for reporting progress. |
| `options.onProgress` | `(event: CheckProgressEvent) => void` | Receives each [check stage or result](#check-progress-and-browser-responsiveness). |

Applies the project's `.aixignore` rules and checks the remaining source files.

**Returns** a promise that resolves to a [`CheckReport`](#source-check-types)
with a `diagnostics` array. Both errors and warnings in the source appear
in the array. It is empty when the check finds no issues.

**Errors** in input collection, checker loading, or the callback reject the
promise.

#### AIX.checkFromFiles(files, options?) {#aix-check-from-files}

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `files` | `File[]` | Browser files. Uses `webkitRelativePath` when available and falls back to `name`. Removes a shared outer folder from a directory selection. |
| `options` | `CheckOptions` | Optional settings for reporting progress. |
| `options.onProgress` | `(event: CheckProgressEvent) => void` | Receives each [check stage or result](#check-progress-and-browser-responsiveness). |

**Returns** a promise that resolves to a [`CheckReport`](#source-check-types).
Its `diagnostics` array contains any source errors and warnings.

**Errors** such as unreadable files and a thrown callback reject the promise.

#### AIX.packFromSource(files, options?) {#aix-pack-from-source}

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `files` | `AixInputFile[]` | Project paths and bytes, with `app.json` at the root. The SDK normalizes paths, applies root and nested `.aixignore` rules, and excludes rule files and `.aix/` caches. |
| `options` | `PackFromSourceOptions` | Optional [build settings](#pack-options). |
| `options.onProgress` | `(event: PackProgressEvent) => void` | Receives [build stages and file reports](#build-progress-callback). |
| `options.onProgressiveAsset` | [Progressive Asset callback](#progressive-asset-callback) | Takes a `ProgressiveAssetCandidate` and `ProgressiveAssetPackContext`. Returns a `ProgressiveAssetDecision` or a promise of one. |

**Returns** a promise that resolves to a [`PackResult`](#pack-result).
Use `data` for the archive, `report` for file sizes, `warnings` for build
warnings, and `progressiveAssets` for media delivery results.

**Errors** such as invalid paths or ignore rules, duplicate normalized paths,
callback failures, and cancellation reject the promise.

#### AIX.packFromFiles(files, options?) {#aix-pack-from-files}

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `files` | `File[]` | Browser files. Uses `webkitRelativePath` when available and falls back to `name`. The paths must place `app.json` at the root. |
| `options` | `PackFromSourceOptions` | Optional [build settings](#pack-options). |
| `options.onProgress` | `(event: PackProgressEvent) => void` | Receives [build stages and file reports](#build-progress-callback). |
| `options.onProgressiveAsset` | [Progressive Asset callback](#progressive-asset-callback) | Takes a `ProgressiveAssetCandidate` and `ProgressiveAssetPackContext`. Returns a `ProgressiveAssetDecision` or a promise of one. |

This method keeps the outer folder in browser paths. See the
[directory picker example](#aix-packfromfiles-files-options) for how to remove
that prefix before calling `packFromSource()`.

**Returns** a promise that resolves to a [`PackResult`](#pack-result).
It contains the archive bytes, file size report, warnings, and media delivery
results.

**Errors** such as unreadable files and other build failures reject the promise.

#### AIX.pack(files, options?) {#aix-pack}

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `files` | `AixInputFile[]` | An already selected list with normalized package paths and `app.json` at the root. Apply any ignore rules before calling this method. |
| `options` | `PackOptions` | Optional [build settings](#pack-options). |
| `options.onProgressiveAsset` | [Progressive Asset callback](#progressive-asset-callback) | Takes a `ProgressiveAssetCandidate` and `ProgressiveAssetPackContext`. Returns a `ProgressiveAssetDecision` or a promise of one. |

AIX generates `VERSION` and the package manifest. Leave `VERSION` and
reserved metadata paths out of the input file list.

**Returns** a promise that resolves to a [`PackResult`](#pack-result).
It contains the archive bytes, file size report, warnings, and media delivery
results.

**Errors** such as invalid inputs, callback failures, and cancellation reject
the promise.

#### AIX.listProgressiveAssets(files, options?) {#aix-list-progressive-assets}

~~~typescript
static listProgressiveAssets(
  files: AixInputFile[],
  options?: { signal?: AbortSignal },
): Promise<ProgressiveAssetCandidate[]>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `files` | `AixInputFile[]` | An already selected and normalized file list, in the same form as for `pack()`. |
| `options` | `{ signal?: AbortSignal }` | Optional discovery settings. |
| `options.signal` | `AbortSignal` | Cancels discovery. |

**Returns** a promise that resolves to a list of [`ProgressiveAssetCandidate`](#progressive-asset-types)
objects. Each includes the source path, media type, size in bytes, references,
and whether the current profile supports remote delivery. A project with no
declarations returns an empty list.

**Errors** such as invalid declarations, missing source files, or
cancellation reject the promise.

#### AIX.optimize(data, options?) {#aix-optimize}

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | An existing `.aix` package. |
| `options` | `OptimizeOptions` | Optional [optimization settings](#optimize-options). Omitting them uses the defaults. |

**Returns** a promise that resolves to a [`PackResult`](#pack-result) for the
optimized copy. Use `data` for the new archive and `report` to compare sizes.
`progressiveAssets` is empty for this operation. The copy keeps `VERSION` and
`agent_id` and drops any existing package signature.

**Errors** such as an invalid archive, a failed read, or failed optimization
reject the promise.

### Instance methods {#instance-methods}

Call these methods on the package returned by `AIX.From()`. They return
synchronously.

| What you need | Methods |
| --- | --- |
| Archive files | [`list()`](#pkg-list), [`readFile()`](#pkg-read-file) |
| Identity and compatibility | [`getVersion()`](#pkg-get-version), [`getAgentId()`](#pkg-get-agent-id), [`supportsEngine()`](#pkg-supports-engine) |
| App settings | [`getTitle()`](#pkg-get-title), [`getPermissions()`](#pkg-get-permissions) |
| Pages, widgets, and tools | [`getPages()`](#pkg-get-pages), [`resolveMetadata()`](#pkg-resolve-metadata), [`getWidgets()`](#pkg-get-widgets), [`getTools()`](#pkg-get-tools) |

#### list() {#pkg-list}

~~~typescript
list(): AixEntry[]
~~~

**Parameters** None.

**Returns** one `AixEntry` per archive entry. Each entry has a normalized
`name`, an uncompressed `size`, and a `compressed_size`, both in bytes.

#### readFile(name) {#pkg-read-file}

~~~typescript
readFile(name: string): Uint8Array
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `name` | `string` | Exact normalized entry name, such as `app.json`. Use [`list()`](#pkg-list) to find available names. |

**Returns** the uncompressed bytes of the entry as a `Uint8Array`.

**Errors** include a missing or corrupt entry, unsupported compression, and
an exceeded read limit.

#### getVersion() {#pkg-get-version}

~~~typescript
getVersion(): string | undefined
~~~

**Parameters** None.

**Returns** the build ID stored in `VERSION`. A missing, unreadable, or
invalid UTF-8 entry yields `undefined`.

#### getAgentId() {#pkg-get-agent-id}

~~~typescript
getAgentId(): string | undefined
~~~

**Parameters** None.

**Returns** the stable `agent_id` from the package manifest, or `undefined`
if there is no ID.

**Errors** include an unreadable or malformed manifest and an invalid ID.

#### supportsEngine(currentVersion) {#pkg-supports-engine}

~~~typescript
supportsEngine(currentVersion: string): boolean
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `currentVersion` | `string` | A concrete AIUI version such as `0.18.0`. |

**Returns** `true` if the version satisfies the manifest's `engine` range
and `false` if it falls outside that range.

**Errors** include a missing or invalid manifest, an invalid range, and an
invalid version.

#### getTitle() {#pkg-get-title}

~~~typescript
getTitle(): string | undefined
~~~

**Parameters** None.

**Returns** `app.json.window.navigationBarTitleText` as a string, or
`undefined` if the title is absent or the configuration cannot be read.

#### getPermissions() {#pkg-get-permissions}

~~~typescript
getPermissions(): string[]
~~~

**Parameters** None.

**Returns** the project's permission declarations in their original order,
including duplicates and custom names. An absent or empty permission list
yields `[]`.

**Errors** include unreadable or invalid `app.json` and a `permissions`
value other than an array of strings.

#### getPages() {#pkg-get-pages}

~~~typescript
getPages(): PageInfo[]
~~~

**Parameters** None.

**Returns** the default page first and the remaining pages in declaration order.
Missing or malformed page resources can produce defaults. An unreadable or invalid page list yields
`[]`. Use [`resolveMetadata()`](#pkg-resolve-metadata) to get page diagnostics.

#### resolveMetadata() {#pkg-resolve-metadata}

~~~typescript
resolveMetadata(): MetadataResolution
~~~

**Parameters** None.

**Returns** `{ pages, tools, diagnostics }`. The page and tool lists include
only pages that resolved successfully. A failed page contributes a diagnostic
with its resource path and message. An unreadable or invalid page list produces
an `app.json` diagnostic.

#### getWidgets(locale?) {#pkg-get-widgets}

~~~typescript
getWidgets(locale?: string): WidgetInfo[]
~~~

| Parameter | Type | Description |
| --- | --- | --- |
| `locale` | `string` | Optional locale such as `en-US`. A matching `app.<locale>.json` can override widget names and descriptions. Missing overrides use the values in `app.json`. |

**Returns** the declared widgets, or `[]` if the project declares none.

**Errors** include invalid configuration, an invalid matching locale file,
and a missing widget entry.

#### getTools() {#pkg-get-tools}

~~~typescript
getTools(): Tool[]
~~~

**Parameters** None.

**Returns** OpenAI-compatible tool definitions derived from the page list.
It uses the same page defaults as `getPages()`. For tools from successfully
resolved pages only, read `resolveMetadata().tools`.

### Options and callbacks {#options-and-callbacks}

#### Archive read limits {#limit-archive-reads}

All fields on `AixReadLimits` are optional. Each value must be a non-negative
safe integer. The limit is inclusive, zero allows no entries or bytes for
that field, and an omitted field has no limit. Unknown fields reject
`AIX.From()`.

| Field | Type | Meaning |
| --- | --- | --- |
| `maxEntries` | `number` | Maximum number of archive entries, including directories and signature entries. |
| `maxEntryBytes` | `number` | Maximum uncompressed size of one entry. |
| `maxTotalBytes` | `number` | Maximum sum of declared uncompressed entry sizes. Repeated reads do not add to the total. |

~~~javascript
const pkg = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
~~~

::: details Reader limits and memory
These limits apply to archive entries. The input buffer, parsed objects,
caller-retained copies, and memory used by source checks or optimization
are outside these limits.

Use `AixReader::new_with_limits()` in Rust or
`AixReaderWasm.with_limits(data, limits)` with the direct WASM reader.
:::

#### Check progress callback {#check-progress-and-browser-responsiveness}

~~~typescript
onProgress?: (event: CheckProgressEvent) => void
~~~

Set this callback on `CheckOptions` for `checkFromSource()` or
`checkFromFiles()`. The SDK calls it synchronously with one event. Its return
value is ignored, and a thrown exception rejects the check.

| `event.type` | Extra fields | When it occurs |
| --- | --- | --- |
| `loading_checker` | None | The SDK starts loading the checker. |
| `checking_sources` | `fileCount: number` | The check starts. The count is the input size before ignore rules. |
| `check_finished` | `check: CheckResult` | One check reports its ID, label, status, error and warning counts, and optional detail. |
| `collecting_diagnostics` | None | The SDK is about to return the diagnostic report. |

The check IDs are `json`, `structure`, `syntax`, `permissions`, and
`schemas`. A check can report `passed`, `warning`, `failed`, or `skipped`.
After loading, the source checker runs synchronously. Run it in a Web Worker
when the page needs to remain responsive during the check.

#### Build options {#pack-options}

All fields on `PackOptions` are optional.

| Field | Type | Meaning |
| --- | --- | --- |
| `buildId` | `string` | Build ID stored as `VERSION`. Defaults to a new UUID v4. |
| `agentId` | `string` | Stable project ID stored as `agent_id`. Use a nonempty string of lowercase ASCII letters, digits, `_`, or `-`, with no leading or trailing `-`. Omit it to leave the ID unset. |
| `engine` | `string` | AIUI version range. Defaults to `app.json.engine`, then to `*`. |
| `optimize` | `false \| OptimizeOptions` | An omitted value or `false` leaves optional JSON and image optimization off. An object enables the [settings below](#optimize-options). |
| `signal` | `AbortSignal` | Signal for cancelling the build and any upload that uses the same signal. |
| `onProgressiveAsset` | `(candidate, context) => ProgressiveAssetDecision \| Promise<ProgressiveAssetDecision>` | [Chooses delivery](#progressive-asset-callback) for each declared asset. |

`PackFromSourceOptions` adds `onProgress?: (event: PackProgressEvent) => void`
for `packFromSource()` and `packFromFiles()`.

#### Build progress callback {#build-progress-callback}

~~~typescript
onProgress?: (event: PackProgressEvent) => void
~~~

The callback receives one event for each build step the SDK reports. Its
return value is ignored, and a thrown exception rejects the build. Builds
that end early can stop before the final event.

| `event.type` | Extra fields | When it occurs |
| --- | --- | --- |
| `transferring_files_to_wasm` | None | The SDK starts passing inputs to the packer. |
| `collecting_source_inputs` | None | The build starts collecting source files. |
| `resolving_engine` | None | The packer determines the engine range. |
| `preparing_files` | None | The packer starts processing the selected files. |
| `file_finished` | `report: FileOptimizeReport` | One entry has been processed, including the generated `VERSION` entry. |
| `finalizing_archive` | None | The packer starts writing the finished archive and its manifest. |

#### Progressive Asset callback {#progressive-asset-callback}

~~~typescript
onProgressiveAsset?: (
  candidate: ProgressiveAssetCandidate,
  context: ProgressiveAssetPackContext,
) => ProgressiveAssetDecision | Promise<ProgressiveAssetDecision>
~~~

The SDK calls this callback once per declared asset and waits for its decision
before moving to the next asset.

| Parameter | Type | What the callback receives |
| --- | --- | --- |
| `candidate` | `ProgressiveAssetCandidate` | Source path, size, media type, references, and profile support for this asset. |
| `context.signal` | `AbortSignal` | Build cancellation signal to pass to your uploader. |
| `context.prepare` | `() => Promise<ProgressiveAssetPlan>` | Prepares media for remote delivery. Call it inside the callback. The plan includes `id`, `assetVersion`, timeline metadata, and the bytes in `remoteObjects`. Unsupported media rejects preparation. |

Return one of these decisions directly or through a promise.

| Decision | Effect |
| --- | --- |
| `{ delivery: 'embedded' }` | Keep the source media in the package. This is the default when the callback is omitted. |
| `{ delivery: 'progressive', planId, receipt }` | Deliver the prepared fragments remotely. Use the `id` from `prepare()` as `planId` and supply the upload receipt. |

The receipt contains a nonempty `uploadId`, the plan's `assetVersion`, and
one entry per object in `plan.remoteObjects`. Each entry includes the matching `id`,
`size`, and `sha256` plus its stable HTTPS `url`. The URL must be free of
credentials, query strings, and URL fragments. A callback error, invalid
receipt, or cancellation rejects the build. See the
[upload example](#progressive-assets) and [type definitions](#progressive-asset-types).

#### Optimization options {#optimize-options}

| Field | Type | Default |
| --- | --- | --- |
| `level` | `1 \| 2 \| 3` | `2` |
| `json` | `boolean` | `true`, optimize JSON. |
| `png` | `boolean` | `true`, optimize PNG images. |
| `jpeg` | `boolean` | `true`, optimize JPEG images. |

Pass an options object to enable optimization during packing. `AIX.optimize()`
uses these defaults even when its options argument is omitted.

#### Pack and optimize result {#pack-result}

| Field | Type | Meaning |
| --- | --- | --- |
| `data` | `Uint8Array` | The new archive bytes. |
| `report` | `OptimizeReport` | Per-entry and total byte counts before ZIP compression. |
| `warnings` | `string[]` | Warnings from the operation. |
| `progressiveAssets` | `ProgressiveAssetReport[]` | Build results for declared assets, including delivery mode and retained source bytes. Optimizing an existing package returns an empty list here. |

#### AixCheckError {#reference-aix-check-error}

~~~typescript
constructor(report: CheckReport)
~~~

The optional source check in `AIX.From()` rejects with this `Error` subclass
when it finds error diagnostics. The constructor takes the full check report.
The instance exposes `name` as `'AixCheckError'`, a summary in `message`,
the supplied `report`, and a `diagnostics` getter that returns
`report.diagnostics`, including warnings.

## Type reference {#type-reference}

Expand a group to see the TypeScript definitions.

### File inputs and build results {#file-and-build-types}

::: details Files, read limits, and build options
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
:::

### Progressive Asset types {#progressive-asset-types}

::: details Progressive Asset types
~~~typescript
type AssetRef = `aix-asset:${string}`;
type ProgressiveAssetKind = 'audio' | 'video';

interface ProgressiveAssetCandidate {
  id: string;
  kind: ProgressiveAssetKind;
  sourcePath: string;
  sourceSha256: string;
  sourceSize: number;
  references: { path: string; line: number; column: number }[];
  format: string;
  dependencies: string[];
  progressiveSupport: {
    supported: boolean;
    profile?: string;
    requiredCapabilities: string[];
    features: string[];
    reason?: string;
  };
}

interface ProgressiveAssetObject {
  id: string;
  size: number;
  sha256: string;
}

interface ProgressiveAssetPlan {
  id: string;
  assetVersion: string;
  profile: string;
  localMetadata: Record<string, Omit<ProgressiveAssetObject, 'id'>>;
  layout: {
    init: string;
    tracks: Record<string, { kind: ProgressiveAssetKind; timescale: number }>;
    fragments: { object: string; ranges: Record<string, { start: number; end: number; presentation_start: number; presentation_end: number }> }[];
  };
  remoteObjects: (ProgressiveAssetObject & { bytes: Uint8Array })[];
}

interface ProgressiveAssetUploadReceipt {
  uploadId: string;
  assetVersion: string;
  objects: (ProgressiveAssetObject & { url: string })[];
}

type ProgressiveAssetDecision =
  | { delivery: 'embedded' }
  | { delivery: 'progressive'; planId: string; receipt: ProgressiveAssetUploadReceipt };

interface ProgressiveAssetPackContext {
  signal: AbortSignal;
  prepare(): Promise<ProgressiveAssetPlan>;
}

interface ProgressiveAssetReport {
  id: string;
  kind: ProgressiveAssetKind;
  sourcePath: string;
  delivery: 'embedded' | 'progressive';
  assetVersion?: string;
  retainedSourceBytes: number;
}
~~~
:::

### Build progress events {#packprogressevent}

::: details PackProgressEvent
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

The `file_finished` event includes the report for the entry just processed,
including the generated `VERSION` entry.
:::

### Source check types {#source-check-types}

::: details Check reports and progress events
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
:::

### Package and derived models {#package-and-derived-models}

::: details Archive entries, pages, widgets, and tools
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

Page `size` and tool `layout` contain width and height constraints. Tools
emit `type: 'function'` with a target of `'_current'` or `'_blank'`. A
`JsonSchema` can contain any JSON value. Use the source checker to validate
schema declarations before relying on them.

Missing strings in these metadata objects are `null`, including in the
raw WASM output. Standalone methods such as `getTitle()` and
`getVersion()` return `undefined` when the value is absent.
:::

## Build the SDK locally {#build-from-source}

~~~bash
cd crates/aix-web
npm install
npm run build
~~~

The build writes the distribution to `crates/aix-web/dist`. When self-hosting,
serve the complete distribution. The SDK loads `pkg-reader/` to open packages
and loads `pkg/` on demand for checks, builds, and optimization. See
[WASM loading](/wasm) for details.
