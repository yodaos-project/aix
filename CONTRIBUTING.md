# Contributing to AIX

Thanks for contributing to AIX.

AIX is an executable package format for AI agents. This repository centers on a Rust workspace with these package-facing surfaces:

- `crates/aix`: core Rust library
- `crates/aix-pack`: in-memory Native/WASM packer and optimizer
- `crates/aix-web`: WASM and TypeScript package
- `packages/cli`: npm-published CLI (`@yodaos-pkg/aix-cli`)
- `docs`: the VitePress documentation site

## Before You Start

Make sure your local environment includes:

- Rust stable
- the `wasm32-unknown-unknown` target
- `wasm-pack`
- Node.js 18+ and npm

Recommended setup:

```bash
rustup toolchain install stable
rustup default stable
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
node --version
npm --version
```

## Repository Layout

```text
.
├── crates/
│   ├── aix/
│   ├── aix-pack/
│   └── aix-web/
├── packages/
│   └── cli/
├── docs/
├── .github/workflows/
├── Cargo.toml
└── CONTRIBUTING.md
```

The workspace is intentionally split by surface area:

- `aix` owns the package model and analysis logic
- `aix-pack` owns in-memory packaging, normalization, and resource optimization
- `aix-web` exposes the same capabilities through WASM and TypeScript
- `packages/cli` is the npm-published command-line interface (`@yodaos-pkg/aix-cli`, binary `aix`) — a TypeScript shell over the same engine
- `docs` is the official site for `Specification`, `Packages`, and `Play`

## Build From Source

### Build and validate the Rust workspace

From the repository root:

```bash
cargo test -p aiui-aix -p aiui-aix-pack
cargo check -p aiui-aix-web --target wasm32-unknown-unknown
```

This matches the baseline validation performed in CI.

### Build the web package

The npm package in `crates/aix-web` depends on the Rust/WASM build output.

```bash
cd crates/aix-web
npm install
npm run build
```

What this does:

- builds the WASM artifact with `wasm-pack`
- compiles the TypeScript entrypoint
- generates publish-ready files under `crates/aix-web/dist`

Important: publishable npm metadata is generated into `dist/package.json`. Do not publish from the package root unless you intentionally know what you are changing.

### Run the docs site locally

The docs site consumes the local web package during development.

```bash
cd docs
npm install
npm run dev
```

## Development Workflow

Typical workflow:

1. Create a branch for your change.
2. Implement the change in the smallest relevant surface.
3. Run the validation commands relevant to that surface.
4. Update docs when behavior, commands, or package interfaces change.
5. Open a pull request with a clear summary and verification notes.

When changing multiple surfaces, prefer validating them in this order:

1. `crates/aix`
2. `crates/aix-pack`
3. `crates/aix-web`
4. `packages/cli`
5. `docs`

This keeps the core model stable before checking derived interfaces.

## Code Style

### General

- Keep changes focused and modular.
- Prefer small, explicit functions over large mixed-responsibility blocks.
- Preserve existing naming and file organization patterns.
- Add comments only when intent would otherwise be hard to infer.

### Rust

- Run `cargo fmt --all` before opening a pull request.
- Prefer returning structured errors instead of panicking in normal flows.
- Keep public APIs small and readable.
- When adding parsing or package logic, include targeted tests when practical.

### TypeScript and web

- Keep the TypeScript surface aligned with the Rust/WASM API.
- Avoid introducing bundler-specific behavior unless it is necessary for `docs` or package distribution.
- Make build outputs reproducible from source; do not hand-edit generated `dist/` artifacts.

### Documentation and terminology

- Use `AI agents`, not `AI applications`.
- Use `Play`, not `Package Lab`.
- Use `Specification`, not `Format`, when referring to the docs information architecture.
- Keep top-level repository docs in English.
- When changing contributor or user-facing behavior, update the relevant docs in the same pull request when possible.

## Testing and Validation

Run the smallest useful set of checks for your change, and expand only when the change crosses package boundaries.

Common commands:

