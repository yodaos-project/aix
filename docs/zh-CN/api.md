# @yodaos-pkg/aix

读取时只加载独立 reader WASM，打包、优化和源码检查首次调用时再加载完整工具模块。
现有 API 和直接使用 WASM 的路径保持可用。自行部署时需包含生成分发目录中的
`pkg-reader/` 和 `pkg/`。构建选项、兼容方案、取舍与测量结果见
[WASM 加载与测量](/wasm)。

@yodaos-pkg/aix 是面向浏览器的 WebAssembly 与 TypeScript 包，用于读取、创建、优化和检查 AIX 产物。
高级 SDK 面向支持 ES 模块、fetch 和 WebAssembly 的现代浏览器及 Web Worker；npm CLI 使用独立的 Node.js WASM 构建。

## 安装

~~~bash
npm install @yodaos-pkg/aix
~~~

~~~typescript
import { AIX } from '@yodaos-pkg/aix';
const response = await fetch('/agents/example.aix');
const aix = await AIX.From(new Uint8Array(await response.arrayBuffer()));
console.log(aix.getTitle(), aix.getPages(), aix.getTools());
~~~

AIX.From 会初始化 WASM Runtime，接受完整归档字节或浏览器 File 对象。

## API 参考

本文对应 v0.11.0 的 TypeScript 公开接口。7 个静态方法均为异步，10 个实例方法均为同步。
源码检查使用 `AIX.checkFromSource()` 或 `AIX.checkFromFiles()`；没有 `AIX.check()` 或实例 `check()`。

### AIX.From(data, limits?)

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits): Promise<AIX>
~~~

从完整 AIX 归档创建实例。ZIP、路径或规范化条目无效，或超出预算时 Promise reject。

~~~typescript
const aix = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
~~~

这些数值是调用方选择的预算示例，等于上限时允许读取，未知预算字段会被拒绝。所有中央目录条目都计入预算，
包括目录和签名元数据。构造时在解压前检查元数据，`readFile` 在输出时执行大小限制，
并保留 CRC 和大小校验。`maxTotalBytes` 限制整个归档声明的解压大小总和，重复读取
不会消耗该预算。每项必须是非负安全整数；零表示不允许条目或字节。省略的字段、`{}`
以及原有单参数调用均保持无限制的兼容策略。读取不可信包时应设置全部三项。
这些 reader 预算不覆盖输入归档缓冲区、解析对象、调用方保留的副本或 `AIX.optimize`。

Rust 使用 `AixReader::new_with_limits(data, AixReadLimits { ... })`，字段为
`Option<u64>` 类型的 `max_entries`、`max_entry_bytes` 和 `max_total_bytes`。
`AixReadLimits::default()` 和 `AixReader::new(data)` 保持无限制。
直接调用 WASM 时使用 `AixReaderWasm.with_limits(data, limits)`，字段名与 TypeScript
SDK 一样使用 camelCase；`new AixReaderWasm(data)` 保持原有策略。

### AIX.checkFromSource(files, options?)

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

检查内存中的源码树，不生成归档、输出文件或运行项目代码。路径相对于应用根目录，
`app.json` 必须位于根目录。收集时规范化路径，应用根目录及嵌套 `.aixignore`，
排除规则文件和任意深度的 `.aix/` 缓存。

检查范围包含 JSON/文本有效性、App/Page/Widget 结构、组件引用与循环依赖、JS/TS 语法
（含 `.mts`、`.cts` 和 Ink `lang="ts"` 脚本）、可静态识别的权限使用及 Page `schema.data`。
Schema 按 `$schema` 指定的草案校验，默认 2020-12；支持本地引用，不获取外部资源。
这里只校验 Schema 声明，不校验运行时数据。TypeScript 类型语义、模块解析、编译选项、
动态 API 调用和运行时行为不属于检查范围。

~~~typescript
import { AIX, type AixInputFile, type CheckReport } from '@yodaos-pkg/aix';

const encode = (text: string) => new TextEncoder().encode(text);
const files: AixInputFile[] = [
  { path: 'app.json', data: encode('{"pages":[]}') },
  { path: 'main.ts', data: encode('const count: number = ;') },
];
const report: CheckReport = await AIX.checkFromSource(files, {
  onProgress(event) {
    if (event.type === 'check_finished') {
      console.log(event.check.id, event.check.status, event.check.detail);
    }
  },
});
for (const diagnostic of report.diagnostics) {
  console.log(diagnostic.path, diagnostic.line, diagnostic.column,
    diagnostic.severity, diagnostic.code, diagnostic.message);
}
const hasErrors = report.diagnostics.some(item => item.severity === 'error');
~~~

