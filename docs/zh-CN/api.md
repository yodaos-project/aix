# @yodaos-pkg/aix

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

### AIX.From(data)

~~~typescript
static From(data: Uint8Array | File): Promise<AIX>
~~~

从完整 AIX 归档创建实例。ZIP、路径或规范化条目无效时 Promise reject。

### AIX.pack(files, options?)

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

打包已规范化的内存条目。输入必须包含 app.json，不得包含 VERSION 或 META-INF/aix 保留路径。

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
supportsEngine(currentVersion: string): boolean
getTitle(): string | undefined
getPages(): PageInfo[]
getWidgets(locale?: string): WidgetInfo[]
getTools(): Tool[]
~~~

list 返回规范化名称、压缩大小和未压缩大小；readFile 返回已校验的未压缩字节；getVersion 读取 VERSION；getTitle 读取应用标题；supportsEngine 检查 Manifest 的 semver 范围；getPages 解析 Ink 或多文件页面；getWidgets 校验 Widget 并应用 locale overlay；getTools 派生 OpenAI 兼容函数记录。

## 类型参考

~~~typescript
interface AixInputFile { path: string; data: Uint8Array }
interface PackOptions { buildId?: string; engine?: string; optimize?: false | OptimizeOptions }
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
interface PageInfo { name: string; title?: string; data_schema: unknown }
interface WidgetInfo { path: string; family: string; placement: 'persistent' | 'overlay'; displayName?: string | null; description?: string | null }
interface Tool { type: string; function: { name: string; description?: string; parameters: unknown } }
~~~

AixEntry 描述归档条目；PageInfo 描述页面和输入 Schema；WidgetInfo 描述 Widget；Tool 是从页面派生的 OpenAI 风格工具。字段语义见格式规范。

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
