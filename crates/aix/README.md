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

## CLI Tool

Install the `aix` command from npm:

- npm: `npm install -g @yodaos-pkg/aix-cli`

## Web Support

For JavaScript/TypeScript support and WASM, please use [aix-web](../aix-web).