源码错误和警告通过 `CheckReport.diagnostics` 返回，不会导致 Promise reject。
调用方应判断是否包含 `severity === 'error'`，决定是否继续。
无效路径、无效忽略规则等收集失败、WASM 加载失败，以及进度回调抛错会导致 reject。
`CheckReport` 没有成功标志或检查清单数组；分类检查结果通过 `check_finished` 事件提供。

### AIX.checkFromFiles(files, options?)

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

检查浏览器 `File` 数组，优先使用 `webkitRelativePath`，否则使用 `name`。
当所有相对路径共享所选目录前缀时，会移除这层外部目录，再调用 `checkFromSource`。
读取文件失败会导致 Promise reject。目录选择示例：

~~~html
<input id="source" type="file" webkitdirectory multiple>
~~~

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector<HTMLInputElement>('#source')!;
input.addEventListener('change', async () => {
  const report = await AIX.checkFromFiles(Array.from(input.files ?? []));
  console.log(report.diagnostics);
});
~~~

### 检查进度与浏览器响应

`CheckOptions.onProgress` 依次收到 `loading_checker`、`checking_sources`
（收集前的输入文件数）、5 个 `check_finished`，以及成功完成时的 `collecting_diagnostics`。
5 个分类 ID 为 `json`、`structure`、`syntax`、`permissions`、`schemas`。
结果包含状态、错误/警告数量及可选 `detail`；无适用工作或被无效输入阻断的检查可能为 `skipped`。

