# CLI

[简体中文](./cli.zh-CN.md)

The AIX CLI packages application directories, inspects archive contents, and
optimizes existing `.aix` artifacts. Two surfaces expose the identical `aix`
command, sharing the same Rust packing engine:

- **npm** (`@yodaos-pkg/aix-cli`) — a TypeScript shell over the Rust engine
  compiled to a Node.js WASM bundle.
- **Native (Rust)** (`aiui-aix-cli`) — a compiled binary from the same engine.

## Install

Pick either install path:

```bash
# npm (no Rust toolchain needed)
npm install -g @yodaos-pkg/aix-cli

# or native Rust
cargo install aiui-aix-cli
```

After installation, confirm that the command is available:

```bash
aix --help
```

## Pack A Directory

Create an AIX package from a source directory:

```bash
aix pack ./my-agent
```

The default output is `bundle.aix`. Use `--output` or `-o` to choose another
path:

```bash
aix pack ./my-agent -o dist/my-agent.aix
```

Packing performs the following work before writing the archive:

- respects `.aixignore` and the standard ignore files
- validates JSON files
- validates the app contract, rejecting a missing `app.json` and warning when
  its `pages` field is not an array of non-empty page paths
- converts supported text files to UTF-8 when needed
- generates a unique `VERSION` build ID
- generates `META-INF/aix/manifest.json` with file digests and package metadata

### Engine Compatibility

Use `--engine` to declare which AIX engine versions may run the package:

```bash
aix pack ./my-agent --engine '^0.14.0'
```

The effective range defaults to `*`. Common forms include:

```text
*           any engine version
0.14.0      exactly 0.14.0
>=0.14.0    0.14.0 or newer
^0.14.0     versions compatible with 0.14.0
```

The range is validated during packing and saved to
`META-INF/aix/manifest.json`. If `--engine` is omitted, the packer falls back to
`app.json.engine`, then to `*`. After packing, runtime compatibility checks read
the engine range only from the manifest.

### Optimize While Packing

Enable JSON, PNG, and JPEG optimization with `--optimize` or `-O`:

```bash
aix pack ./my-agent -O
```

Optimization levels range from 1 to 3 and default to 2:

```bash
aix pack ./my-agent -O --opt-level 3
```

Optimization changes only the packaged output. Source files remain untouched.

### Pack Options

```text
Usage: aix pack [OPTIONS] <INPUT_DIR>

Arguments:
  <INPUT_DIR>  Input directory to pack

Options:
  -o, --output <OUTPUT_FILE>   Output file [default: bundle.aix]
  -O, --optimize               Enable optimization
      --opt-level <LEVEL>      Optimization level, 1-3 [default: 2]
    --engine <RANGE>         Supported engine range
  -h, --help                   Print help
```

## List Package Contents

Inspect entry names and their uncompressed and compressed sizes:

```bash
aix list ./bundle.aix
```

`ls` is available as a shorter alias:

```bash
aix ls ./bundle.aix
```

`aix list` only prints archive entries. For packaged metadata such as the engine
range, readers use `META-INF/aix/manifest.json` rather than `app.json`.

## Optimize An Existing Package

Write an optimized copy of an existing artifact:

```bash
aix optimize ./bundle.aix -o ./bundle.optimized.aix
```

Choose an optimization level when needed:

```bash
aix optimize ./bundle.aix -o ./bundle.optimized.aix --level 3
```

The optimizer preserves the existing engine range. Because optimization changes
package contents, it removes any previous signature and writes a new unsigned
manifest. Sign the optimized artifact again before distribution when signature
verification is required.

## Preview In The Browser

Preview either an existing `.aix` package or an AIX source directory in a local
browser host powered by `@yodaos-pkg/ink`:

```bash
aix preview ./bundle.aix
aix preview ./my-agent
```

By default, the command starts a local HTTP server, serves the preview page from
memory, and prints the preview URL without opening the browser automatically.
The default preview mode embeds a snapshot of the current bundle contents into a
single HTML document while loading the Ink SDK from the network at runtime. The
preview uses the `blank` target by default with a `480x352` viewport. Use
`--launch-target current` to open the launch page with the current target at a fixed `448x150` viewport:

