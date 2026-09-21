# @yodaos-pkg/aix-cli

[简体中文](README.zh-CN.md)

Command-line tool for packing, inspecting, installing, and previewing **AIX**
(AI eXecutable) packages.

## Table of Contents

- [Installation](#installation)
- [Package Commands](#package-commands)
  - [`aix pack`](#aix-pack-input_dir)
  - [`aix show`](#aix-show-input)
  - [`aix list`](#aix-list-aix_file)
  - [`aix optimize`](#aix-optimize-aix_file)
- [Device Commands](#device-commands)
  - [`aix device`](#aix-device-action)
  - [`aix install`](#aix-install-input)
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
