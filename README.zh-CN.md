# AIX

AIX 是一种面向 AI agents 的可执行包格式。

它把页面、schema 和 tools 打包成一个可分发的 artifact，同时保持对 Rust 工具、命令行和浏览器环境可读。

## 仓库包含什么

这个仓库以 Rust workspace 为核心，包含多层面向格式本身的实现：

- `crates/aix`：支持 `no_std + alloc` 的包读取、页面分析和 tool 推导核心
- `crates/aix-pack`：面向 Native 和 Web/WASM 的纯内存打包与资源优化能力
- `crates/aix-web`：面向浏览器的 WASM 和 TypeScript 接口
- `packages/cli`：通过 npm 发布的命令行入口（`@yodaos-pkg/aix-cli`），基于同一 WASM 引擎的 TypeScript 壳
- `docs`：官方文档站，包含 `Specification`、`API` 和 `Play`

## Workspace 结构

```text
.
├── crates/
│   ├── aix/
│   ├── aix-pack/
│   └── aix-web/
├── packages/
│   └── cli/
├── docs/
├── Cargo.toml
└── README.zh-CN.md
```

## AIX 承载什么

`.aix` 不只是一个 zip 包，它是一个保留结构语义的 artifact，可以承载：

- 包内文件与版本元信息
- 应用和页面定义
- schema 定义的输入契约
- 布局与目标环境提示
- 面向 AI agents 的派生 tool surface

因此，同一个 `.aix` 包可以被 Rust、CLI 和 Web/WASM 工具链读取，而不丢失其原生结构。

## API

### `crates/aix`

核心 crate 定义了 AIX 的读取模型，主要负责：

- 列出包内文件
- 读取归档中的具体文件
- 解析版本和标题元信息
- 解析页面定义
- 提取页面 schema
- 基于包内容推导 tool 定义

关闭默认 feature 后，核心包支持 `no_std + alloc`。

### `crates/aix-pack`

`aix-pack` 完全在内存中构建和优化 `.aix`，负责文本规范化、JSON 紧凑化以及使用纯 Rust 编解码器优化 PNG/JPEG，并由 CLI 和 Web/WASM 共同使用。

### `packages/cli`

npm CLI 使用 Rust 打包引擎，提供 `aix` 命令：

- **npm**：`npm install -g @yodaos-pkg/aix-cli`（源码在 `packages/cli`，是把 Rust 引擎编译成 Node.js WASM 包的 TypeScript 壳）

CLI 把格式能力转成终端工作流，目前主要包括：

- `aix pack <INPUT_DIR>`：构建 `.aix` artifact
- `aix list <AIX_FILE>` 或 `aix ls <AIX_FILE>`：检查包内容
- `aix optimize <AIX_FILE> -o <OUTPUT_FILE>`：优化已有 artifact
- 打包阶段的校验与规范化处理
- `.aixignore` 支持与可选优化流程

### `crates/aix-web`

Web 包通过 WASM 和 TypeScript 暴露同一套 AIX 能力。

典型接口包括：

- `AIX.From(data)`，从 `Uint8Array` 或 `File` 初始化
- `list()`，获取包内文件
- `readFile(name)`，读取原始文件
- `getVersion()`、`getTitle()`、`getPages()` 和 `getTools()`

这个包对应的官方浏览器入口是 `docs/play.md`，发布后的路由是 `/play`。

## 典型包结构

```text
.
├── AGENTS.md
├── VERSION
├── app.json
├── app.js
└── pages/
```

这些文件通常表示：

- `AGENTS.md`：agent 身份和能力上下文
- `VERSION`：包版本
- `app.json`：应用级元数据和路由配置
- `app.js`：运行时入口逻辑
- `pages/`：页面定义、资源以及包含 schema 的文件

## 快速开始

### 在 Rust 中读取 AIX

```rust
use aix::AixReader;

fn main() -> anyhow::Result<()> {
    let data = std::fs::read("bundle.aix")?;
    let reader = AixReader::new(data)?;

    println!("version = {:?}", reader.get_version());
    println!("title = {:?}", reader.get_title());
    println!("pages = {:?}", reader.get_pages());
    println!("tools = {:?}", reader.get_tools());
    println!(
        "supports engine 0.14.2 = {}",
        reader.supports_engine("0.14.2")?
    );

    Ok(())
}
```

### 用 CLI 打包或检查

通过 npm 安装 `aix`：

```bash
npm install -g @yodaos-pkg/aix-cli

```

```bash
aix pack ./my-agent -o bundle.aix --engine '^0.14.0'
aix list ./bundle.aix
```

如果没有显式传入 `--engine`，打包器会依次回退到 `app.json.engine` 和
`*`，并把最终解析出的范围写入 `META-INF/aix/manifest.json`。对于已经打
包好的 artifact，读取侧会把 manifest 作为 engine 判断的唯一来源。像
`0.14.0` 这样的完整版本表示精确匹配，`>=0.14.0`、`^0.14.0` 这样的范围
也都支持。

### 在 Rust 中签名和校验

下面的例子展示了如何在打包时写入 engine 范围、为包签名，并在读取时校验
签名与运行时兼容性：

```rust
use aix::crypto::{PrivateKey, PublicKey};
use aix_pack::{pack, InputFile, PackOptions};
use rand_core::OsRng;

fn main() -> anyhow::Result<()> {
    let private_key = PrivateKey::generate(&mut OsRng);
    let mut options = PackOptions::new("build-1");
    options.engine = Some("^0.14.0".into());
    options.signing_key = Some(&private_key);

    let output = pack(
        vec![InputFile::new("app.json", br#"{"pages":[]}"#)],
        options,
    )?;

    let trusted_key: PublicKey = private_key.public_key();
    let reader = AixReader::new(output.data)?;
    let report = reader.verify_signature(&trusted_key)?;

    assert!(reader.supports_engine("0.14.2")?);
    println!("verified package {}", report.package_id);
    Ok(())
}
```

对于已打包 artifact，`supports_engine()` 读取的是
`META-INF/aix/manifest.json` 中已经解析好的 engine 范围。

### 在浏览器中读取 AIX

```ts
import { AIX } from "@yodaos-pkg/aix";

async function inspect(file: File) {
  const aix = await AIX.From(file);
  console.log(aix.getTitle());
  console.log(aix.getPages());
  console.log(aix.getTools());
  console.log(aix.supportsEngine("0.14.2"));
}
```

对于已打包 artifact，`supportsEngine()` 同样会从
`META-INF/aix/manifest.json` 读取 engine 范围。

## 文档站

文档站位于 `docs/`，当前主要包括三个路由：

- `/spec`：AIX Specification 概览
- `/api`：`@yodaos-pkg/aix` WebAssembly 和 TypeScript API 参考
- `/play`：在浏览器里上传并检查真实的 `.aix` artifact

本地启动：

```bash
cd docs
npm install
npm run dev
```

## 开发

版本策略、发布检查清单、crate 发布 workflow 和 npm 发布命令请参阅
[RELEASING.zh-CN.md](./RELEASING.zh-CN.md)。

在仓库根目录进行基础校验：

```bash
cargo test -p aiui-aix -p aiui-aix-pack
cargo check -p aiui-aix-web --target wasm32-unknown-unknown
```

构建 Web 包输出：

```bash
cd crates/aix-web
npm install
npm run build
```

## License

MIT
