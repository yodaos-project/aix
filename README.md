# AIX

**Build, inspect, and deliver AI agents with interactive interfaces.**

AIX (AI eXecutable) is a portable package format for AI agents. One `.aix` file brings together an agent's resources, Ink pages and widgets, input schemas, and the metadata a host needs to present them. AIX derives an agent-facing tool surface from those pages, so the same package can be inspected by developers and used by a compatible runtime.

AIX extends the [Open Agent Format](https://openagentformat.com/spec.html): agent instructions and other OAF resources remain intact while AIX adds an inspectable UI and a distributable archive.

[简体中文](README.zh-CN.md) · [Specification](docs/spec.md) · [CLI guide](docs/cli.md) · [Web API](docs/api.md) · [Play](docs/play.md)

## Get started

Install the CLI from npm, then check and package an AIX project:

```bash
npm install -g @yodaos-pkg/aix-cli

aix check ./my-agent
aix pack ./my-agent -o my-agent.aix
aix list ./my-agent.aix
```

`check` reports static source issues without creating a package. `pack` writes the distributable `.aix` file and its manifest. `list` shows its contents. For an existing project, use `aix preview ./my-agent` to open a local preview, or `aix show ./my-agent` to inspect the effective Agent Definition.

An AIX project typically looks like this:

```text
my-agent/
├── AGENTS.md             # Agent identity and instructions
├── app.json              # Pages, widgets, and app metadata
└── pages/                # Ink page code and assets
```

The packer generates `VERSION` and `META-INF/aix/manifest.json`; keep those generated files out of the source directory. See the [Specification](docs/spec.md) for the complete package model and page formats.

## From source to device

| Step | What it does | Command |
| --- | --- | --- |
| Check | Find supported source and permission issues | `aix check ./my-agent` |
| Preview | Run a local Ink preview | `aix preview ./my-agent` |
| Package | Produce a portable artifact | `aix pack ./my-agent -o my-agent.aix` |
| Inspect | Examine files or the effective Agent Definition | `aix list my-agent.aix` / `aix show my-agent.aix` |
| Install | Submit the agent to a connected Rokid Glasses device over ADB | `aix install my-agent.aix` |

The CLI also supports launching installed pages and widgets, device inspection, and package optimization. See the [CLI guide](docs/cli.md) for options and device requirements. Static checks and a local preview are development aids; behavior on a device depends on the host runtime.

## What the package provides

- **An inspectable interface.** Ink pages and widgets, assets, and app metadata travel together in one archive.
- **Agent-callable pages.** Page schemas become OpenAI-style tool definitions with layout and target hints; the host decides how to use them.
- **Portable inspection.** Read package files, pages, and derived tools from Rust, a browser, or the CLI.
- **Integrity and compatibility metadata.** Generated manifests record entry digests, a content-derived package ID, and an engine range. Publishers can optionally sign a package with Ed25519; a verifier must use a trusted public key.

## Use AIX in your application

| Surface | Package | Use it for |
| --- | --- | --- |
| Rust reader | [`aiui-aix`](crates/aix) | Read packages, inspect pages and tools, and verify signatures; supports `no_std + alloc`. |
| Rust packer | [`aiui-aix-pack`](crates/aix-pack) | Build and optimize packages in memory. |
| Web / TypeScript | [`@yodaos-pkg/aix`](crates/aix-web) | Read, pack, and inspect `.aix` files through WASM. |
| CLI | [`@yodaos-pkg/aix-cli`](packages/cli) | Check, preview, package, inspect, and install from a terminal. |

For example, the Web package can inspect a file selected in the browser:

```ts
import { AIX } from "@yodaos-pkg/aix";

const packageFile = await AIX.From(file);
console.log(packageFile.getPages());
console.log(packageFile.getTools());
```

See the [Web API](docs/api.md) for setup and the full API. You can also use [Play](docs/play.md) to inspect an `.aix` file in the browser.

## Develop this repository

The implementation lives in `crates/aix`, `crates/aix-pack`, and `crates/aix-web`; the npm CLI lives in `packages/cli`, and the documentation site in `docs`.

```bash
cargo test -p aiui-aix -p aiui-aix-pack
cargo check -p aiui-aix --no-default-features
cargo check -p aiui-aix-web --target wasm32-unknown-unknown
```

The WASM check needs the `wasm32-unknown-unknown` target. See [RELEASING.md](RELEASING.md) for versioning and publishing.

## License

MIT
