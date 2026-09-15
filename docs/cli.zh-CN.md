# CLI

[English](./cli.md)

AIX CLI 用于打包 Agent 目录、查看归档内容、优化 `.aix`，以及在 Rokid Glasses 上安装和调试 Agent。它提供两种安装方式，二者都暴露相同的 `aix` 命令并共享 Rust 打包引擎：

- **npm**（`@yodaos-pkg/aix-cli`）：基于 Node.js WASM 的 TypeScript 外壳。
- **Native Rust**（`aiui-aix-cli`）：由同一引擎编译的原生二进制。

## 安装

任选一种方式：

```bash
# npm，无需 Rust 工具链
npm install -g @yodaos-pkg/aix-cli

# 或原生 Rust
cargo install aiui-aix-cli
```

```bash
aix --help
```

## 打包目录

```bash
aix pack ./my-agent
aix pack ./my-agent -o dist/my-agent.aix
```

默认输出为 `bundle.aix`。打包过程会：

- 遵循 `.aixignore` 和标准 ignore 文件；
- 校验 JSON；
- 校验 app contract，缺少 `app.json` 时拒绝打包；
- 按需将支持的文本转换为 UTF-8；
- 生成唯一的 `VERSION` build ID；
- 生成包含摘要和包元数据的 `META-INF/aix/manifest.json`；
- 默认压缩 JS/TS，但不修改源文件。

### Engine 兼容范围

```bash
aix pack ./my-agent --engine '^0.14.0'
```

常见写法：

```text
*           任意版本
0.14.0      仅 0.14.0
>=0.14.0    0.14.0 或更高
^0.14.0     与 0.14.0 兼容的版本
```

未传 `--engine` 时依次使用 `app.json.engine` 和 `*`。最终范围写入 manifest，运行时只以 manifest 为准。

### 打包时优化

```bash
aix pack ./my-agent -O
aix pack ./my-agent -O --opt-level 3
```

`--optimize`/`-O` 启用 JSON、PNG 和 JPEG 优化；等级为 1–3，默认 2。优化只修改输出包，不修改源文件，也不控制默认的 JS/TS 压缩。

### Pack 参数

```text
Usage: aix pack [OPTIONS] <INPUT_DIR>

Options:
  -o, --output <OUTPUT_FILE>  输出文件，默认为 bundle.aix
  -O, --optimize              启用资源优化
      --opt-level <LEVEL>     优化等级 1–3，默认为 2
      --engine <RANGE>        支持的 engine 范围
  -h, --help                  显示帮助
```

## 查看包内容

```bash
aix list ./bundle.aix
aix ls ./bundle.aix
```

输出归档条目及压缩前后大小。包元数据（如 engine 范围）来自 `META-INF/aix/manifest.json`。

## 优化已有包

```bash
aix optimize ./bundle.aix -o ./bundle.optimized.aix
aix optimize ./bundle.aix -o ./bundle.optimized.aix --level 3
```

优化器保留 engine 范围，但内容变化会使旧签名失效，因此会删除签名并生成新的未签名 manifest；需要验签时应重新签名。

## 在浏览器中预览

```bash
aix preview ./bundle.aix
aix preview ./my-agent
```

默认启动本地 HTTP 服务，从内存提供预览页并打印 URL，不自动打开浏览器。默认 `blank` 目标视口为 `480x352`；current 目标固定为 `448x150`：

```bash
aix preview ./bundle.aix --launch --launch-target current
```

使用 `--launch` 自动打开默认浏览器。归档输入从 manifest 读取元数据，目录输入直接读取源码树。

### 导出静态预览

```bash
aix preview ./bundle.aix --html-out ./artifacts/preview.html
```

使用 `--html-out` 时：

- 写入指定 HTML，自动创建父目录；
- 相对路径从当前工作目录解析；
- 不启动本地服务器和浏览器；
- 不能同时使用 `--launch`。

预览通过 import map 从 `jspm.io` 加载 Ink，版本由 `aix runtime select` 决定；未选择时使用当前 `AIX_NPM_REGISTRY` 的 `latest`。预览侧栏会显示实际 Ink 版本。

### 开发模式

