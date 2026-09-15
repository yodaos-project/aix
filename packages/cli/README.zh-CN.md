# @yodaos-pkg/aix-cli

[English](README.md)

用于打包、校验、查看和调试 **AIX**（AI eXecutable）Agent 包的命令行工具。

```bash
npm install -g @yodaos-pkg/aix-cli
```

安装后可在终端中使用 `aix`。

## 命令

### `aix pack <INPUT_DIR>`

将目录打包为 `.aix`，自动生成 UUID v4 格式的 `VERSION`，校验 JSON，并将非 UTF-8 的 `.json`、`.js` 和 `.ink` 转换为 UTF-8。

JS/TS 默认压缩，与 `--optimize` 无关。`--optimize` 额外启用 JSON 和 PNG/JPEG 优化。

```bash
aix pack ./my-agent
aix pack ./my-agent -o my-app.aix
aix pack ./my-agent --optimize
aix pack ./my-agent -O --opt-level 3
aix pack ./my-agent --engine '^0.14.0'
aix pack ./my-agent --log-time
```

`--engine` 缺省时依次使用 `app.json.engine` 和 `*`，最终范围写入 `META-INF/aix/manifest.json`。打包器遵循 `.aixignore`。

脚本压缩只修改包内内容，不修改源文件；路径和扩展名保持不变。压缩失败会终止并报告具体文件。

`--log-time` 会给打包日志添加本地时间，便于定位扫描、打包或归档收尾阶段的耗时。

### `aix show <INPUT>`

无需 ADB，打印与 `install` 实际上传内容一致的 Agent Definition JSON。

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### `aix install <INPUT>`

将项目或已有 `.aix` 通过 ADB 提交到 Rokid Glasses 的 AIUI DEVELOP 公开入口。目录复用 `.aix/agent-id`；归档使用 `VERSION`；已有 `agent.json` 可覆盖元数据，但不能覆盖规则生成的 `agentId`。

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

单台在线设备自动选择；多台设备使用交互式选择器；非交互环境请传 `--serial`。命令会校验 Apply 结果和手机上传状态，但手机确认不代表云端已经完成索引。

### 设备与启动命令

```bash
aix device
aix device set-dev
aix device unset-dev
aix launch-page ./my-agent pages/index/index --card
aix launch-widget ./my-agent widgets/order/index --position 2
aix widget-layout show
aix widget-layout --clear
```

`aix device` 会显示开发者模式、Widget 布局以及设备包目录中实际安装的 `.aix` Agents。

`launch-widget` 从设备读取当前布局，并从 `app.json.widgets[].family` 推导尺寸。可共存时保留现有动态 Widget；指定位置冲突时自动替换相交项；没有空位时清理旧的动态 placements，但不会自动删除常驻 Widget。

设备操作期间显示 spinner。中间阶段结束后只固化一行成功状态，只有最终阶段保留 Agent、路径、设备等详情。

### `aix list <AIX_FILE>`

列出包内文件及大小，别名为 `aix ls`。

```bash
aix list bundle.aix
```

包元数据（例如 engine 范围）来自 `META-INF/aix/manifest.json`。

### `aix optimize <AIX_FILE> -o <OUTPUT>`

优化已有包中的 JSON、PNG 和 JPEG：

```bash
aix optimize input.aix -o output.aix --level 2
```

### `aix preview <INPUT>`

使用 `@yodaos-pkg/ink` 浏览器 SDK 预览 `.aix` 或源码目录：

```bash
aix preview bundle.aix
aix preview ./my-agent
aix preview bundle.aix --launch
```

默认启动本地 HTTP 服务，只打印 URL，不自动打开浏览器。默认目标为 `blank`（`480x352`）；使用 `--launch-target current` 可按 `448x150` 打开 current 目标。

导出静态 HTML：

```bash
aix preview bundle.aix --html-out ./artifacts/preview.html
```

使用 `--html-out` 时不会启动服务器或浏览器，且不能同时使用 `--launch`。

开发模式通过 WebSocket 监听 `.aix` 或目录变化，并重建 `InkView`：

```bash
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

预览运行时来自 `aix runtime select` 选择的版本；未选择时使用当前 `AIX_NPM_REGISTRY` 的 `latest`。

### Runtime 命令

```bash
aix runtime versions
aix runtime current
aix runtime select
```

- `versions`：列出稳定版本和当前选择。
- `current`：只输出当前默认版本，适合脚本调用。
- `select`：交互选择稳定版本并保存到 `~/.aix/runtime.json`。

通过环境变量选择 registry：

```bash
AIX_NPM_REGISTRY=npm aix runtime versions
AIX_NPM_REGISTRY=npmmirror aix runtime versions
```

## 开发

```bash
npm install
npm run build
```

构建会将 Rust 引擎编译为 Node.js WASM，再打包 TypeScript CLI。命令注册使用 `commander`，交互选择使用 `@inquirer/prompts`；打包、优化和读取逻辑与 Rust/Web 表面共享。

## 许可证

MIT