这些是阶段事件，不代表完成百分比。浏览器在初始化后同步执行检查；需要保持界面响应时，
应放入 Web Worker。npm CLI 使用同一个检查引擎，在 Node.js Worker 中执行；参见
[`aix check`](/cli?lang=zh-CN#aix-check-input-dir)。

### AIX.pack(files, options?)

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

打包已规范化的内存条目。输入必须包含 `app.json`，不得包含 `VERSION` 或 `META-INF/aix/` 保留路径。
这个底层入口不收集源码树，也不应用 `.aixignore`。

`buildId` 默认生成 UUID v4 并写入 `VERSION`；`engine` 依次采用显式选项、`app.json.engine`、`*`。
所有 `.json` 都会校验，并将支持的文本规范化为 UTF-8。独立 `.js` / `.ts` 即使省略
`optimize` 或设为 `false`，仍会压缩；这不会把 TypeScript 转译为 JavaScript。

省略 `optimize` 或设为 `false` 时，不启用可选的 JSON/图片优化；传 `{}` 时使用
`level: 2`、`json: true`、`png: true`、`jpeg: true`。打包不会自动执行完整源码检查。
返回的 `warnings` 是非致命打包诊断，完整静态报告需调用源码检查 API。

跨构建复用相同的 `options.agentId` 会写入 Manifest 的 `agent_id`；它是规范化的
项目 ID；SDK 不会创建文件系统身份文件或自动生成该 ID。它必须非空，只包含小写 ASCII
字母、数字、`_`、`-`，且不能以 `-` 开头或结尾。它与 `buildId` / `VERSION` 独立，`optimize` 会保留它。`getAgentId()` 返回
该 ID，旧包缺少身份时返回 `undefined`，Manifest 身份无效时抛错。直接调用 WASM
打包函数时将 ID 作为末尾可选参数，读取时调用 `get_agent_id()`；Rust 使用
`PackOptions.agent_id` 和 `AixReader::get_agent_id()`。

### AIX.packFromSource(files, options?)

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

打包源码树，规范化路径，应用嵌套 `.aixignore`，并排除规则文件和任意深度的 `.aix/` 缓存。
规范化后路径重复或收集规则无效会导致 Promise reject；已完成收集时可使用 `pack`。
`onProgress` 仅用于本方法及 `packFromFiles`，不用于 `pack` 或 `optimize`；回调抛错会使调用失败。

### AIX.packFromFiles(files, options?)

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

使用 `webkitRelativePath`（不存在时使用 `name`）将浏览器 `File` 转换为源码条目，
然后调用 `packFromSource`。

与 `checkFromFiles` 不同，本方法保留 `webkitRelativePath` 中的外部目录。
使用 `webkitdirectory` 选择目录后，应自行去掉这层前缀，使 `app.json` 位于根目录，
再调用 `packFromSource`：

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector<HTMLInputElement>('#source')!;
const files = await Promise.all(Array.from(input.files ?? []).map(async file => ({
  path: file.webkitRelativePath.split('/').slice(1).join('/'),
  data: new Uint8Array(await file.arrayBuffer()),
})));
const packed = await AIX.packFromSource(files, { agentId: 'my-agent' });
~~~


### AIX.optimize(data, options?)

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

写入已有包的优化副本，保留 `VERSION` 和 Manifest `agent_id`，移除原签名。
省略选项时使用上述优化默认值。报告中的字节总数是未压缩条目数据大小，不是 ZIP 下载大小。

### 实例方法

~~~typescript
list(): AixEntry[]
readFile(name: string): Uint8Array
getVersion(): string | undefined
getAgentId(): string | undefined
supportsEngine(currentVersion: string): boolean
getTitle(): string | undefined
getPages(): PageInfo[]
resolveMetadata(): MetadataResolution
getWidgets(locale?: string): WidgetInfo[]
getTools(): Tool[]
~~~

| 方法 | 行为 |
| --- | --- |
| `list()` | 返回规范化条目名、压缩及未压缩大小。 |
| `readFile(name)` | 按准确条目名读取并校验 CRC/大小；条目缺失、损坏或超出预算时抛错。 |
| `getVersion()` | 读取 `VERSION`；缺失、不可读或非 UTF-8 时返回 `undefined`。 |
| `getAgentId()` | 读取 Manifest `agent_id`；缺失身份返回 `undefined`，Manifest 或身份无效时抛错。 |
| `supportsEngine(version)` | 匹配 Manifest semver 范围；Manifest/范围缺失或无效、传入版本无效时抛错。 |
| `getTitle()` | 读取 `app.json.window.navigationBarTitleText`；缺失或配置无效时返回 `undefined`。 |
| `getPages()` | 按下述宽容策略解析页面元数据。 |
| `resolveMetadata()` | 返回成功解析的页面/工具及资源诊断。 |
| `getWidgets(locale?)` | 校验入口，并按需叠加 `app.<locale>.json` 的 `displayName`/`description`；无效配置/语言文件或入口缺失时抛错。 |
| `getTools()` | 从宽容解析的页面列表派生 OpenAI 兼容函数记录。 |

同步方法可能在读取包资源时抛错。`AIX.From()` 校验归档结构，但不会立即读取全部条目，
也不会验证发布者的加密签名。签名验证由 Rust reader 提供，不属于公开 TypeScript 方法。
页面、布局、Widget 和工具语义参见[规范](/spec?lang=zh-CN)。

### 元数据解析与诊断

`getPages()` 和 `getTools()` 保留宽容行为：页面列表无效时返回空数组，缺失资源或无效
页面元数据仍可能生成使用默认值的页面和工具。`app.json` 的 Page、Widget 和标题字段
分别解析，因此无效 Widget 字段不会隐藏有效页面。`getWidgets()` 仍会在 Widget 配置
无效或入口缺失时抛错。

需要区分成功解析与默认结果时，使用 `resolveMetadata()`：

~~~typescript
const result = aix.resolveMetadata();
for (const diagnostic of result.diagnostics) {
  console.error(diagnostic.path, diagnostic.message);
}
// 只有解析成功的页面才会生成可调用工具。
const tools = result.tools;
// 如果要求全部页面解析成功，可在 diagnostics 非空时拒绝使用该结果。
~~~

该同步 API 返回 `pages`、`tools` 和 `diagnostics`，解析失败通过诊断返回。
有效的 `pages: []` 没有诊断；`app.json.pages` 缺失、无法读取或无效时，诊断路径为
`app.json`。每个失败页面返回第一个资源或元数据错误，并从页面和工具列表中排除，
其余页面保留声明顺序。工具 target 使用与 `getTools()` 相同的规则，作用于成功解析的页面列表。

页面需要可读取的 `.ink`，或传统页面的 `.json` 和 `.wxml`。已有资源必须通过 ZIP
CRC/大小校验和 UTF-8 解码；页面 JSON 和完整的 Ink `<script def>` 必须能反序列化。
损坏的 `.ink` 不会回退到多文件页面。定义、schema 和样式允许省略；`.wcss` 优先于
`.wxss`。Widget 和标题字段不属于该报告。此 API 不替代源码检查中的Ink 结构、
组件引用和 JSON Schema 校验，也不验证运行时行为。打包校验保持不变。

Rust 使用 `AixReader::resolve_metadata() -> MetadataResolution`；直接调用 WASM 时使用
`reader.resolve_metadata()`，报告结构一致。

## 类型参考

~~~typescript
interface AixInputFile {
    path: string;
    data: Uint8Array;
}

interface AixReadLimits {
    maxEntries?: number;
    maxEntryBytes?: number;
    maxTotalBytes?: number;
}

interface PackOptions {
    buildId?: string;
    agentId?: string;
    engine?: string;
    optimize?: false | OptimizeOptions;
}

interface OptimizeOptions {
    level?: 1 | 2 | 3;
    json?: boolean;
    png?: boolean;
    jpeg?: boolean;
}

interface PackFromSourceOptions extends PackOptions {
    onProgress?: (event: PackProgressEvent) => void;
}

interface PackResult {
    data: Uint8Array;
    report: OptimizeReport;
    warnings: string[];
}

interface OptimizeReport {
    files: FileOptimizeReport[];
    original_size: number;
    output_size: number;
    saved_bytes: number;
}

interface FileOptimizeReport {
    path: string;
    status: 'optimized' | 'unchanged' | 'skipped';
    original_size: number;
    output_size: number;
    saved_bytes: number;
    converted_to_utf8: boolean;
}
~~~

### PackProgressEvent

~~~typescript
type PackProgressEvent = {
    type: 'transferring_files_to_wasm';
} | {
    type: 'collecting_source_inputs';
} | {
    type: 'resolving_engine';
} | {
    type: 'preparing_files';
} | {
    type: 'file_finished';
    report: FileOptimizeReport;
} | {
    type: 'finalizing_archive';
};
~~~

打包回调依次报告传输、源码收集、Engine 解析、文件准备、单文件完成和归档最终化。
`file_finished` 携带该条目的 `FileOptimizeReport`，也包含生成的 `VERSION` 条目。
这些是同步阶段通知，不是完成百分比。

### 源码检查类型

~~~typescript
interface CheckDiagnostic {
    code: string;
    severity: 'error' | 'warning';
    path: string;
    line: number;
    column: number;
    message: string;
}

interface CheckReport {
    diagnostics: CheckDiagnostic[];
}

interface CheckOptions {
    onProgress?: (event: CheckProgressEvent) => void;
}

interface CheckResult {
    id: 'json' | 'structure' | 'syntax' | 'permissions' | 'schemas';
    label: string;
    status: 'passed' | 'warning' | 'failed' | 'skipped';
    errors: number;
    warnings: number;
    detail?: string;
}

type CheckProgressEvent = {
    type: 'loading_checker';
} | {
    type: 'checking_sources';
    fileCount: number;
} | {
    type: 'check_finished';
    check: CheckResult;
} | {
    type: 'collecting_diagnostics';
};
~~~

### 包与派生模型

~~~typescript
interface AixEntry {
    name: string;
    size: number;
    compressed_size: number;
}

type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;

interface JsonObject {
    [key: string]: JsonValue;
}

type JsonSchema = JsonValue;

interface PageConstraint {
    width: number;
    height: number;
}

interface PageInfo {
    name: string;
    title: string | null;
    description: string | null;
    data_schema: JsonSchema;
    size: PageConstraint;
}

interface MetadataDiagnostic {
    path: string;
    message: string;
}

interface MetadataResolution {
    pages: PageInfo[];
    tools: Tool[];
    diagnostics: MetadataDiagnostic[];
}

type WidgetPlacement = 'persistent' | 'overlay';

interface WidgetInfo {
    path: string;
    family: string;
    placement: WidgetPlacement;
    displayName: string | null;
    description: string | null;
}

type ToolTarget = '_current' | '_blank';

interface FunctionDefinition {
    name: string;
    description: string | null;
    parameters: JsonSchema;
}

interface Tool {
    type: string;
    target: ToolTarget;
    layout: PageConstraint;
    function: FunctionDefinition;
}
~~~

Page 的 `size` 和 Tool 的 `layout` 均包含数值宽高约束。工具当前输出 `type: "function"`，
目标值为 `_current` / `_blank`；无下划线的名称仅是反序列化输入别名。
`JsonSchema` 保留任意 JSON，包括布尔 Schema 和未经校验的旧版数据；Schema 校验使用源码检查 API，
访问对象属性前需要先收窄 `JsonValue` 类型。

以上元数据字段均必需。缺失字符串和嵌套 JSON 空值在原始 WASM、SDK getter 和
`resolveMetadata()` 中统一保留为 `null`，与 Rust 原生 JSON 一致。早期 WASM 构建会将这些值
返回为 `undefined`，迁移时需将元数据的显式 `=== undefined` 判断改为 `=== null`。
独立的 `getTitle()` 和 `getVersion()` 仍返回 `string | undefined`。

## 浏览器示例

~~~typescript
const input = document.querySelector<HTMLInputElement>('#package')!;
input.addEventListener('change', async () => {
  const file = input.files?.[0];
  if (!file) return;
  const aix = await AIX.From(file);
  console.log(aix.getVersion());
  console.table(aix.list());
  console.log(aix.getPages(), aix.getTools());
});
~~~

示例从文件输入读取 AIX，并输出版本、条目、页面和工具。

## Play

官方浏览器实验室位于 /play?lang=zh-CN。它展示标题、版本、页面、派生工具、归档条目和原始内容，并支持上传包及构建目录。

## 从源码构建

~~~bash
cd crates/aix-web
npm install
npm run build
~~~

发行目录为 crates/aix-web/dist。应用应使用已发布的 npm 包，不应依赖生成目录内部实现。

## 许可证

MIT