```bash
aix preview ./bundle.aix --launch --launch-target current
```

The preview page also provides Blank/Current controls for switching targets.

When the input is a packaged `.aix` artifact, package metadata is read from
`META-INF/aix/manifest.json`. Directory preview uses the source tree directly
and does not treat `app.json.engine` as a runtime compatibility source.

If you want the CLI to launch your default browser automatically, add
`--launch`:

```bash
aix preview ./bundle.aix --launch
```

### Export A Static Preview

Use `--html-out` when you want to write the generated HTML to disk instead of
starting a local server:

```bash
aix preview ./bundle.aix --html-out ./artifacts/preview.html
```

When `--html-out` is provided:

- the CLI writes the generated HTML to the requested file
- relative output paths resolve from the current working directory
- parent directories are created automatically when needed
- no local preview server is started
- the browser is not opened automatically
- `--launch` is not allowed

Preview loads the Ink browser runtime from `jspm.io` using an import map and
the version selected by `aix runtime select`. If no local runtime has been
selected, it falls back to the `latest` version from the active
`AIX_NPM_REGISTRY`.

The preview sidebar status area also shows the active Ink runtime version, for
example `Ink runtime: 0.14.0`, so it is easy to confirm which runtime version
the current session is using.

### Development Mode

Use `--dev` when you want the preview page to load state from the preview server
instead of embedding a fixed snapshot:

