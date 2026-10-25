# @yodaos-pkg/aix

读取时只加载独立 reader WASM，打包、优化和源码检查首次调用时再加载完整工具模块。
现有 API 和直接使用 WASM 的路径保持可用。自行部署时需包含生成分发目录中的
`pkg-reader/` 和 `pkg/`。构建选项、兼容方案、取舍与测量结果见
[WASM 加载与测量](/wasm)。

@yodaos-pkg/aix 是面向浏览器的 WebAssembly 与 TypeScript 包，用于读取、创建、优化和检查 AIX 产物。

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

## API Reference

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

这些数值是调用方选择的预算示例，等于上限时允许读取。所有中央目录条目都计入预算，
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

### AIX.pack(files, options?)

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

打包已规范化的内存条目。输入必须包含 app.json，不得包含 VERSION 或 META-INF/aix 保留路径。

跨构建复用相同的 `options.agentId` 会写入 Manifest 的 `agent_id`；它是规范化的
项目 ID，与 `buildId` / `VERSION` 独立，`optimize` 会保留它。`getAgentId()` 返回
该 ID，旧包缺少身份时返回 `undefined`，Manifest 身份无效时抛错。直接调用 WASM
打包函数时将 ID 作为末尾可选参数，读取时调用 `get_agent_id()`；Rust 使用
`PackOptions.agent_id` 和 `AixReader::get_agent_id()`。

### AIX.packFromSource(files, options?)

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

打包源码树、规范化路径、应用嵌套 .aixignore 并忽略规则文件；步骤已完成时使用 pack。

### AIX.packFromFiles(files, options?)

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

使用 webkitRelativePath 将浏览器 File 转换为源码条目，然后调用 packFromSource。

### AIX.optimize(data, options?)

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

写入已有包的优化副本。优化会改变字节，因此不会保留原签名。

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

list 返回规范化名称、压缩大小和未压缩大小；readFile 返回已校验的未压缩字节；getVersion 读取 VERSION；getTitle 读取应用标题；supportsEngine 检查 Manifest 的 semver 范围；getPages 解析 Ink 或多文件页面；getWidgets 校验 Widget 并应用 locale overlay；getTools 派生 OpenAI 兼容函数记录。

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
`.wxss`。Widget 和标题字段不属于该报告。此 API 不替代源码检查中的完整 Ink 语法、
组件引用和 JSON Schema 校验，也不验证运行时行为。打包校验保持不变。

Rust 使用 `AixReader::resolve_metadata() -> MetadataResolution`；直接调用 WASM 时使用
`reader.resolve_metadata()`，报告结构一致。

## 类型参考

~~~typescript
interface MetadataDiagnostic { path: string; message: string }
interface MetadataResolution {
  pages: PageInfo[];
  tools: Tool[];
  diagnostics: MetadataDiagnostic[];
}

interface AixInputFile { path: string; data: Uint8Array }
interface PackOptions { buildId?: string; agentId?: string; engine?: string; optimize?: false | OptimizeOptions }
interface OptimizeOptions { level?: 1 | 2 | 3; json?: boolean; png?: boolean; jpeg?: boolean }
interface PackFromSourceOptions extends PackOptions { onProgress?: (event: PackProgressEvent) => void }
interface PackResult { data: Uint8Array; report: OptimizeReport; warnings: string[] }
interface OptimizeReport { files: FileOptimizeReport[]; original_size: number; output_size: number; saved_bytes: number }
interface FileOptimizeReport { path: string; status: 'optimized' | 'unchanged' | 'skipped'; original_size: number; output_size: number; saved_bytes: number; converted_to_utf8: boolean }
~~~

### PackProgressEvent

~~~typescript
type PackProgressEvent =
  | { type: 'transferring_files_to_wasm' }
  | { type: 'collecting_source_inputs' }
  | { type: 'resolving_engine' }
  | { type: 'preparing_files' }
  | { type: 'file_finished'; report: FileOptimizeReport }
  | { type: 'finalizing_archive' };
~~~

事件分别表示传输到 WASM、收集输入、解析 Engine、准备文件、完成单文件和归档最终化；顺序只用于进度展示。

### 包与派生模型

~~~typescript
interface AixEntry { name: string; size: number; compressed_size: number }
type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;
interface JsonObject { [key: string]: JsonValue }
type JsonSchema = JsonValue;
interface PageConstraint { width: number; height: number }
type ToolTarget = '_current' | '_blank';
type WidgetPlacement = 'persistent' | 'overlay';
interface PageInfo {
  name: string;
  title: string | null;
  description: string | null;
  data_schema: JsonSchema;
  size: PageConstraint;
}
interface WidgetInfo {
  path: string;
  family: string;
  placement: WidgetPlacement;
  displayName: string | null;
  description: string | null;
}
interface Tool {
  type: string;
  target: ToolTarget;
  layout: PageConstraint;
  function: FunctionDefinition;
}
interface FunctionDefinition {
  name: string;
  description: string | null;
  parameters: JsonSchema;
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

## Package Lab

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
