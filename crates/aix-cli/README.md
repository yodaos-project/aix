# AIX CLI

AIX CLI is a command-line utility for managing AIX packages. It packs
directories into `.aix` artifacts, inspects package contents, and optimizes
existing archives.

## Installation

You can build and install it locally using Cargo:

```bash
cargo install --path crates/aix-cli
```

Once installed, the `aix` command will be available in your shell.

If you want to install the published crate from crates.io:

```bash
cargo install aiui-aix-cli
```

## Core Features

### 1. Pack

Packs a specified directory into an `.aix` file. During packing, a unique
`VERSION` file (UUID v4) is generated automatically and placed at the archive
root.

The packer also validates all `.json` files before writing them into the
archive. If a JSON file is invalid, packing fails with an error. For `.json`,
`.js`, and `.ink` files, non-UTF-8 input is converted to UTF-8 inside the
packaged `.aix` output without modifying the source files on disk.

In addition, `.js` and `.ts` files are minified by default while packing. This
default script processing is independent from `--optimize`: it runs even when
you do not pass `--optimize` or `-O`.

**Basic Usage:**

```bash
aix pack <INPUT_DIR>
```
*Defaults to `bundle.aix` if output path is not specified.*

**Specify Output Path:**

```bash
aix pack <INPUT_DIR> -o my-app.aix
```

**Default JS/TS Processing:**

```bash
# JS/TS files are processed even without --optimize
aix pack <INPUT_DIR>
```

When `.js` or `.ts` files are included in the input directory:

- their packaged contents are minified by default
- source files on disk are not modified
- paths and file extensions inside the package stay the same
- non-script files continue to follow the existing packing rules

If script minification fails, packing stops with an error that includes the
file path. The CLI does not silently fall back to copying the original script
into the archive unchanged.

**Enable Optimization:**

AIX CLI supports additional resource optimization to reduce package size.
`--optimize` only enables JSON minification plus PNG/JPEG compression. It does
not turn JS/TS minification on or off.

```bash
# Enable default optimization (Level 2)
aix pack <INPUT_DIR> --optimize

# Specify optimization level (1-3)
aix pack <INPUT_DIR> -O --opt-level 3
```

`--opt-level` applies to the JSON/PNG/JPEG optimization pipeline enabled by
`--optimize`. It does not change the default JS/TS minification behavior.

**Ignore Files:**

The packer respects `.aixignore` files within the source directory, using the same syntax as `.gitignore`. Use it to exclude source code, documentation, or temporary files.

### 2. Show

Prints the effective Agent Definition JSON without requiring ADB. This uses the
same resolver as `install`.

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### 3. Install

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

### 4. Device And Launch

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

### 5. List

Lists all files and their size information within an `.aix` package.

```bash
aix list <AIX_FILE>
# or use the alias
aix ls <AIX_FILE>
```

### 6. Optimize

Optimizes JSON, PNG, and JPEG entries in an existing package using the same
cross-platform engine as the Web/WASM package.

This command is different from the default JS/TS processing in `aix pack`:

- `aix pack` minifies `.js` and `.ts` files by default while creating a new package
- `aix pack --optimize` additionally minifies JSON and compresses PNG/JPEG inputs
- `aix optimize` reprocesses an existing `.aix` package for JSON/PNG/JPEG only

```bash
aix optimize input.aix -o output.aix --level 2
```

## Development & Debugging

If you are at the project root, you can run the CLI directly using:

```bash
cargo run -p aiui-aix-cli -- pack ./my-agent -o test.aix
```

## License

MIT