```bash
aix preview ./bundle.aix --dev
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

In `--dev` mode:

- the local preview server always starts
- the page fetches preview state from the server
- the page connects to a WebSocket endpoint for change notifications
- `.aix` file changes and directory file changes both trigger live reload
- the page rebuilds the `InkView` without refreshing the full document
- `--html-out` is not allowed
- add `--launch` if you want the browser to open automatically

## Show Agent Metadata

Print the effective AIUI DEVELOP Agent Definition without connecting to ADB:

```bash
aix show ./my-agent
aix show ./bundle.aix
```

`show` and `install` use the same resolver, so the JSON shown here is the JSON
that `install` uploads. Standard output contains JSON only. Write it to a file or
use compact single-line output when needed:

```bash
aix show ./my-agent --output ./agent.json
aix show ./my-agent --compact
```

An explicit Definition can be inspected with `--definition <file>`. Its
`agentId` is still replaced by the stable directory/package-derived ID.

For directory inputs, `show` creates `.aix/agent-id` if this is the first time
the project identity has been resolved.

## Install On Rokid Glasses

Pack a source project and submit it to the public AIUI DEVELOP endpoint over
ADB:

```bash
aix install ./my-agent
aix install ./bundle.aix
```

For a source directory, the command creates `.aix/agent-id` on the first run
and reuses it on later runs. For an existing `.aix` file, it derives the ID
from the package `VERSION` entry. Both become
`develop.rokid.agent.<id>` in the generated Definition.
The local `.aix/` state directory is excluded from packaged artifacts.

If `<project>/agent.json` exists, its fields remain authoritative except for
`agentId`, which always follows the stable rule above. Otherwise `install`
generates the temporary Definition fields from `app.json` and the first usable
paragraph in `AGENTS.md`, with protocol defaults for optional fields. Choose a
different Definition file or ADB device explicitly when needed:

```bash
aix install ./my-agent --definition ./develop-agent.json --serial <serial>
```

The glasses must already have Developer Mode enabled and be authorized for ADB.
The command automatically uses the sole online device. When multiple devices
are available, it prompts for one in an interactive terminal; pass `--serial`
to select explicitly or when running non-interactively. It creates a temporary
AIX package, enforces the 128 MiB limit, clears only the
documented DEVELOP staging inbox, and executes `prepare -> push -> apply`. It
validates each structured `result_data`, including the Definition `agentId`,
Apply outcome, and `operationId`.

After Apply, the command makes one status query. The final JSON distinguishes
local Apply (`outcome`), phone confirmation (`publishState: "UPLOADED"`), and
cloud indexing (`cloudIndexed: "unverified"`). A `QUEUED` or `DISPATCHED`
state is reported as pending; `FAILED_RETRYABLE` and `FAILED_PERMANENT` fail the
command.

Packing options are also available during installation:

```bash
aix install ./my-agent --engine '>=0.17.0' --optimize --opt-level 2
```

## Device Developer Mode

Interactive device operations show a spinner for the active stage. Completed
intermediate stages collapse to a single check-mark line; only the latest result
keeps its detailed Agent, path, device, and runtime information.

Query the current environment and Widget readiness without changing device
state, or explicitly set/unset Developer Mode:

```bash
aix device
aix device set-dev
aix device unset-dev
```

Setting or unsetting Developer Mode reloads all AIUI Widgets. Permanent Widgets
are recreated automatically; dynamic Widgets must be launched again. All device
commands accept `--serial <serial>`. The read-only status also lists installed
`.aix` packages from the device package directory and enriches them with Agent
metadata from `developer_manifest.json`.

## Launch An Installed Page

Open the first declared Page full screen, a selected Page, or a card:

```bash
aix launch-page ./my-agent
aix launch-page ./my-agent pages/index/index
aix launch-page ./my-agent pages/index/index --card
```

Pass parameters with `--params <JSON>` or `--params-file <FILE>`. This command
only invokes DEVELOP `open`; it does not install or upload the Agent.

## Configure And Launch A Dynamic Widget

Configure a dynamic Widget from the device's current layout and then open it:

```bash
aix launch-widget ./my-agent widgets/order/index
aix launch-widget ./my-agent widgets/order/index --position 2
```

The command reads `widget-snapshot.configurationJson` on every run. If no
configuration exists, it starts with an empty 4-cell, 2-column layout. Widget
dimensions come exclusively from the declared `family`; no size option is
exposed. Existing placements are reused, otherwise the first free position is
selected. A requested-position conflict replaces the overlapping dynamic
Widget automatically. If no free position remains, existing dynamic placements
are cleared and the new Widget is placed without removing permanent Widgets.

When the layout changes, the command runs
`prepare -> push widget-config.json -> widget-apply -> open`. An unchanged
layout calls `open` directly; if the device reports a missing runtime placement,
the CLI reapplies the saved layout and retries once. Layout changes and recovery
reapplies reload all Widgets, so other dynamic Widgets must be launched again.

## Inspect Or Clear Widget Layout

```bash
aix widget-layout
aix widget-layout show
aix widget-layout --clear
aix widget-layout --clear --yes
```

Show is read-only. Clear preserves the grid configuration while emptying both
module arrays, requires confirmation, and skips `widget-apply` when the layout
is already empty.

## Inspect Preview Runtime Versions

Use the `runtime` command group to inspect the preview runtime used by the npm
CLI.

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

### List Available Runtime Versions

```bash
aix runtime versions
```

This prints the effective current runtime version, the selected local version
when present, and the published stable version list for `@yodaos-pkg/ink`.
The output includes:

- `Runtime source`
- `Package`
- `Current`
- `Selected` when a local runtime has been saved
- `Versions`

### Print The Current Default Runtime

```bash
aix runtime current
```

The command prints only the resolved version string, so it also works well in
shell scripts. It prefers the version saved by `aix runtime select`.

### Select A Runtime Interactively

```bash
aix runtime select
```

This opens an interactive terminal selector, saves the chosen version to
`~/.aix/runtime.json`, prints it, and then exits. Only stable versions are
shown. The currently selected version is marked as `(selected)`, and the
registry default version is marked as `(current default)` when they differ.

## Run From The Workspace

During development, run either surface without installing it globally:

```bash
# npm surface
cd packages/cli
npm install
npm run build
node dist/cli.js pack ./my-agent -o bundle.aix

# native Rust surface
cargo run -p aiui-aix-cli -- pack ./my-agent -o bundle.aix
```

Pass `--help` after a subcommand to inspect its current options:

```bash
node dist/cli.js pack --help
node dist/cli.js runtime --help
```

## Related Documentation

- Read the [Specification](/spec) for the package model.
- Review [Packages](/packages) for the Rust and Web/WASM surfaces.
- Open the [Package Lab](/play) to inspect an artifact in the browser.
