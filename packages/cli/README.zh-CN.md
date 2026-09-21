# @yodaos-pkg/aix-cli

[English](README.md)

用于打包、查看、安装和预览 **AIX**（AI eXecutable）Agent 包的命令行工具。

## 目录

- [安装](#安装)
- [包管理命令](#包管理命令)
  - [`aix pack`](#aix-pack-input_dir)
  - [`aix show`](#aix-show-input)
  - [`aix list`](#aix-list-aix_file)
  - [`aix optimize`](#aix-optimize-aix_file)
- [设备命令](#设备命令)
  - [`aix device`](#aix-device-action)
  - [`aix install`](#aix-install-input)
  - [`aix launch-page`](#aix-launch-page-input-path)
  - [`aix launch-widget`](#aix-launch-widget-input-path)
  - [`aix widget-layout`](#aix-widget-layout-show)
- [预览命令](#预览命令)
  - [`aix preview`](#aix-preview-input)
  - [`aix runtime`](#aix-runtime)
- [开发](#开发)
- [许可证](#许可证)

## 安装

```bash
npm install -g @yodaos-pkg/aix-cli
```

## 包管理命令

### `aix pack <INPUT_DIR>`

将目录打包为 `.aix`，校验 JSON，将支持的文本转换为 UTF-8，生成 UUID v4 格式的 `VERSION`，并写入 `META-INF/aix/manifest.json`。

```bash
aix pack ./my-agent
aix pack ./my-agent -o my-app.aix
aix pack ./my-agent --engine '^0.14.0'
aix pack ./my-agent --log-time
```

JS/TS 默认压缩且不修改源文件。`--optimize` 额外优化 JSON、PNG 和 JPEG：

```bash
aix pack ./my-agent --optimize
aix pack ./my-agent -O --opt-level 3
```

未传 `--engine` 时依次使用 `app.json.engine` 和 `*`。`.aixignore` 使用 `.gitignore` 语法；`--log-time` 为打包日志添加时间。

### `aix show <INPUT>`

无需 ADB，打印最终 Agent Definition JSON。`show` 与 `install` 使用同一个解析器。

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### `aix list <AIX_FILE>`

列出归档条目和大小，`aix ls` 是别名。

```bash
aix list bundle.aix
aix ls bundle.aix
```

### `aix optimize <AIX_FILE>`

优化已有包中的 JSON、PNG 和 JPEG：

```bash
aix optimize input.aix -o output.aix --level 2
```

## 设备命令

所有设备命令均支持 `-s, --serial <serial>`。单台在线授权设备自动选择；多台设备显示交互选择器；非交互环境必须传 `--serial`。

当前阶段会显示 spinner。中间阶段完成后收敛为一行 `✔`，只有最新结果保留详细信息。

### `aix device [ACTION]`

```bash
aix device
aix device set-dev
aix device unset-dev
```

只读形式显示 Developer Mode、Widget 就绪状态和布局，以及已安装的 `.aix` Agents。切换 Developer Mode 会重载所有 Widget，动态 Widget 需要重新启动。

### `aix install <INPUT>`

打包项目或使用已有 `.aix`，然后通过 ADB 提交到 AIUI DEVELOP。

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

目录复用 `.aix/agent-id`，归档使用 `VERSION`。`agent.json` 可覆盖生成的元数据，但不能覆盖 `agentId`。命令校验 Apply 和手机上传结果；手机确认不代表云端完成索引。

### `aix launch-page <INPUT> [PATH]`

```bash
aix launch-page ./my-agent
aix launch-page ./my-agent pages/index/index
aix launch-page ./my-agent pages/index/index --card
aix launch-page ./my-agent pages/index/index --params '{"id":"123"}'
```

文件参数使用 `--params-file <FILE>`。该命令只打开已安装 Agent，不负责安装。

### `aix launch-widget <INPUT> <PATH>`

```bash
aix launch-widget ./my-agent widgets/order/index
aix launch-widget ./my-agent widgets/order/index --position 2
```

尺寸来自 `app.json.widgets[].family`。兼容的 placements 会保留；位置冲突时替换相交的可叠加 Widget；没有空间时清理旧的可叠加 placements。常驻 Widget 永远不会被自动删除。

必要时执行 `prepare -> push widget-config.json -> widget-apply -> open`。运行时 placement 丢失时会重新提交布局并重试一次 Open。

### `aix widget-layout [show]`

```bash
aix widget-layout
aix widget-layout show
aix widget-layout --clear
aix widget-layout --clear --yes
```

Show 只读。Clear 保留网格、清空两个模块数组，并在未传 `--yes` 时要求确认。

## 预览命令

### `aix preview <INPUT>`

使用 `@yodaos-pkg/ink` 预览归档或源码目录：

```bash
aix preview bundle.aix
aix preview ./my-agent
aix preview bundle.aix --launch
aix preview bundle.aix --launch --launch-target current
```

默认启动本地服务器并打印 URL。`blank` 视口为 `480x352`，`current` 为 `448x150`。

导出静态 HTML，不启动服务器：

```bash
aix preview bundle.aix --html-out ./artifacts/preview.html
```

`--html-out` 不能与 `--launch` 同时使用。实时 WebSocket 重载使用：

```bash
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

### `aix runtime`

```bash
aix runtime versions
aix runtime current
aix runtime select
```

- `versions`：列出稳定版本和当前选择；
- `current`：只打印解析后的版本；
- `select`：交互选择并保存到 `~/.aix/runtime.json`。

通过 `AIX_NPM_REGISTRY=npm`（默认）或 `AIX_NPM_REGISTRY=npmmirror` 选择 registry。

## 开发

```bash
npm install
npm run build
node dist/cli.js --help
```

构建会将 Rust 引擎编译为 Node.js WASM，再打包 TypeScript CLI。打包、优化和读取逻辑与 Rust/Web 表面共享。

## 许可证

MIT
