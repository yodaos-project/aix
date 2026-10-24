# @yodaos-pkg/aix-cli

Command-line tool for packing, inspecting, installing, and previewing **AIX**
(AI eXecutable) packages.

## Table of Contents

- [Installation](#installation)
- [Package Commands](#package-commands)
- [Device Commands](#device-commands)
- [Preview Commands](#preview-commands)
- [Development](#development)
- [License](#license)

## Installation

```bash
npm install -g @yodaos-pkg/aix-cli
```

## Package Commands

### `aix pack <INPUT_DIR>`

Packs a directory into `.aix`. Packing validates JSON, converts supported text
to UTF-8, generates a UUID v4 `VERSION`, and writes
`META-INF/aix/manifest.json`.

```bash
aix pack ./my-agent
aix pack ./my-agent -o my-app.aix
aix pack ./my-agent --engine '^0.14.0'
aix pack ./my-agent --log-time
```

JS/TS is minified by default without changing source files. `--optimize`
additionally optimizes JSON, PNG, and JPEG:

```bash
aix pack ./my-agent --optimize
aix pack ./my-agent -O --opt-level 3
```

Without `--engine`, the range comes from `app.json.engine`, then `*`.
`.aixignore` uses `.gitignore` syntax. `--log-time` timestamps pack logs.

### `aix check <INPUT_DIR>`

Checks source files without creating an artifact. It validates `app.json`, Page
and Widget entries, local component references, JS/TS syntax,
Page JSON Schemas, and statically recognizable permission use. It applies `.aixignore` like `pack`. Errors make
the command exit with status 1; warnings do not. `--format json` emits a
machine-readable diagnostic list with file, line, column, severity, and rule
code.

```bash
aix check ./my-agent
aix check ./my-agent --format json
```

Text mode reports scanning, checker loading, source/schema validation, and
diagnostic collection on stderr. A terminal displays an animated
spinner; checking runs in a Worker to keep it responsive. `--format json`
disables progress and preserves the structured report on stdout.
Startup status appears before loading the CLI command modules and is replaced
by later stages on the same progress line.
The final checklist preserves each check result: JSON/text validation,
App/Page/Widget/component structure, JS/TS syntax, permission declarations and
direct API use, and Page JSON Schemas. Items show passed, warning, failed, or skipped status, work counts,
and diagnostic counts. Checks with no applicable input or blocked by invalid
input show a skip reason instead of a success marker.

JS/TS syntax checks include `.ts`, `.mts`, `.cts`, and Ink `<script lang="ts">`
blocks, preserving original file locations. TypeScript semantic checks, import
resolution, and compiler-option validation are currently excluded. No type
environment is loaded. Checks never emit files or run project code.

The CLI and Web SDK use the same in-memory Rust/WASM check engine. The CLI reads
source files and formats diagnostics; the shared checker has no filesystem or
network access. Web callers use `AIX.checkFromSource(files)` or
`AIX.checkFromFiles(files)`. Both accept an optional `{ onProgress(event) {} }`
argument. `check_finished` events include a `CheckResult` for each of the five
items in the final checklist.

Page `schema.data` is validated as a JSON Schema, including nested keyword
types, regex patterns, and local `$ref` targets. Drafts 4, 6, 7, 2019-09, and
2020-12 are selected through `$schema`; without it, 2020-12 is used. Invalid
schemas produce `AIX047`. Missing or null `schema.data` remains accepted for
pages without inputs. External `$ref` resources are not fetched and unresolved
references are errors. This validates the schema declaration, not runtime page
data. `pack` and the AIX reader retain their existing schema pass-through behavior.

Permission analysis recognizes direct calls such as `navigator.geolocation`,
`navigator.mediaDevices.getUserMedia()`, and media-library reads. Dynamic
arguments and host-provided components produce warnings when they cannot be
verified locally. A clean result does not prove runtime or platform permission
availability.

| Recognized use | Required declaration |
| --- | --- |
| `navigator.geolocation.getCurrentPosition()` / `watchPosition()` | `GEOLOCATION` |
| `navigator.mediaDevices.getUserMedia({ audio: true })` | `RECORD_AUDIO` |
| `navigator.mediaDevices.getUserMedia({ video: true })` | `CAMERA` |
| `navigator.mediaLibrary.list()` / `get()` with a known media type | `READ_MEDIA_IMAGES` or `READ_MEDIA_AUDIO` |

Network calls without `INTERNET` and `mediaLibrary.save()` produce warnings.
This first version checks direct calls in included
JS/TS files and Ink `<script>` blocks; aliases and dynamically constructed
calls are outside its coverage.

### `aix show <INPUT>`

Prints the effective Agent Definition JSON without ADB. `show` and `install`
use the same resolver.

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### `aix list <AIX_FILE>`

Lists archive entries and sizes. `aix ls` is an alias.

```bash
aix list bundle.aix
aix ls bundle.aix
```

### `aix optimize <AIX_FILE>`

Optimizes JSON, PNG, and JPEG in an existing package:

```bash
aix optimize input.aix -o output.aix --level 2
```

## Device Commands

All device commands accept `-s, --serial <serial>`. One online authorized ADB
device is selected automatically; multiple devices open an interactive
selector. Non-interactive shells must pass `--serial`.

Active stages show a spinner. Completed intermediate stages collapse to one
check-mark line; only the latest result keeps detailed information.

### `aix device [ACTION]`

```bash
aix device
aix device set-dev
aix device unset-dev
```

The read-only form shows Developer Mode, Widget readiness and layout, and
installed `.aix` Agents. Changing Developer Mode reloads all Widgets; dynamic
Widgets must then be launched again.

### `aix install <INPUT>`

Packs a project or accepts `.aix`, then submits it to AIUI DEVELOP over ADB.

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

Directories reuse `.aix/agent-id`; artifacts use `VERSION`. `agent.json` may
override generated metadata except `agentId`. Apply and phone-upload results
are validated; phone confirmation does not prove cloud indexing.

### `aix uninstall <INPUT>`

Remove one DEVELOP Agent from the device by project directory, `.aix` file, or
explicit `develop.rokid.*` Agent ID:

```bash
aix uninstall ./my-agent
aix uninstall ./bundle.aix
aix uninstall develop.rokid.mobility.ride --serial <serial>
```

This calls the device's `remove` operation without Prepare or Push. Success
requires local removal and dispatch confirmation; cloud removal is not verified.
It does not guarantee that an already open Page closes, and it cannot remove
built-in Agents.

### `aix launch-page <INPUT> [PATH]`

```bash
aix launch-page ./my-agent
aix launch-page ./my-agent pages/index/index
aix launch-page ./my-agent pages/index/index --card
aix launch-page ./my-agent pages/index/index --params '{"id":"123"}'
```

Use `--params-file <FILE>` for file parameters. This opens an installed Agent;
it does not install one.

### `aix launch-widget <INPUT> <PATH>`

```bash
aix launch-widget ./my-agent widgets/order/index
aix launch-widget ./my-agent widgets/order/index --position 2
```

Size comes from `app.json.widgets[].family`. Compatible placements are kept.
Position conflicts replace overlapping overlay Widgets; when no space remains,
old overlay placements are cleared. Persistent Widgets are never removed.

When needed, the command runs
`prepare -> push widget-config.json -> widget-apply -> open`. A missing runtime
placement causes one layout reapply and Open retry.

### `aix widget-layout [show]`

```bash
aix widget-layout
aix widget-layout show
aix widget-layout --clear
aix widget-layout --clear --yes
```

Show is read-only. Clear preserves the grid, empties both module arrays, and
asks for confirmation unless `--yes` is supplied.

## Preview Commands

### `aix preview <INPUT>`

Previews an artifact or source directory with `@yodaos-pkg/ink`:

```bash
aix preview bundle.aix
aix preview ./my-agent
aix preview bundle.aix --launch
aix preview bundle.aix --launch --launch-target current
```

By default, a local server starts and prints its URL. The `blank` viewport is
`480x352`; `current` uses `448x150`.

Export static HTML without starting a server:

```bash
aix preview bundle.aix --html-out ./artifacts/preview.html
```

`--html-out` cannot be combined with `--launch`. For live reload through
WebSocket, use:

```bash
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

### `aix runtime`

```bash
aix runtime versions
aix runtime current
aix runtime select
```

- `versions` lists stable versions and the effective selection.
- `current` prints only the resolved version.
- `select` saves an interactive choice to `~/.aix/runtime.json`.

Choose the registry with `AIX_NPM_REGISTRY=npm` (default) or
`AIX_NPM_REGISTRY=npmmirror`.

## Development

```bash
npm install
npm run build
node dist/cli.js --help
```

The build compiles the Rust engine to Node.js WASM and bundles the TypeScript
CLI. Packing, optimization, and reading logic are shared with the Web surface.

## License

MIT