```bash
aix preview ./bundle.aix --dev
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

`--dev` 模式始终启动服务器，通过 WebSocket 监听包或目录变化并重建 `InkView`，无需刷新整个页面；不能与 `--html-out` 一起使用。

## 查看 Agent 元数据

```bash
aix show ./my-agent
aix show ./bundle.aix
aix show ./my-agent --output ./agent.json
aix show ./my-agent --compact
```

`show` 与 `install` 使用同一个解析器，因此输出就是安装时上传的 Definition。标准输出只包含 JSON。显式 Definition 可通过 `--definition <file>` 指定，但 `agentId` 仍会替换为目录或包推导出的稳定 ID。目录首次解析时会创建 `.aix/agent-id`。

## 安装到 Rokid Glasses

```bash
aix install ./my-agent
aix install ./bundle.aix
```

目录首次执行会创建并持续复用 `.aix/agent-id`；已有 `.aix` 从根目录 `VERSION` 推导 ID，最终格式为 `develop.rokid.agent.<id>`。`.aix/` 状态目录不会被打进包中。

如果 `<project>/agent.json` 存在，除 `agentId` 外均以其中字段为准；否则从 `app.json` 和 `AGENTS.md` 第一段有效文本生成，并补齐协议默认值。

```bash
aix install ./my-agent --definition ./develop-agent.json --serial <serial>
```

设备必须已开启 Developer Mode 并完成 ADB 授权。单台在线设备自动选择，多台设备使用交互式选择器，非交互环境必须传 `--serial`。

命令执行 `prepare -> push -> apply`，限制包不超过 128 MiB，并校验 `result_data`、`agentId`、Apply outcome 和 `operationId`。随后查询一次上传状态：`UPLOADED` 只表示手机确认上传，不代表云端已完成索引；失败状态会使命令退出。

```bash
aix install ./my-agent --engine '>=0.17.0' --optimize --opt-level 2
```

## 设备开发者模式

设备操作在当前阶段显示 spinner。中间阶段完成后会清除详情或仅保留一行 `✔`，只有最终结果保留 Agent、路径、设备和运行时等信息。

```bash
aix device
aix device set-dev
aix device unset-dev
```

切换 Developer Mode 会重载所有 AIUI Widget：常驻 Widget 自动重建，动态 Widget 需要重新 launch。所有设备命令都支持 `--serial <serial>`。

只读的 `aix device` 还会列出 `/sdcard/aiui/package/*.aix` 中实际安装的 Agent，并从 `developer_manifest.json` 补充名称和版本等元数据。

## 启动已安装页面

```bash
aix launch-page ./my-agent
aix launch-page ./my-agent pages/index/index
aix launch-page ./my-agent pages/index/index --card
```

使用 `--params <JSON>` 或 `--params-file <FILE>` 传参。该命令只调用 DEVELOP `open`，不会安装或上传 Agent。

## 配置并启动动态 Widget

```bash
aix launch-widget ./my-agent widgets/order/index
aix launch-widget ./my-agent widgets/order/index --position 2
```

每次执行都会读取 `widget-snapshot.configurationJson`；无配置时从 4 格、2 列的空布局开始。尺寸仅从 `app.json.widgets[].family` 推导，不提供 size 参数。

布局策略：

- 同一 Widget 复用原位置；
- 有兼容空位时保留已有动态 Widget；
- 指定位置冲突时自动替换相交的动态 Widget；
- 完全放不下时清空旧的动态 placements 后重新放置；
- 永远不自动删除常驻 Widget。

布局变化时执行 `prepare -> push widget-config.json -> widget-apply -> open`。布局未变化时直接 Open；如果设备返回 `WIDGET_PLACEMENT_MISSING`，CLI 会重新提交已保存布局并重试一次。

Widget module 使用设备要求的 `custom_<序号>` 名称。布局 apply 会重载全部 Widget，其他动态 Widget 需要重新启动。

## 查看或清空 Widget 布局

```bash
aix widget-layout
aix widget-layout show
aix widget-layout --clear
aix widget-layout --clear --yes
```

Show 只读。Clear 保留网格参数，将常驻和动态数组清空；默认要求确认，布局已经为空时不会执行 `widget-apply`。

## 查看预览 Runtime 版本

```bash
AIX_NPM_REGISTRY=npm aix runtime versions
AIX_NPM_REGISTRY=npmmirror aix runtime versions

aix runtime versions
aix runtime current
aix runtime select
```

- `versions` 列出当前、已选择及发布的稳定版本；
- `current` 只打印解析后的版本字符串，适合脚本；
- `select` 打开交互选择器并保存到 `~/.aix/runtime.json`。

支持的 registry 为 `npm`（默认）和 `npmmirror`。

## 从工作区运行

开发时无需全局安装：

```bash
# npm
cd packages/cli
npm install
npm run build
node dist/cli.js pack ./my-agent -o bundle.aix

# 原生 Rust
cargo run -p aiui-aix-cli -- pack ./my-agent -o bundle.aix
```

```bash
node dist/cli.js pack --help
node dist/cli.js runtime --help
```

## 相关文档

- [规范](/spec)
- [Packages](/packages)
- [Play](/play)
