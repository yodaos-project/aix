# @yodaos-pkg/aix-cli

用于打包、检查、安装和预览 **AIX**（AI eXecutable）包的命令行工具。

## 目录

- [安装](#安装)
- [包管理命令](#包管理命令)
- [设备命令](#设备命令)
- [预览命令](#预览命令)
- [开发](#开发)
- [许可证](#许可证)

## 安装

```bash
npm install -g @yodaos-pkg/aix-cli
```

## 包管理命令

### `aix pack <INPUT_DIR>`

将目录打包为 `.aix`。打包时会校验 JSON，将支持的文本转换为 UTF-8，生成 UUID v4 `VERSION`，并写入 `META-INF/aix/manifest.json`。

```bash
aix pack ./my-agent
aix pack ./my-agent -o my-app.aix
aix pack ./my-agent --engine '^0.14.0'
aix pack ./my-agent --log-time
```

JS/TS 默认压缩但不会修改源文件。传入 `--optimize` 会进一步优化 JSON、PNG 和 JPEG：

```bash
aix pack ./my-agent --optimize
aix pack ./my-agent -O --opt-level 3
```

未传 `--engine` 时优先使用 `app.json.engine`，否则使用 `*`。 `.aixignore` 遵循 `.gitignore` 语法；`--log-time` 为日志添加时间戳。

### `aix show <INPUT>`

在不使用 ADB 的情况下输出生效的 Agent Definition JSON。 `show` 与 `install` 使用同一个解析器。

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### `aix list <AIX_FILE>`

列出归档条目及大小；`aix ls` 是别名。

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

所有设备命令接受 `-s, --serial <serial>`。连接多个设备时打开交互式选择器；只有一个在线且已授权的 ADB 设备时会自动选择。非交互 Shell 必须传入 `--serial`。

活动阶段显示 spinner；已完成的中间阶段折叠为一行勾选结果，只有最新结果保留详细信息。

### `aix device [ACTION]`

```bash
aix device
aix device set-dev
aix device unset-dev
```

只读形式显示开发者模式、Widget 就绪状态与布局，以及已安装的 `.aix` Agent。更改开发者模式会重新加载全部 Widget，之后必须再次启动动态 Widget。

### `aix install <INPUT>`

打包项目或接收 `.aix`，然后通过 ADB 提交到 AIUI DEVELOP。

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

目录使用 `.aix/agent-id`，产物使用 `VERSION`。 `agent.json` 可以覆盖生成的元数据，但不能覆盖 `agentId`。Apply 和手机上传结果会被校验；手机确认不代表云端已完成索引。

### `aix launch-page <INPUT> [PATH]`

```bash
aix launch-page ./my-agent
aix launch-page ./my-agent pages/index/index
aix launch-page ./my-agent pages/index/index --card
aix launch-page ./my-agent pages/index/index --params '{"id":"123"}'
```

文件参数使用 `--params-file <FILE>`。该命令打开已安装的 Agent，不负责安装。

### `aix launch-widget <INPUT> <PATH>`

```bash
aix launch-widget ./my-agent widgets/order/index
aix launch-widget ./my-agent widgets/order/index --position 2
```

尺寸来自 `app.json.widgets[].family`，并保留兼容的 placement。位置冲突时替换重叠的 overlay Widget；没有空间时清理旧的 overlay placement，persistent Widget 永远不会被移除。

必要时命令执行 `prepare -> push widget-config.json -> widget-apply -> open`。Runtime 缺少 placement 时会重新应用一次布局并重试 Open。

### `aix widget-layout [show]`

```bash
aix widget-layout
aix widget-layout show
aix widget-layout --clear
aix widget-layout --clear --yes
```

`show` 为只读操作。 `clear` 保留网格、清空两个模块数组；除非传入 `--yes`，否则会要求确认。

## 预览命令

### `aix preview <INPUT>`

使用 `@yodaos-pkg/ink` 预览产物或源码目录：

```bash
aix preview bundle.aix
aix preview ./my-agent
aix preview bundle.aix --launch
aix preview bundle.aix --launch --launch-target current
```

默认启动本地服务器并打印 URL。 `blank` viewport 为 `480x352`，`current` 使用 `448x150`。

不启动服务器时导出静态 HTML：

```bash
aix preview bundle.aix --html-out ./artifacts/preview.html
```

`--html-out` 不能与 `--launch` 同时使用。需要 WebSocket 热重载时：

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

- `versions` 列出稳定版本和当前生效选择；
- `current` 只输出解析后的版本；
- `select` 将交互式选择保存到 `~/.aix/runtime.json`。

使用 `AIX_NPM_REGISTRY=npm`（默认）或 `AIX_NPM_REGISTRY=npmmirror` 选择 Registry。

## 开发

```bash
npm install
npm run build
node dist/cli.js --help
```

构建过程把 Rust 引擎编译为 Node.js WASM，并打包 TypeScript CLI。打包、优化和读取逻辑与 Web 表面共享。

## 许可证

MIT

