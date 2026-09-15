# @yodaos-pkg/aix-cli

Command-line tool for packing, validating, and inspecting **AIX** (AI eXecutable)
packages — the executable package format for AI agents.

Install with npm:

```bash
npm install -g @yodaos-pkg/aix-cli
```

Once installed, the `aix` command is available in your shell.

## Commands

### `aix pack <INPUT_DIR>`

Packs a directory into an `.aix` file. A unique `VERSION` entry (UUID v4) is
generated automatically. JSON files are validated before packing; non-UTF-8
`.json`/`.js`/`.ink` files are converted to UTF-8 inside the artifact.

`.js` and `.ts` files are minified by default during `aix pack`. This default
script processing is separate from `--optimize`, so it still runs when
`--optimize` or `-O` is not passed.

```bash
aix pack ./my-agent                  # defaults to bundle.aix
aix pack ./my-agent -o my-app.aix
aix pack ./my-agent --optimize       # adds PNG/JPEG + JSON optimization
aix pack ./my-agent -O --opt-level 3 # optimization level 1-3
aix pack ./my-agent --engine '^0.14.0'
aix pack ./my-agent --log-time       # prefix each pack log line with a local timestamp
```

If `--engine` is omitted, the packer falls back to `app.json.engine`, then to
`*`, and writes the resolved range to `META-INF/aix/manifest.json`. Packaged
readers use the manifest as the only source for engine compatibility checks.

`.aixignore` files inside the input directory are honored (`.gitignore` syntax).

When `.js` or `.ts` files are packed:

- only the packaged file contents change
- source files on disk are not modified
- paths and file extensions inside the artifact stay the same
- a minification failure stops packing and reports the file path

`--opt-level` only affects the JSON/PNG/JPEG optimization pipeline enabled by
`--optimize`. It does not change the default JS/TS minification behavior.

Use `--log-time` when you want to diagnose where `aix pack` is spending time.
This prefixes each pack log line with a local timestamp so gaps are easy to
spot during scanning, packing, and archive finalization.

### `aix show <INPUT>`

Prints the effective Agent Definition JSON without requiring ADB. This uses the
same resolver as `install`.

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### `aix install <INPUT>`

Packs a project, or accepts an existing `.aix`, and submits it to the public
AIUI DEVELOP endpoint on an authorized Rokid Glasses device over ADB. A project
reuses `.aix/agent-id`; an artifact uses its `VERSION`. An existing
`<PROJECT>/agent.json` overrides generated Definition metadata except for the
rule-derived `agentId`.

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

The sole online ADB device is selected automatically. If multiple devices are
online, the CLI prompts for one; use `--serial` in non-interactive shells.

The final JSON reports the local Apply outcome, operation ID, and the result of
one upload-status query. `cloudIndexed` remains `unverified` because device
acknowledgement does not prove cloud indexing.

### Device And Launch Commands

```bash
aix device
aix device set-dev
aix device unset-dev
aix launch-page ./my-agent pages/index/index --card
aix launch-widget ./my-agent widgets/order/index --position 2
aix widget-layout show
aix widget-layout --clear
```

`aix device` also lists the `.aix` Agents currently installed in the device
package directory, including manifest metadata when available.

### `aix list <AIX_FILE>`

Lists all files and sizes inside an `.aix` package. Alias: `aix ls`.

```bash
aix list bundle.aix
```

`aix list` only prints archive entries. Package metadata such as the engine
range comes from `META-INF/aix/manifest.json`.

### `aix optimize <AIX_FILE> -o <OUTPUT>`

Optimizes JSON, PNG, and JPEG entries in an existing package.

This is different from the default JS/TS processing in `aix pack`:

- `aix pack` minifies `.js` and `.ts` by default
- `aix pack --optimize` additionally optimizes JSON, PNG, and JPEG inputs
- `aix optimize` rewrites an existing `.aix` package for JSON/PNG/JPEG only

```bash
aix optimize input.aix -o output.aix --level 2
```

### `aix preview <INPUT>`

Generates a browser preview for an existing `.aix` package or an AIX source
directory using the `@yodaos-pkg/ink` browser SDK.

By default, the command starts a local HTTP server, serves the generated HTML
directly from memory, and prints the preview URL without opening the browser
automatically. In the default preview mode, the page embeds a snapshot of the
current bundle contents into a single HTML document, while still loading the
Ink SDK from the network at runtime. The current preview viewport is fixed at
`480x352`. Use `--launch --launch-target current` to open with a fixed
`448x150` current target. The preview page also provides Blank/Current controls
for switching targets.

When the input is a packaged `.aix` artifact, package metadata is read from
`META-INF/aix/manifest.json`. Directory preview reads the source tree directly.

```bash
aix preview bundle.aix
aix preview ./my-agent
```

If you want the CLI to open your default browser automatically, add `--launch`.

```bash
aix preview bundle.aix --launch
```

If you want to export the preview page instead of starting a local server, use
`--html-out`. Relative output paths are resolved from the current working
directory.

```bash
aix preview bundle.aix --html-out ./artifacts/preview.html
```

When `--html-out` is provided:

- the CLI writes the generated HTML to the requested file
- parent directories are created automatically when needed
- no local preview server is started
- the browser is not opened automatically
- `--launch` is not allowed

For active development, use `--dev`. In this mode, the preview page no longer
embeds the current bundle snapshot. Instead, it loads state from the local
preview server, connects to a WebSocket endpoint, and rebuilds the `InkView`
when the input `.aix` file or source directory changes.

Preview loads the Ink browser runtime from `jspm.io` using an import map and
the version selected by `aix runtime select`. If no local runtime has been
selected, it falls back to the `latest` version from the active
`AIX_NPM_REGISTRY`.

```bash
aix preview bundle.aix --dev
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

When `--dev` is provided:

- the local preview server always starts
- `--html-out` is not allowed
- the browser page fetches preview state from the server
- file changes trigger a WebSocket reload signal
- the page rebuilds the `InkView` without refreshing the full document
- add `--launch` if you want the browser to open automatically

### `aix runtime versions`

Lists the available stable preview runtime versions for `@yodaos-pkg/ink`.

```bash
aix runtime versions
```

The command prints:

- the runtime source summary
- the package name
- the effective current runtime version
- the locally selected version when present
- the published stable version list

Use `AIX_NPM_REGISTRY` to choose which registry provides Ink package metadata:

```bash
AIX_NPM_REGISTRY=npm aix runtime versions
AIX_NPM_REGISTRY=npmmirror aix runtime versions
```

Supported values:

- `npm` (default)
- `npmmirror`

When a local runtime has been selected, the CLI stores it in
`~/.aix/runtime.json`. `aix runtime current` returns that selected version
before falling back to the active registry `latest`.

### `aix runtime current`

Prints the current default preview runtime version as a single line.

```bash
aix runtime current
```

This output is designed to work well in scripts.

### `aix runtime select`

Starts an interactive terminal selector for choosing a stable preview runtime
version. After you confirm a choice, the command saves it to
`~/.aix/runtime.json`, prints the selected version, and exits.

```bash
aix runtime select
```

## Development

```bash
npm install
npm run build   # compiles the Rust engine to WASM (requires rustup + wasm32-unknown-unknown + wasm-pack), then bundles the CLI
```

## How it works

The CLI is a thin TypeScript shell over the Rust AIX engine compiled to a
Node.js WASM bundle (`wasm-pack --target nodejs`). Command registration uses
`commander`, and interactive runtime selection uses `@inquirer/prompts`.
Packing, optimization, and reading logic are shared byte-for-byte with the
Rust/Web surfaces — no behavior fork.

## License

MIT
