# AIX Release Guide

This document describes the versioning policy and release procedure for the AIX workspace.

## Versioning Policy

AIX uses Semantic Versioning. A release version is shared by the Rust workspace crates and the two published npm packages:

- Rust crates: `aiui-aix`, `aiui-aix-pack`, and `aiui-aix-web`
- npm packages: `@yodaos-pkg/aix` and `@yodaos-pkg/aix-cli`

For a release such as `0.10.1`, update all three `crates/*/Cargo.toml` package versions, internal path-dependency constraints, `Cargo.lock`, both npm `package.json` files, and both npm lockfiles. The `docs` package has an independent version and is not part of an AIX release.

Use patch releases for fixes, documentation-only changes, and backwards-compatible metadata additions; minor releases for backwards-compatible API or format features; and major releases for incompatible API, package, or manifest changes.

Do not reuse a published version. The release tag is `v<version>`, for example `v0.10.1`.

## Release Checklist

1. Merge the implementation and documentation changes into `main`.
2. Update every package version listed above and review the lockfile changes.
3. Run the checks locally:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo check -p aiui-aix-web --target wasm32-unknown-unknown
   npm ci --prefix crates/aix-web
   npm run build --prefix crates/aix-web
   npm ci --prefix packages/cli
   npm run build --prefix packages/cli
   git diff --check
   ```

4. Open a version-bump pull request and wait for CI to pass.
5. After merge, create and push the annotated tag:

   ```bash
   git checkout main
   git pull --ff-only
   git tag -a v0.10.1 -m "Release v0.10.1"
   git push origin v0.10.1
   ```

## Publishing Rust Crates

Start the `Publish Crates` workflow from GitHub Actions with `workflow_dispatch`:

1. Run `crate=all` with `dry_run=true`.
2. Inspect the package contents and resolve packaging errors.
3. Run `crate=all` with `dry_run=false`.

The workflow publishes `aiui-aix`, `aiui-aix-pack`, and `aiui-aix-web` in dependency order. All three can also be published manually:

```bash
cargo publish -p aiui-aix --dry-run
cargo publish -p aiui-aix-pack --dry-run
cargo publish -p aiui-aix-web --dry-run
cargo publish -p aiui-aix --token "$CARGO_REGISTRY_TOKEN"
cargo publish -p aiui-aix-pack --token "$CARGO_REGISTRY_TOKEN"
cargo publish -p aiui-aix-web --token "$CARGO_REGISTRY_TOKEN"
```

Keep the crates.io token in a secure environment variable. Never commit it or print it in a workflow log. Allow index propagation between publishing a dependency and its dependents.

## Publishing npm Packages

### `@yodaos-pkg/aix-cli`

The `Publish npm CLI` workflow builds the Node.js WASM bundle and publishes `packages/cli`.

1. Run the workflow with `dry_run=true`.
2. Confirm the dry-run tarball contains `dist/cli.js`, the WASM files, and the expected metadata.
3. Run it again with `dry_run=false` and the `NPM_TOKEN` repository secret configured.

Equivalent local commands:

```bash
cd packages/cli
npm ci
npm run build
npm publish --dry-run
# npm publish --access public
```

### `@yodaos-pkg/aix`

There is currently no dedicated GitHub Actions workflow for the Web/WASM npm package. Build the publishable directory and publish `crates/aix-web/dist`:

```bash
cd crates/aix-web
npm ci
npm run build
npm publish ./dist --access public --dry-run
# npm publish ./dist --access public
```

The build reads the version from `crates/aix-web/Cargo.toml` and writes it into the generated `dist/package.json`, so update the Rust crate version before building. Verify the generated package version before publishing.

## After Publishing

- Check the published versions on crates.io and npm.
- Install the exact version in a clean temporary directory and run a smoke test:

  ```bash
  npm install --global @yodaos-pkg/aix-cli@0.10.1
  aix --help
  ```

- Confirm the GitHub Release points to `v<version>` and includes release notes.
- Record manual publishing steps or workflow gaps in a follow-up issue.
