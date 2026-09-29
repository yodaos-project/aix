# AIX 发布指南

本文说明 AIX workspace 的版本策略，以及 Rust crate 和 npm 包的发布流程。

## 版本策略

AIX 使用语义化版本。一次发布使用同一个版本号，覆盖 Rust workspace crate 和两个 npm 包：

- Rust crate：`aiui-aix`、`aiui-aix-pack`、`aiui-aix-web`
- npm 包：`@yodaos-pkg/aix`、`@yodaos-pkg/aix-cli`

例如发布 `0.10.1` 时，需要同步修改三个 `crates/*/Cargo.toml` 的 package 版本、内部 path dependency 版本约束、`Cargo.lock`、两个 npm `package.json` 以及两个 npm lockfile。`docs` 是独立的文档站点包，不参与 AIX 版本发布。

版本类型按以下规则选择：patch 用于修复、文档更新和向后兼容的元数据扩展；minor 用于向后兼容的 API 或格式能力；major 用于不兼容的 API、包格式或 manifest 变化。

已经发布过的版本号不能重复使用。Git tag 使用 `v<version>`，例如 `v0.10.1`。

## 发布检查清单

1. 将实现和文档变更合并到 `main`。
2. 更新上述所有包的版本，并检查 lockfile 的变化。
3. 本地运行检查：

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

4. 提交版本升级 PR，确认 CI 全部通过。
5. 合并后创建并推送带注释的 tag：

   ```bash
   git checkout main
   git pull --ff-only
   git tag -a v0.10.1 -m "Release v0.10.1"
   git push origin v0.10.1
   ```

## 发布 Rust Crate

通过 GitHub Actions 的 `Publish Crates` workflow 手动触发发布：

1. 使用 `crate=all`、`dry_run=true` 运行一次。
2. 检查打包内容并解决所有打包错误。
3. 使用 `crate=all`、`dry_run=false` 再运行一次。

该 workflow 按依赖顺序发布 `aiui-aix`、`aiui-aix-pack` 和 `aiui-aix-web`。也可以手动发布：

```bash
cargo publish -p aiui-aix --dry-run
cargo publish -p aiui-aix-pack --dry-run
cargo publish -p aiui-aix-web --dry-run
cargo publish -p aiui-aix --token "$CARGO_REGISTRY_TOKEN"
cargo publish -p aiui-aix-pack --token "$CARGO_REGISTRY_TOKEN"
cargo publish -p aiui-aix-web --token "$CARGO_REGISTRY_TOKEN"
```

Token 必须通过安全的环境变量提供，不能提交到仓库或输出到 workflow 日志。发布依赖 crate 后，应等待 crates.io 索引传播，再发布依赖它的 crate。

## 发布 npm 包

### `@yodaos-pkg/aix-cli`

`Publish npm CLI` workflow 会构建 Node.js WASM bundle 并发布 `packages/cli`。

1. 使用 `dry_run=true` 运行 workflow。
2. 确认 dry-run tarball 包含 `dist/cli.js`、WASM 文件和正确的包元数据。
3. 确认仓库已配置 `NPM_TOKEN` secret 后，使用 `dry_run=false` 再运行一次。

等价的本地命令：

```bash
cd packages/cli
npm ci
npm run build
npm publish --dry-run
# npm publish --access public
```

### `@yodaos-pkg/aix`

当前没有专门发布 Web/WASM npm 包的 GitHub Actions workflow。应先构建发布目录，再发布 `crates/aix-web/dist`：

```bash
cd crates/aix-web
npm ci
npm run build
npm publish ./dist --access public --dry-run
# npm publish ./dist --access public
```

构建脚本会从 `crates/aix-web/Cargo.toml` 读取版本，并写入生成的 `dist/package.json`，所以必须先更新 Rust crate 版本再构建。发布前要检查生成包中的版本号。

## 发布后

- 在 crates.io 和 npm 检查已发布版本。
- 在干净的临时环境安装准确版本并执行 smoke test：

  ```bash
  npm install --global @yodaos-pkg/aix-cli@0.10.1
  aix --help
  ```

- 确认 GitHub Release 使用 `v<version>` tag，并包含发布说明。
- 将手动发布步骤或 workflow 缺口记录到后续 issue。
