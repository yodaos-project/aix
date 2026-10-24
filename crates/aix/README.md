# aix

`aix` is a Rust library for reading and managing **.aix** (AI eXecutable) packages. The `.aix` format is an extension of the [Open Agent Format (OAF)](https://openagentformat.com/spec.html), designed to package AI agents with rich UI/UX capabilities powered by Ink Mini Programs.

## AIX Format

An AIX package is essentially a zip archive containing the agent's definition, logic, and UI resources.

### File Structure

```text
.
├── AGENTS.md     # Agent identity & capabilities (OAF)
├── app.json      # UI configuration & routing
├── app.js        # Agent logic entry
└── pages/        # UI Page definitions
```

### Supported Page Formats

When defining pages or components, the AIX runtime supports two formats:

1. **Multi-file Format**: The traditional way with separated files.
   - `page.json`
   - `page.js`
   - `page.wxml`
   - `page.wxss`

2. **Single File Component (.ink)**: A Vue-like SFC format that bundles everything in one file.
   - `page.ink`

Example of an `.ink` file:
```xml
<script def>
{
  "navigationBarTitleText": "Example",
  "schema": {
    "data": {}
  }
}
</script>

<script setup>
export default {
  onLoad() { }
}
</script>

<page>
  <view class="container">Hello</view>
</page>

<style>
.container { color: red; }
</style>
```

## Usage (Rust)

```rust
use aix::AixReader;

let data = std::fs::read("bundle.aix")?;
let reader = AixReader::new(data)?;

for entry in reader.list() {
    println!("File: {}, Size: {}", entry.name, entry.size);
}

if let Some(version) = reader.get_version() {
    println!("Version: {}", version);
}

let content = reader.read_file("app.json")?;
```

### Archive resource limits

`AixReader::new` and `AixReadLimits::default()` preserve the unlimited policy for
compatibility. Hosts accepting untrusted archives should set all three budgets:

```rust
use aiui_aix::{AixReadLimits, AixReader};

let data = std::fs::read("bundle.aix")?;
let reader = AixReader::new_with_limits(data, AixReadLimits {
    max_entries: Some(1024),
    max_entry_bytes: Some(8 * 1024 * 1024),
    max_total_bytes: Some(32 * 1024 * 1024),
})?;
```

Budgets are inclusive, include directories and signature metadata, and are
checked against the central directory before extraction. Output is bounded for
stored and deflated entries, with CRC and size verification retained. `None`
means unlimited; zero permits no entries or bytes. The total budget sums declared
uncompressed bytes across the archive; repeated reads do not consume it. The
example values are host policy, not format requirements. Input archive bytes,
parsed objects, and copies retained by the caller need separate memory budgets.

### Metadata resolution diagnostics

`get_pages()` and `get_tools()` keep their tolerant defaults, including metadata
for missing page resources. Use the additive diagnostic API when exposing tools
or distinguishing a valid empty page list from failed resolution:

```rust
let resolution = reader.resolve_metadata();
for diagnostic in &resolution.diagnostics {
    eprintln!("{}: {}", diagnostic.path, diagnostic.message);
}
let tools = resolution.tools;
```

`MetadataResolution` contains `pages`, `tools`, and `diagnostics`. Failed pages are
omitted from both lists; other pages retain declaration order. To require every
page to resolve, reject reports with nonempty diagnostics. A valid empty page
list has no diagnostics; invalid/unreadable `app.json.pages` reports `app.json`.

Resolution requires readable `.ink` or both `.json` and `.wxml` resources. CRC,
size, UTF-8, and page metadata deserialization failures are reported, one per
failed page. Present styles are checked too, with `.wcss` preferred to `.wxss`.
Missing optional definitions, schemas, and styles retain defaults. A broken Ink
entry does not fall back to traditional files. This is resource/metadata
resolution, not full syntax, JSON Schema, component, or runtime validation.

Page, Widget, and title configuration are parsed independently. Invalid Widget
fields cannot hide valid pages; `get_widgets()` still reports Widget errors.
Packing validation and the archive/signature format are unchanged.

## CLI Tool

Install the `aix` command from npm:

- npm: `npm install -g @yodaos-pkg/aix-cli`

## Web Support

For JavaScript/TypeScript support and WASM, please use [aix-web](../aix-web).
