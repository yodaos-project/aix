# @yodaos-pkg/aix-cli

[简体中文](README.zh-CN.md)

Command-line tool for packing, inspecting, installing, and previewing **AIX**
(AI eXecutable) packages.

## Table of Contents

- [Installation](#installation)
- [Package Commands](#package-commands)
  - [`aix pack`](#aix-pack-input_dir)
  - [`aix check`](#aix-check-input_dir)
  - [`aix show`](#aix-show-input)
  - [`aix list`](#aix-list-aix_file)
  - [`aix optimize`](#aix-optimize-aix_file)
- [Device Commands](#device-commands)
  - [`aix device`](#aix-device-action)
  - [`aix install`](#aix-install-input)
  - [`aix uninstall`](#aix-uninstall-input)
  - [`aix launch-page`](#aix-launch-page-input-path)
  - [`aix launch-widget`](#aix-launch-widget-input-path)
  - [`aix widget-layout`](#aix-widget-layout-show)
- [Preview Commands](#preview-commands)
  - [`aix preview`](#aix-preview-input)
  - [`aix runtime`](#aix-runtime)
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

Run static checks without writing an artifact. Checks include `app.json`,
Page and Widget entries, local components, JS/TS syntax, TypeScript types,
Page `schema.data` JSON Schemas, and direct API calls
with known permission requirements. Errors exit with status 1; warnings do not.
Use `--format json` for structured diagnostics. Dynamic permission use and
host components may need manual review.

```bash
aix check ./my-agent
aix check ./my-agent --format json
```

Text output shows check stages on stderr, with an animated spinner in a
terminal. Startup status appears before loading command modules. Checks run
in a Worker so progress remains responsive during TypeScript analysis. JSON
mode omits progress and keeps stdout machine-readable.

TypeScript checks include `.ts`, `.mts`, `.cts`, and Ink `<script lang="ts">` blocks,
with original file locations, imports, project declarations, and `tsconfig.json`
compiler options/`extends`. One project `Program` uses `files`/`include`/`exclude`
to select roots and follows their imports, preserving NodeNext ESM/CJS resolution
and conditional exports. `.aixignore` excludes source availability. Without a
config, strict ES2022 checks use Ink's `@yodaos-pkg/ink-env` types; the project's
copy is preferred, with embedded declarations as a fallback. No files are emitted.
JavaScript is syntax-checked and supplies inferred types when imported.

The complete check engine is shared with the Web SDK. The CLI only reads the
directory and formats diagnostics; type resolution uses that input snapshot,
including configs and dependency declarations. Files outside the supplied tree
are not read, and dependencies are not downloaded.

Page schemas are checked against their `$schema` draft (2020-12 by default),
including nested keywords, patterns, and local references. Missing or null
`schema.data` is accepted. External references are not fetched; unresolved refs
are errors. This checks declarations, not runtime data. Diagnostic codes are
`AIX047` (schema), `AIX048` (TypeScript), and `AIX049` (TypeScript config).

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

Remove one DEVELOP Agent by project directory, `.aix` file, or Agent ID:

```bash
aix uninstall ./my-agent
aix uninstall ./bundle.aix
aix uninstall develop.rokid.mobility.ride --serial <serial>
```

The device must confirm local removal and dispatch. Cloud removal is not
verified, and the command does not guarantee that an already open Page closes.

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
CLI. Packing, optimization, and reading logic are shared with the Rust and Web
surfaces.

## License

MIT
