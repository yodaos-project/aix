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

## Using the Package Lab

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
optimizer as the native CLI:

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

#### `static async From(data: Uint8Array | File): Promise<AIX>`
Initializes the WASM module and creates an AIX instance from the given `.aix` file content. It supports both `Uint8Array` and the standard Web `File` object.
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

#### `getPages(): PageInfo[]`
Returns information about all pages defined in the package, including their paths, titles, and data schemas.
```typescript
const pages = aix.getPages();
/*
[{
  name: "pages/index/index",
  title: "Home",
  data_schema: { ... }
}]
*/
```

#### `getWidgets(locale?): WidgetInfo[]`
Returns widget declarations after verifying that every path resolves to an `.ink` entry in the package. When `locale` is provided, merges `displayName` and `description` from the matching `app.<locale>.json` overlay. Throws when a declared entry is missing or the selected locale file is invalid.
```typescript
const widgets = aix.getWidgets('en-US');
// [{ path: "widgets/clock/index", family: "1x1", placement: "persistent", displayName: "Clock", description: "Shows the current time." }]
```

#### `getTools(): Tool[]`
Generates a list of OpenAI-compatible tool definitions based on the pages and their schemas.
```typescript
const tools = aix.getTools();
/*
[{
  type: "function",
  function: {
    name: "pages_index_index",
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

### `PageInfo`
```typescript
interface PageInfo {
  name: string;
  title?: string;
  data_schema: any;
}
```

### `WidgetInfo`
```typescript
interface WidgetInfo {
  path: string;
  family: string;
  placement: "persistent" | "overlay";
  displayName?: string | null;
  description?: string | null;
}
```

### `Tool`
OpenAI compatible tool format.
```typescript
interface Tool {
  type: string;
  function: {
    name: string;
    description?: string;
    parameters: any;
  };
}
```

## Build

To build the library for distribution:

```bash
npm run build
```

The output will be generated in the `dist` directory, including WASM binaries and TypeScript definitions.

## License

MIT
