# AIX CLI

[English](README.md)

AIX CLI 是用于管理 AIX 包的命令行工具，可将目录打包为 `.aix`、查看包内容、优化已有归档，并通过 ADB 在 Rokid Glasses 上安装和调试 Agent。

## 安装

使用 Cargo 从本地源码构建：

```bash
cargo install --path crates/aix-cli
```

或安装 crates.io 上发布的版本：

```bash
cargo install aiui-aix-cli
```

安装完成后可直接使用 `aix` 命令。

## 核心功能

### 1. 打包

将目录打包为 `.aix`。打包时会在归档根目录自动生成 UUID v4 格式的 `VERSION`，校验所有 JSON，并在不修改磁盘源文件的前提下，将非 UTF-8 的 `.json`、`.js` 和 `.ink` 转换为 UTF-8。

`.js` 和 `.ts` 默认会压缩，这与 `--optimize` 相互独立：即使不传 `--optimize` 或 `-O` 也会执行。

```bash
aix pack <INPUT_DIR>
aix pack <INPUT_DIR> -o my-app.aix
```

未指定输出路径时默认为 `bundle.aix`。

打包后的脚本将保持原路径和扩展名；如果脚本压缩失败，命令会报告文件路径并终止，不会静默回退到原始内容。

使用 `--optimize` 额外启用 JSON 压缩及 PNG/JPEG 压缩：

```bash
aix pack <INPUT_DIR> --optimize
aix pack <INPUT_DIR> -O --opt-level 3
```

优化等级为 1–3，默认为 2，只影响由 `--optimize` 启用的 JSON/PNG/JPEG 流程。打包器还会遵循源目录中的 `.aixignore`，语法与 `.gitignore` 相同。

### 2. 查看 Agent Definition

无需连接 ADB，即可打印最终生效的 Agent Definition。该命令与 `install` 使用同一个解析器。

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### 3. 安装到设备

打包项目或使用已有 `.aix`，然后通过 ADB 提交到 Rokid Glasses 的 AIUI DEVELOP 公开入口。目录输入复用 `.aix/agent-id`，`.aix` 文件使用包内 `VERSION`。已有 `<PROJECT>/agent.json` 可以覆盖生成的 Definition 元数据，但 `agentId` 始终按稳定规则生成。

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

只有一台在线设备时会自动选择；有多台设备时会显示交互式选择器；非交互环境请使用 `--serial`。

### 4. 设备与启动

```bash
aix device
aix device set-dev
aix device unset-dev
aix launch-page ./my-agent pages/index/index --card
aix launch-widget ./my-agent widgets/order/index --position 2
aix widget-layout show
aix widget-layout --clear
```

`aix device` 还会列出设备包目录中实际存在的 `.aix` Agent，并在可用时补充 manifest 元数据。

设备操作在执行期间显示 spinner；中间阶段完成后只保留简短的成功状态，详细信息只附在最终结果下。

### 5. 查看包内容

```bash
aix list <AIX_FILE>
aix ls <AIX_FILE>
```

列出 `.aix` 中的文件及大小信息。

### 6. 优化已有包

使用与 Web/WASM 包相同的跨平台引擎优化已有归档中的 JSON、PNG 和 JPEG：

```bash
aix optimize input.aix -o output.aix --level 2
```

`aix pack` 默认压缩 JS/TS；`aix pack --optimize` 额外处理 JSON/PNG/JPEG；`aix optimize` 只重新处理已有包中的 JSON/PNG/JPEG。

## 开发与调试

无需全局安装，可在仓库根目录直接运行：

```bash
cargo run -p aiui-aix-cli -- pack ./my-agent -o test.aix
```

## 许可证

MIT
