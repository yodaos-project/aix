# AIX

**构建、检查并交付带有交互界面的 AI Agent。**

AIX（AI eXecutable）是一种面向 AI Agent 的可移植包格式。一个 `.aix` 文件汇集 Agent 资源、Ink 页面与 Widget、输入 Schema，以及宿主展示这些内容所需的元数据。AIX 还能从页面派生面向 Agent 的 Tool，让开发者和兼容的运行时读取同一个包。

AIX 扩展了 [Open Agent Format](https://openagentformat.com/spec.html)：保留 Agent 指令及其他 OAF 资源，同时加入可检查的界面与可分发的归档文件。

[English](README.md) · [格式规范](docs/zh-CN/spec.md) · [CLI 指南](docs/zh-CN/cli.md) · [Web API](docs/zh-CN/api.md) · [Play](https://yodaos-project.github.io/aix/play)

## 快速开始

通过 npm 安装 CLI，然后检查并打包 AIX 项目：

```bash
npm install -g @yodaos-pkg/aix-cli

aix check ./my-agent
aix pack ./my-agent -o my-agent.aix
aix list ./my-agent.aix
```

`check` 只报告静态源码问题，不生成包；`pack` 生成可分发的 `.aix` 文件及其 Manifest；`list` 列出包内文件。已有项目还可以用 `aix preview ./my-agent` 打开本地预览，或用 `aix show ./my-agent` 查看最终生效的 Agent Definition。

典型的 AIX 项目结构如下：

```text
my-agent/
├── AGENTS.md             # Agent 身份与指令
├── app.json              # 页面、Widget 与应用元数据
└── pages/                # Ink 页面代码与资源
```

打包器会生成 `VERSION` 和 `META-INF/aix/manifest.json`，源码目录中无需放置这些文件。完整包结构和页面格式见[格式规范](docs/zh-CN/spec.md)。

## 从源码到设备

| 步骤 | 作用 | 命令 |
| --- | --- | --- |
| 检查 | 发现支持范围内的源码与权限问题 | `aix check ./my-agent` |
| 预览 | 启动本地 Ink 预览 | `aix preview ./my-agent` |
| 打包 | 生成可移植的文件 | `aix pack ./my-agent -o my-agent.aix` |
| 查看 | 检查文件或最终生效的 Agent Definition | `aix list my-agent.aix` / `aix show my-agent.aix` |
| 安装 | 通过 ADB 提交到已连接的 Rokid Glasses 设备 | `aix install my-agent.aix` |

CLI 还支持启动已安装的页面和 Widget、查看设备状态以及优化包。命令选项与设备要求见 [CLI 指南](docs/zh-CN/cli.md)。静态检查和本地预览用于开发；设备上的实际行为取决于宿主运行时。

## AIX 包提供什么

- **可检查的界面。** Ink 页面、Widget、资源及应用元数据保存在同一个归档中。
- **Agent 可调用的页面。** 页面 Schema 派生为 OpenAI 风格的 Tool 定义，附带布局与目标提示；宿主决定如何使用。
- **跨环境读取。** Rust、浏览器和 CLI 都能读取包文件、页面及派生 Tool。
- **完整性与兼容性元数据。** 生成的 Manifest 记录文件摘要、基于内容计算的 Package ID 和 Engine 范围。发布者可选用 Ed25519 签名；校验方必须使用可信公钥。

## 在应用中使用 AIX

| 接口 | 包 | 用途 |
| --- | --- | --- |
| Rust 读取器 | [`aiui-aix`](crates/aix) | 读取包、检查页面与 Tool、校验签名；支持 `no_std + alloc`。 |
| Rust 打包器 | [`aiui-aix-pack`](crates/aix-pack) | 在内存中构建、优化包。 |
| Web / TypeScript | [`@yodaos-pkg/aix`](crates/aix-web) | 通过 WASM 读取、打包和检查 `.aix` 文件。 |
| CLI | [`@yodaos-pkg/aix-cli`](packages/cli) | 在终端检查、预览、打包、查看和安装。 |

例如，Web 包可以检查浏览器中选取的文件：

```ts
import { AIX } from "@yodaos-pkg/aix";

const packageFile = await AIX.From(file);
console.log(packageFile.getPages());
console.log(packageFile.getTools());
```

安装和完整接口见 [Web API](docs/zh-CN/api.md)。也可以通过 [Play](https://yodaos-project.github.io/aix/play) 在浏览器中检查 `.aix` 文件。

## 开发本仓库

实现位于 `crates/aix`、`crates/aix-pack` 和 `crates/aix-web`；npm CLI 位于 `packages/cli`，文档站位于 `docs`。

```bash
cargo test -p aiui-aix -p aiui-aix-pack
cargo check -p aiui-aix --no-default-features
cargo check -p aiui-aix-web --target wasm32-unknown-unknown
```

WASM 检查需要 `wasm32-unknown-unknown` target。版本与发布流程见 [RELEASING.zh-CN.md](RELEASING.zh-CN.md)。

## License

MIT