```bash
# Rust validation
cargo test -p aiui-aix -p aiui-aix-pack
cargo check -p aiui-aix-web --target wasm32-unknown-unknown

# Web package build
cd crates/aix-web && npm install && npm run build

# npm CLI build and smoke test
cd packages/cli && npm install && npm run build && node dist/cli.js --help

# Docs site build
cd docs && npm install && npm run build
```

If your change affects packaging, CLI output, WASM bindings, or docs integration, mention the exact commands you ran in the pull request description.

## Release Process

This repository currently has two release tracks:

1. crates published to crates.io
2. the web package published to npm

### Release checklist

Before any release:

1. Make sure the intended version is correct in crate manifests.
2. Run baseline validation from the repository root.
3. Confirm README and docs still match the shipped behavior.
4. Make sure the release commit is merged and tagged according to your release policy.

### Publish crates to crates.io

The repository already includes a manual GitHub Actions workflow at `.github/workflows/publish-crates.yml`.

It supports `aiui-aix`, `aiui-aix-pack`, `aiui-aix-web`, or `all`, with an optional dry run. Publish the crates in that dependency order and allow the crates.io index to update between dependent packages. The workflow expects `CARGO_REGISTRY_TOKEN` in GitHub Actions secrets for real releases.

Validate locally with:

```bash
cargo publish -p aiui-aix --dry-run
cargo publish -p aiui-aix-pack --dry-run
cargo publish -p aiui-aix-web --dry-run
```

### Publish the npm CLI package

The npm CLI package lives under `packages/cli` and is published as `@yodaos-pkg/aix-cli`.

Recommended process:

```bash
cd packages/cli
npm install
npm run build
npm pack --dry-run
npm publish --access public
```

The repository also includes a manual GitHub Actions workflow at `.github/workflows/publish-npm-cli.yml`; it expects `NPM_TOKEN` in GitHub Actions secrets for non-dry-run releases.

Notes:

- The build runs `wasm-pack build --target nodejs` from `crates/aix-web` and then bundles `src/cli.ts` with esbuild into `dist/cli.js`, copying the WASM artifact to `dist/pkg/`.
- The WASM wrapper (`dist/pkg/aix_web.js`) resolves `aix_web_bg.wasm` relative to its own `__dirname`, so the two files must stay adjacent — do **not** bundle the wrapper into cli.js (the `require(path.join(__dirname, 'pkg', ...))` in `src/wasm.ts` is intentional).
- `ignore` is the only runtime dependency (`.aixignore` support); it is external in the esbuild bundle.
- The npm CLI build prefers `wasm-pack --no-opt` to skip binaryen/wasm-opt downloads, but falls back to the default `wasm-pack build` flow when the local `wasm-pack` version does not support that flag.
- The published version is generated from `crates/aix-web/Cargo.toml`, not copied from the source `package.json` version field.
- `npm run build` creates `dist/package.json`, copies the README, and prepares `dist/pkg` for publishing.
- Publish from `dist/`, not from `crates/aix-web/`.
- Make sure you are logged in to the correct npm account before publishing.

If a release includes both crates and npm:

1. publish `aiui-aix`
2. publish `aiui-aix-pack` and `aiui-aix-web`
3. build and publish `@yodaos-pkg/aix`
4. build and publish `@yodaos-pkg/aix-cli`
5. verify the docs site still resolves the released package as expected

## Pull Request Guidelines

- Keep each pull request scoped to one logical change.
- Include context, what changed, and how you verified it.
- Link related issues when relevant.
- Call out breaking changes explicitly.
- Include screenshots only when the docs UI or `Play` experience changes.

Good pull requests are easy to review because they explain:

- what changed
- why it changed
- how it was validated

## Reporting Issues

When filing an issue, include as much of the following as possible:

- the package or surface involved (`aix`, `aix-pack`, `aix-web`, `aix-cli`, or `docs`)
- reproduction steps
- expected behavior
- actual behavior
- sample `.aix` artifact or minimal fixture when relevant
- platform details if the issue appears environment-specific

## Questions

If you are unsure where a change belongs, start with the smallest surface that can own it cleanly. If the change affects package semantics, begin in `crates/aix` and let the CLI, web, and docs layers follow from there.
