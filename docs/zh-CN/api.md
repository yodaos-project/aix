---
outline: [2, 4]
---

# AIX Web SDK {#yodaos-pkg-aix}

在网页中使用 `@yodaos-pkg/aix`，可以检查 AI agent 项目、生成 `.aix` 文件，或读取已有的包。AIX 包是一个 ZIP 归档，里面包含 agent 的文件和元数据。

想直接体验，可以打开 [Play](/zh-CN/play)。想在终端操作，可以看 [CLI 指南](/zh-CN/cli)。查询方法签名和类型时，可跳到 [API Reference](#api-reference)。

<div id="_1-安装并导入"></div>

## 快速开始 {#quick-start}

### 安装 SDK {#installation}

在你的前端项目中运行以下命令。

~~~bash
npm install @yodaos-pkg/aix
~~~

### 打包并打开第一个 AIX 文件 {#build-and-open-your-first-package}

把下面的 JavaScript 放进应用的模块脚本。示例会创建一个最小项目，检查源码、打包，再打开生成的包。

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const files = [{
  path: 'app.json',
  data: new TextEncoder().encode(JSON.stringify({
    pages: [],
    window: { navigationBarTitleText: 'My first agent' },
  })),
}];

const report = await AIX.checkFromSource(files);
if (report.diagnostics.some(item => item.severity === 'error')) {
  console.table(report.diagnostics);
  throw new Error('Fix the source errors before packing.');
}

const result = await AIX.packFromSource(files);
const pkg = await AIX.From(result.data);

console.log(pkg.getTitle()); // My first agent
console.table(pkg.list());
~~~

每个源文件有两个字段。`path` 是相对于项目根目录的路径，`data` 是 `Uint8Array` 字节。用 `TextEncoder` 可以把文本转成字节。打包后的字节在 `result.data` 中。用 `AIX.From()` 打开之后，`pkg.getTitle()` 和 `pkg.list()` 等方法会直接返回结果。

<div id="_2-检查项目并下载打包结果"></div>

## 检查并打包项目 {#check-and-build-a-project}

### 检查源文件 {#aix-checkfromsource-files-options}

代码已经拿到项目文件的路径和字节时，调用 `AIX.checkFromSource(files)`。文件列表的根目录应包含 `app.json`。

返回的 `diagnostics` 列出了发现的问题。每一项包含级别（`error` 或 `warning`）、说明、文件路径、行和列。空数组表示本次检查没有发现问题。文件读取、收集、检查器加载或进度回调失败时，Promise 会拒绝。

::: details 检查范围
检查包括 JSON 和文本编码、应用、页面和小组件的结构、组件引用、JavaScript 和 TypeScript 语法、可识别的权限用法，以及页面数据的 Schema 声明。TypeScript 检查支持 `.mts`、`.cts` 和带 `lang="ts"` 的 `.ink` 脚本块。

Schema 检查默认使用 JSON Schema 2020-12，并解析本地引用。类型问题要单独用 TypeScript 检查，运行效果要在 AIUI 中验证。各阶段的通知见[检查进度回调](#check-progress-and-browser-responsiveness)。
:::

### 检查浏览器中选择的文件夹 {#aix-checkfromfiles-files-options}

如果项目由用户提供，可以使用文件夹选择框。

~~~html
<input id="project-folder" type="file" webkitdirectory multiple>
~~~

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector('#project-folder');
input.addEventListener('change', async () => {
  if (!input.files.length) return;
  const report = await AIX.checkFromFiles(Array.from(input.files));
  console.table(report.diagnostics);
});
~~~

请选择包含 `app.json` 的文件夹。`checkFromFiles()` 读取浏览器提供的相对路径，并移除这些文件共有的外层目录。浏览器没有提供相对路径时，SDK 使用文件名。

### 从源码打包 {#aix-packfromsource-files-options}

用 `AIX.packFromSource(files)` 打包项目。SDK 会规范化路径，应用根目录和子目录的 `.aixignore`，并排除规则文件与 `.aix/` 缓存目录。路径无效、规范化后重复或忽略规则无效都会使打包失败。

打包时会检查生成归档所需的文件。要取得完整的源码检查报告，请像快速开始中的示例一样，先调用 `checkFromSource()`。

| 结果字段 | 含义 |
| --- | --- |
| `data` | 新 `.aix` 文件的字节。 |
| `warnings` | 未阻止打包的提醒。 |
| `report` | 处理前后的文件字节数，按 ZIP 压缩前的大小计算。 |
| `progressiveAssets` | 每个媒体资源的交付方式和留在包里的源文件字节数。 |

要在浏览器中下载结果，可以调用 `downloadPackage(result.data, 'my-agent.aix')`。

~~~javascript
function downloadPackage(data, filename) {
  const url = URL.createObjectURL(new Blob([data], { type: 'application/zip' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
~~~

### 打包浏览器文件 {#aix-packfromfiles-files-options}

如果浏览器文件的路径已经从项目根目录开始，可以调用 `AIX.packFromFiles(files)`。方法优先使用 `webkitRelativePath`，否则使用 `name`。

文件夹选择框通常会返回 `my-agent/app.json` 这样的路径。`packFromFiles()` 会保留外层目录。这时可先去掉目录前缀，再将结果传给 `packFromSource()`。

~~~javascript
// Call after the user selects the project folder.
const files = await Promise.all(Array.from(input.files, async file => ({
  path: file.webkitRelativePath.split('/').slice(1).join('/'),
  data: new Uint8Array(await file.arrayBuffer()),
})));
const result = await AIX.packFromSource(files);
downloadPackage(result.data, 'my-agent.aix');
~~~

### 打包已准备好的文件列表 {#aix-pack-files-options}

如果代码已经筛选好文件并规范化路径，可以用 `AIX.pack(files)` 直接打包。需要 AIX 应用项目的忽略规则时，用 `packFromSource()`。

所有打包方法都要求根目录包含 `app.json`。AIX 会生成 `VERSION` 和 `META-INF/aix/` 下的元数据，这些条目应从输入中排除。

<div id="_4-按需设置打包选项"></div>

### 设置打包选项 {#set-build-options}

将选项作为打包方法的第二个参数。例如，`AIX.packFromSource(files, { agentId: 'my-agent', optimize: {} })` 会设置项目标识并开启优化。

| 选项 | 用途 |
| --- | --- |
| `agentId` | 写入清单的稳定项目标识。同一项目多次打包时复用它。 |
| `buildId` | 本次构建的 `VERSION`，默认生成新的 UUID。 |
| `engine` | 兼容的 AIUI 版本范围。默认使用 `app.json.engine`，再回退到表示任意版本的 `*`。 |
| `optimize` | 传入 `{}` 以级别 2 优化 JSON、PNG 和 JPEG。可将 `level` 设为 1、2 或 3，也可以单独关闭某种格式。 |
| `onProgress` | 接收 `packFromSource()` 和 `packFromFiles()` 的打包阶段通知。 |
| `onProgressiveAsset` | 决定每个[渐进资源](#progressive-assets)的交付方式。 |
| `signal` | 通过 `AbortSignal` 取消打包。 |

`agentId` 必须是非空字符串，只能包含小写 ASCII 字母、数字、`_` 或 `-`，且不能以 `-` 开头或结尾。需要长期保留项目标识时，请自行提供这个值。

省略 `optimize` 或设为 `false` 时，JSON 和图片的可选优化关闭。打包仍会校验 JSON、将支持的文本转为 UTF-8，并压缩独立的 JavaScript 和 TypeScript 文件。TypeScript 文件仍是 TypeScript。

<div id="_3-读取一个已有的-aix-文件"></div>

## 打开已有的包 {#aix-from-data-limits}

`AIX.From()` 接受浏览器 `File` 或包含完整包内容的 `Uint8Array`。方法名中的 **F** 是大写。

### 打开本地文件 {#browser-example}

~~~html
<input id="aix-file" type="file" accept=".aix">
~~~

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const input = document.querySelector('#aix-file');
input.addEventListener('change', async () => {
  const file = input.files[0];
  if (!file) return;

  const pkg = await AIX.From(file);
  console.log(pkg.getTitle(), pkg.getVersion());
  console.table(pkg.list());
});
~~~

<div id="从网址读取"></div>

### 从网址打开包 {#open-a-package-from-a-url}

~~~javascript
const response = await fetch('/agents/example.aix');
if (!response.ok) throw new Error(`Download failed (${response.status})`);

const pkg = await AIX.From(new Uint8Array(await response.arrayBuffer()));
const text = new TextDecoder().decode(pkg.readFile('app.json'));
console.log(JSON.parse(text));
~~~

`pkg.readFile()` 返回一个条目的字节。JSON 或脚本可以解码为文本，图片等二进制文件可以直接使用这些字节。

<div id="打开时自动检查"></div>

### 打开时检查源码 {#aix-check-error}

将第三个参数设为 `true`，就会在打开包时检查源码。只有提醒时可以打开。发现错误时会得到 `AixCheckError`，其中包含完整的 `report` 和 `diagnostics` 数组。

~~~javascript
import { AIX, AixCheckError } from '@yodaos-pkg/aix';

async function openCheckedPackage(file) {
  try {
    return await AIX.From(file, undefined, true);
  } catch (error) {
    if (error instanceof AixCheckError) console.table(error.diagnostics);
    throw error;
  }
}
~~~

默认的 `AIX.From(file)` 会校验归档结构，并按需读取各个文件。无效归档和解包失败会各自报错。Rust 读取器提供发布者签名验证。

### 读取页面和工具详情 {#metadata-resolution-and-diagnostics}

`pkg.getPages()` 读取页面元数据，`pkg.getTools()` 返回根据页面生成的 OpenAI 兼容工具定义。页面信息不完整时，这两个方法可能使用默认值。

要确认哪些页面成功读取，可以用 `pkg.resolveMetadata()`。它返回 `pages`、`tools` 和 `diagnostics`。解析失败的页面会产生诊断，并从页面和工具列表中排除。

::: details 页面解析规则
页面需要一个可读取的 `.ink` 文件，或者同时提供 `.json` 和 `.wxml`。损坏的 `.ink` 文件会使该页面报错。可选的定义、Schema 和样式可以省略。如果两种样式表同时存在，优先读取 `.wcss`。

`app.json.pages` 缺失或无效时，诊断会指向 `app.json`。空页面列表有效。成功解析的页面保持声明顺序，并沿用常规的工具目标规则。组件与标题的元数据会单独读取。
:::

## Progressive Assets（渐进资源） {#progressive-assets}

渐进资源允许 AIUI 分片播放媒体。选择远程交付后，包内保留启动播放所需的信息，AIUI 按需下载分片。

在 JavaScript 模块或 `.ink` 脚本中声明媒体。导入得到的 `AssetRef` 形如 `aix-asset:<id>`，可以直接交给 AIUI 的视频或音频组件。

~~~javascript
import clip from './assets/clip.mp4' with { type: 'video', progressive: 'prefer' };
~~~

音频使用 `type: 'audio'`。目前支持 H.264 MP4 和 AAC-LC M4A。对于符合条件的普通媒体文件，AIX 可以将其分片封装为 MP4，无需重新编码。

### 选择分片的存放位置 {#choose-where-the-pieces-are-stored}

默认将原始媒体放在包内。要远程交付，请给 `packFromSource()` 提供 `onProgressiveAsset` 回调。

1. 返回 `{ delivery: 'embedded' }` 可将媒体保留在包内。
2. 要远程交付，调用 `prepare()`，再用自己的存储服务上传 `plan.remoteObjects`。
3. 返回计划 ID 和上传回执。AIX 会先校验回执，再生成包。

~~~javascript
const result = await AIX.packFromSource(files, {
  async onProgressiveAsset(candidate, { prepare, signal }) {
    if (!candidate.progressiveSupport.supported) {
      return { delivery: 'embedded' };
    }

    const plan = await prepare();
    // Implement this function with your storage service.
    const receipt = await uploadToStorage(plan, signal);
    return { delivery: 'progressive', planId: plan.id, receipt };
  },
});
~~~

`uploadToStorage(plan, signal)` 由调用方实现。所有对象上传成功后，该函数返回 `ProgressiveAssetUploadReceipt`。例如，可以按下面的方式组织回执，并根据自己的存储服务实现 `uploadObject`，让它上传字节并返回可以访问的 HTTPS 地址。

~~~javascript
async function uploadToStorage(plan, signal) {
  const objects = await Promise.all(plan.remoteObjects.map(async object => {
    // Implement uploadObject to upload the bytes and return their HTTPS URL.
    const url = await uploadObject(object.bytes, { id: object.id, signal });
    return { id: object.id, size: object.size, sha256: object.sha256, url };
  }));

  return {
    uploadId: crypto.randomUUID(),
    assetVersion: plan.assetVersion,
    objects,
  };
}
~~~

| 回执字段 | 填写内容 |
| --- | --- |
| `uploadId` | 本次上传的非空标识。示例在本地生成，也可以使用服务端提供的标识。 |
| `assetVersion` | 原样使用 `prepare()` 返回的 `plan.assetVersion`。 |
| `objects` | `plan.remoteObjects` 中每个对象对应一项，不能重复或遗漏。 |
| `objects[].id`、`size`、`sha256` | 原样使用对应待上传对象的字段，`size` 的单位是字节。 |
| `objects[].url` | 对应对象上传后的完整、稳定 HTTPS 地址，不能包含凭据、查询参数或片段标识。 |

将 `signal` 传给上传函数，使上传可以响应取消操作。AIX 在写入包前校验回执中的标识、大小、哈希和 URL 格式。上传失败、回执无效或取消操作都会终止打包。

### 在包内查看分片地址 {#find-the-urls-in-the-package}

选择远程交付时，AIX 将资源索引存入 `META-INF/assets/progressive_assets/index.json`。索引将 `aix-asset:<id>` 中的 ID 映射到 `asset.json` 描述文件。每个分片的完整 HTTPS 地址都保存在描述文件的 `locators` 中。

播放时，AIUI 从 `layout.fragments[n].object` 取得对象 ID，再读取 `locators[objectId].url`。AIUI 按需下载并核对大小和 SHA-256。初始化文件 `init.mp4` 从包内读取。参见[包格式规范](/zh-CN/spec#_7-1-渐进媒体元数据)。

### 在 Play 中试用打包 {#play}

打包前，可用 `AIX.listProgressiveAssets(files)` 查看媒体路径、类型、源文件大小和格式支持情况。传入与 `AIX.pack()` 相同的已筛选文件列表。

在 [Play 的构建页](/zh-CN/play?mode=build) 选择项目文件夹，即可看到媒体列表。点击**构建并下载**会为支持的文件生成分片，并模拟上传。在**检查**页打开下载的包，可以查看保存的占位地址。

<div id="_5-优化已有的包"></div>

## 优化已有的包 {#aix-optimize-data-options}

将浏览器 `File` 或包字节传给 `AIX.optimize()`，即可生成优化副本。这里可以复用打包示例里的下载函数。

~~~javascript
const result = await AIX.optimize(file, { level: 2 });
downloadPackage(result.data, 'my-agent-optimized.aix');
~~~

省略选项会启用默认的 JSON、PNG 和 JPEG 优化。副本会保留 `VERSION` 和 `agent_id`。由于包内容可能变化，AIX 会移除已有签名。

<div id="_6-常用方法速查"></div>

## API Reference {#api-reference}

[静态方法](#static-methods) · [实例方法](#instance-methods) ·
[选项和回调](#options-and-callbacks) · [类型](#type-reference)

`?` 表示可选参数，空括号表示无需传参。返回 `Promise` 的方法可以用 `await` 等待结果。

### 静态方法 {#static-methods}

通过导入的 `AIX` 类调用这些方法。

| 方法 | Promise 结果 |
| --- | --- |
| [`AIX.From(data, limits?, check?)`](#aix-from) | 打开的 `AIX` 包 |
| [`AIX.checkFromSource(files, options?)`](#aix-check-from-source) | `CheckReport` |
| [`AIX.checkFromFiles(files, options?)`](#aix-check-from-files) | `CheckReport` |
| [`AIX.packFromSource(files, options?)`](#aix-pack-from-source) | `PackResult` |
| [`AIX.packFromFiles(files, options?)`](#aix-pack-from-files) | `PackResult` |
| [`AIX.pack(files, options?)`](#aix-pack) | `PackResult` |
| [`AIX.listProgressiveAssets(files, options?)`](#aix-list-progressive-assets) | `ProgressiveAssetCandidate[]` |
| [`AIX.optimize(data, options?)`](#aix-optimize) | `PackResult` |

#### AIX.From(data, limits?, check?) {#aix-from}

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits, check?: boolean): Promise<AIX>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | 完整的 `.aix` 字节或浏览器文件。要打开网址，请先下载文件字节。 |
| `limits` | `AixReadLimits` | 可选的[归档读取上限](#limit-archive-reads)。 |
| `check` | `boolean` | 默认为 `false`。传入 `true` 会先检查解包后的源码，再返回包对象。 |

**返回** 一个 Promise，结果是带有[实例方法](#instance-methods)的包对象。

**错误**包括归档无效、超出上限或读取失败。`check` 为 `true` 时，源码错误会使 Promise 以 [`AixCheckError`](#reference-aix-check-error) 拒绝。只有提醒时仍可打开。

#### AIX.checkFromSource(files, options?) {#aix-check-from-source}

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 每项包含项目相对路径 `path` 和 `Uint8Array` 类型的 `data`。根目录应包含 `app.json`。 |
| `options` | `CheckOptions` | 可选的进度设置。 |
| `options.onProgress` | `(event: CheckProgressEvent) => void` | 接收[检查阶段和结果](#check-progress-and-browser-responsiveness)。 |

SDK 会应用项目的 `.aixignore`，检查剩下的源文件。

**返回** 一个 Promise，结果为 [`CheckReport`](#source-check-types)。源码的错误和提醒都放在 `diagnostics` 数组中。本次检查未发现问题时，该数组为空。

**错误**如收集输入、加载检查器或执行回调失败，会使 Promise 拒绝。

#### AIX.checkFromFiles(files, options?) {#aix-check-from-files}

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `File[]` | 浏览器文件。优先使用 `webkitRelativePath`，否则使用 `name`。选择文件夹时会移除文件共有的外层目录。 |
| `options` | `CheckOptions` | 可选的进度设置。 |
| `options.onProgress` | `(event: CheckProgressEvent) => void` | 接收[检查阶段和结果](#check-progress-and-browser-responsiveness)。 |

**返回** 一个 Promise，结果为 [`CheckReport`](#source-check-types)。`diagnostics` 包含源码错误和提醒。

**错误**如文件无法读取或回调抛出异常，会使 Promise 拒绝。

#### AIX.packFromSource(files, options?) {#aix-pack-from-source}

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 项目文件的路径和字节，根目录包含 `app.json`。SDK 会规范化路径，应用根目录和子目录的 `.aixignore`，并排除规则文件和 `.aix/` 缓存。 |
| `options` | `PackFromSourceOptions` | 可选的[打包设置](#pack-options)。 |
| `options.onProgress` | `(event: PackProgressEvent) => void` | 接收[打包阶段和文件报告](#build-progress-callback)。 |
| `options.onProgressiveAsset` | [渐进资源回调](#progressive-asset-callback) | 接收 `ProgressiveAssetCandidate` 和 `ProgressiveAssetPackContext`，返回 `ProgressiveAssetDecision` 或解析为该类型的 Promise。 |

**返回** 一个 Promise，结果为 [`PackResult`](#pack-result)。`data` 是包字节，`report` 是文件大小报告，`warnings` 是打包提醒，`progressiveAssets` 是媒体交付结果。

**错误**如路径或忽略规则无效、规范化后路径重复、回调失败或取消，会使 Promise 拒绝。

#### AIX.packFromFiles(files, options?) {#aix-pack-from-files}

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `File[]` | 浏览器文件。优先使用 `webkitRelativePath`，否则使用 `name`。按这些路径打包时，`app.json` 必须在根目录。 |
| `options` | `PackFromSourceOptions` | 可选的[打包设置](#pack-options)。 |
| `options.onProgress` | `(event: PackProgressEvent) => void` | 接收[打包阶段和文件报告](#build-progress-callback)。 |
| `options.onProgressiveAsset` | [渐进资源回调](#progressive-asset-callback) | 接收 `ProgressiveAssetCandidate` 和 `ProgressiveAssetPackContext`，返回 `ProgressiveAssetDecision` 或解析为该类型的 Promise。 |

此方法保留浏览器路径中的外层目录。需要移除前缀时，可参考[文件夹选择示例](#aix-packfromfiles-files-options)，处理完再调用 `packFromSource()`。

**返回** 一个 Promise，结果为 [`PackResult`](#pack-result)，包含包字节、文件大小报告、提醒和媒体交付结果。

**错误**如文件无法读取或打包失败，会使 Promise 拒绝。

#### AIX.pack(files, options?) {#aix-pack}

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 已筛选的文件列表，路径已规范化，`app.json` 在根目录。调用前自行应用忽略规则。 |
| `options` | `PackOptions` | 可选的[打包设置](#pack-options)。 |
| `options.onProgressiveAsset` | [渐进资源回调](#progressive-asset-callback) | 接收 `ProgressiveAssetCandidate` 和 `ProgressiveAssetPackContext`，返回 `ProgressiveAssetDecision` 或解析为该类型的 Promise。 |

AIX 会生成 `VERSION` 和包清单。请从输入列表中排除 `VERSION` 与保留的元数据路径。

**返回** 一个 Promise，结果为 [`PackResult`](#pack-result)，包含包字节、文件大小报告、提醒和媒体交付结果。

**错误**如输入无效、回调失败或取消，会使 Promise 拒绝。

#### AIX.listProgressiveAssets(files, options?) {#aix-list-progressive-assets}

~~~typescript
static listProgressiveAssets(
  files: AixInputFile[],
  options?: { signal?: AbortSignal },
): Promise<ProgressiveAssetCandidate[]>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 已筛选并规范化的文件列表，格式与 `pack()` 相同。 |
| `options` | `{ signal?: AbortSignal }` | 可选的发现设置。 |
| `options.signal` | `AbortSignal` | 取消查找资源。 |

**返回** 一个 Promise，结果为 [`ProgressiveAssetCandidate`](#progressive-asset-types) 对象数组。每项包含源路径、媒体类型、字节数、引用位置和当前格式是否支持远程交付。没有资源声明时返回空数组。

**错误**如声明无效、源文件缺失或取消，会使 Promise 拒绝。

#### AIX.optimize(data, options?) {#aix-optimize}

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | 已有的 `.aix` 包。 |
| `options` | `OptimizeOptions` | 可选的[优化设置](#optimize-options)。省略时使用默认值。 |

**返回** 一个 Promise，结果为优化副本的 [`PackResult`](#pack-result)。`data` 是新包，`report` 用于比较大小，`progressiveAssets` 为空。副本保留 `VERSION` 和 `agent_id`，并移除已有签名。

**错误**如归档无效、读取失败或优化失败，会使 Promise 拒绝。

<div id="aix-instance"></div>
<div id="打开包后查看内容"></div>

### 实例方法 {#instance-methods}

在 `AIX.From()` 返回的包对象上调用这些方法。它们同步返回。

| 用途 | 方法 |
| --- | --- |
| 归档文件 | [`list()`](#pkg-list)、[`readFile()`](#pkg-read-file) |
| 标识和兼容性 | [`getVersion()`](#pkg-get-version)、[`getAgentId()`](#pkg-get-agent-id)、[`supportsEngine()`](#pkg-supports-engine) |
| 应用设置 | [`getTitle()`](#pkg-get-title)、[`getPermissions()`](#pkg-get-permissions) |
| 页面、小组件和工具 | [`getPages()`](#pkg-get-pages)、[`resolveMetadata()`](#pkg-resolve-metadata)、[`getWidgets()`](#pkg-get-widgets)、[`getTools()`](#pkg-get-tools) |

#### list() {#pkg-list}

~~~typescript
list(): AixEntry[]
~~~

**参数** 无。

**返回** 归档条目的 `AixEntry[]`。条目包含规范化的 `name`、未压缩字节数 `size` 和压缩后字节数 `compressed_size`。

<div id="pkg-readfile-name"></div>

#### readFile(name) {#pkg-read-file}

~~~typescript
readFile(name: string): Uint8Array
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `name` | `string` | 精确的规范化条目名称，例如 `app.json`。可以通过 [`list()`](#pkg-list) 查询。 |

**返回** 条目解压后的字节，类型为 `Uint8Array`。

**错误**包括条目缺失或损坏、压缩方式不受支持以及超过读取上限。

<div id="pkg-getversion"></div>

#### getVersion() {#pkg-get-version}

~~~typescript
getVersion(): string | undefined
~~~

**参数** 无。

**返回** `VERSION` 中的构建标识。条目缺失、无法读取或不是有效 UTF-8 时返回 `undefined`。

<div id="pkg-getagentid"></div>

#### getAgentId() {#pkg-get-agent-id}

~~~typescript
getAgentId(): string | undefined
~~~

**参数** 无。

**返回** 清单中的稳定项目标识 `agent_id`。没有标识时返回 `undefined`。

**错误**包括清单无法读取、清单格式无效或标识无效。

<div id="pkg-supportsengine-currentversion"></div>

#### supportsEngine(currentVersion) {#pkg-supports-engine}

~~~typescript
supportsEngine(currentVersion: string): boolean
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `currentVersion` | `string` | 具体的 AIUI 版本号，例如 `0.18.0`。 |

**返回** 版本符合清单的 `engine` 范围时为 `true`，超出范围时为 `false`。

**错误**包括清单缺失或无效、版本范围无效和版本号无效。

<div id="pkg-gettitle"></div>

#### getTitle() {#pkg-get-title}

~~~typescript
getTitle(): string | undefined
~~~

**参数** 无。

**返回** `app.json.window.navigationBarTitleText` 字符串。标题缺失或配置无法读取时返回 `undefined`。

<div id="pkg-getpermissions"></div>

#### getPermissions() {#pkg-get-permissions}

~~~typescript
getPermissions(): string[]
~~~

**参数** 无。

**返回** 项目声明的权限列表，保留原有顺序、重复项和自定义名称。列表未声明或为空时返回 `[]`。

**错误**包括 `app.json` 无法读取或无效，以及 `permissions` 不是字符串数组。

<div id="pkg-getpages"></div>

#### getPages() {#pkg-get-pages}

~~~typescript
getPages(): PageInfo[]
~~~

**参数** 无。

**返回** 按声明顺序排列的页面。页面资源缺失或无效时可能使用默认值。页面列表无法读取或无效时返回 `[]`。如需页面诊断，使用 [`resolveMetadata()`](#pkg-resolve-metadata)。

<div id="pkg-resolvemetadata"></div>

#### resolveMetadata() {#pkg-resolve-metadata}

~~~typescript
resolveMetadata(): MetadataResolution
~~~

**参数** 无。

**返回** `{ pages, tools, diagnostics }`。页面和工具列表只包含成功解析的页面。解析失败的页面会产生带资源路径和说明的诊断。页面列表无法读取或无效时，诊断会指向 `app.json`。

<div id="pkg-getwidgets-locale"></div>

#### getWidgets(locale?) {#pkg-get-widgets}

~~~typescript
getWidgets(locale?: string): WidgetInfo[]
~~~

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `locale` | `string` | 可选的语言区域，例如 `en-US`。匹配的 `app.<locale>.json` 可以覆盖小组件的名称和说明。没有覆盖项时使用 `app.json` 的值。 |

**返回** 项目声明的小组件列表。没有声明时返回 `[]`。

**错误**包括配置无效、匹配的语言文件无效或小组件入口缺失。

<div id="pkg-gettools"></div>

#### getTools() {#pkg-get-tools}

~~~typescript
getTools(): Tool[]
~~~

**参数** 无。

**返回** 根据页面列表生成的 OpenAI 兼容工具定义。页面默认值与 `getPages()` 相同。仅查看成功解析的页面所对应的工具时，读取 `resolveMetadata().tools`。

### 选项和回调 {#options-and-callbacks}

<div id="限制读取大小"></div>
<div id="aixreadlimits"></div>

#### 归档读取上限 {#limit-archive-reads}

`AixReadLimits` 的字段均可省略。每个值必须是非负安全整数。允许达到上限，`0` 表示不允许对应的条目或字节。省略的字段没有上限。未知字段会使 `AIX.From()` 拒绝。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `maxEntries` | `number` | 最大条目数，包括目录和签名条目。 |
| `maxEntryBytes` | `number` | 单个条目解压后的最大字节数。 |
| `maxTotalBytes` | `number` | 所有条目声明的未压缩字节数之和的上限。重复读取不累加。 |

~~~javascript
const pkg = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
~~~

::: details 上限与内存
上限适用于归档条目。原始输入缓冲区、解析出的对象、调用方保留的副本，以及源码检查和优化使用的内存，均不计入这些上限。

Rust 使用 `AixReader::new_with_limits()`。直接调用 WASM 读取器时使用 `AixReaderWasm.with_limits(data, limits)`。
:::

<div id="checkoptions"></div>

#### 检查进度回调 {#check-progress-and-browser-responsiveness}

~~~typescript
onProgress?: (event: CheckProgressEvent) => void
~~~

通过 `CheckOptions` 将回调传给 `checkFromSource()` 或 `checkFromFiles()`。SDK 同步调用回调并传入一个事件。返回值会被忽略，抛出异常会使检查拒绝。

| `event.type` | 附加字段 | 触发时机 |
| --- | --- | --- |
| `loading_checker` | 无 | SDK 开始加载检查器。 |
| `checking_sources` | `fileCount: number` | 开始检查。数量为应用忽略规则前的输入文件数。 |
| `check_finished` | `check: CheckResult` | 一类检查结束，返回 ID、名称、状态、错误与提醒数量，以及可选的详情。 |
| `collecting_diagnostics` | 无 | 即将返回诊断报告。 |

检查 ID 包括 `json`、`structure`、`syntax`、`permissions` 和 `schemas`。状态可以是 `passed`、`warning`、`failed` 或 `skipped`。加载完成后，源码检查同步执行。需要让页面在检查期间保持响应时，可以在 Web Worker 中执行。

<div id="packoptions-packfromsourceoptions"></div>

#### 打包选项 {#pack-options}

`PackOptions` 的字段均可省略。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `buildId` | `string` | 写入 `VERSION` 的构建标识。默认生成 UUID v4。 |
| `agentId` | `string` | 写入 `agent_id` 的稳定项目标识。使用小写 ASCII 字母、数字、`_` 或 `-`，不能为空，且首尾不能是 `-`。省略时不设置标识。 |
| `engine` | `string` | AIUI 版本范围。默认采用 `app.json.engine`，再回退到 `*`。 |
| `optimize` | `false \| OptimizeOptions` | 省略或传 `false` 时关闭可选的 JSON 和图片优化。传入对象会开启[优化设置](#optimize-options)。 |
| `signal` | `AbortSignal` | 取消打包，也可用于取消使用同一信号的上传。 |
| `onProgressiveAsset` | `(candidate, context) => ProgressiveAssetDecision \| Promise<ProgressiveAssetDecision>` | [决定每个资源的交付方式](#progressive-asset-callback)。 |

`PackFromSourceOptions` 还提供 `onProgress?: (event: PackProgressEvent) => void`，供 `packFromSource()` 和 `packFromFiles()` 使用。

#### 打包进度回调 {#build-progress-callback}

~~~typescript
onProgress?: (event: PackProgressEvent) => void
~~~

SDK 每次报告打包阶段时都会传入一个事件。返回值会被忽略，抛出异常会使打包拒绝。提前结束的打包可能不会发出最后一个事件。

| `event.type` | 附加字段 | 触发时机 |
| --- | --- | --- |
| `transferring_files_to_wasm` | 无 | SDK 开始向打包器传入文件。 |
| `collecting_source_inputs` | 无 | 开始收集源文件。 |
| `resolving_engine` | 无 | 打包器确定兼容版本范围。 |
| `preparing_files` | 无 | 开始处理选定文件。 |
| `file_finished` | `report: FileOptimizeReport` | 一个条目处理完成，也包括生成的 `VERSION`。 |
| `finalizing_archive` | 无 | 开始写入最终归档和清单。 |

#### 渐进资源回调 {#progressive-asset-callback}

~~~typescript
onProgressiveAsset?: (
  candidate: ProgressiveAssetCandidate,
  context: ProgressiveAssetPackContext,
) => ProgressiveAssetDecision | Promise<ProgressiveAssetDecision>
~~~

SDK 为每个声明的资源调用一次回调，等待当前资源的交付决定，再处理下一个。

| 参数 | 类型 | 回调收到的内容 |
| --- | --- | --- |
| `candidate` | `ProgressiveAssetCandidate` | 资源的源路径、大小、媒体类型、引用位置和格式支持情况。 |
| `context.signal` | `AbortSignal` | 可传给上传函数的构建取消信号。 |
| `context.prepare` | `() => Promise<ProgressiveAssetPlan>` | 在回调内调用，准备远程交付。计划包含 `id`、`assetVersion`、时间线元数据和 `remoteObjects` 中的字节。媒体不受支持时，Promise 会拒绝。 |

直接返回下列任一决定，或返回解析为该决定的 Promise。

| 决定 | 效果 |
| --- | --- |
| `{ delivery: 'embedded' }` | 把源媒体保留在包内。省略回调时默认采用此方式。 |
| `{ delivery: 'progressive', planId, receipt }` | 远程交付准备好的分片。用 `prepare()` 返回的 `id` 作为 `planId`，并提供上传回执。 |

回执包含非空的 `uploadId`、计划的 `assetVersion`，以及 `plan.remoteObjects` 中每个对象的对应条目。每项包括匹配的 `id`、`size`、`sha256` 和稳定的 HTTPS `url`。URL 不能带凭据、查询参数或片段标识。回调出错、回执无效或取消都会使打包拒绝。参见[上传示例](#progressive-assets)和[类型定义](#progressive-asset-types)。

<div id="optimizeoptions"></div>

#### 优化选项 {#optimize-options}

| 字段 | 类型 | 默认值 |
| --- | --- | --- |
| `level` | `1 \| 2 \| 3` | `2` |
| `json` | `boolean` | `true`，优化 JSON。 |
| `png` | `boolean` | `true`，优化 PNG 图片。 |
| `jpeg` | `boolean` | `true`，优化 JPEG 图片。 |

打包时传入选项对象即可开启优化。`AIX.optimize()` 即使省略选项，也会使用这些默认值。

#### 打包和优化结果 {#pack-result}

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `data` | `Uint8Array` | 新归档的字节。 |
| `report` | `OptimizeReport` | 各条目及总字节数，按 ZIP 压缩前的大小统计。 |
| `warnings` | `string[]` | 操作期间的提醒。 |
| `progressiveAssets` | `ProgressiveAssetReport[]` | 声明资源的交付结果，包括交付方式和保留的源文件字节数。优化已有包时返回空数组。 |

#### AixCheckError {#reference-aix-check-error}

~~~typescript
constructor(report: CheckReport)
~~~

`AIX.From()` 的可选源码检查发现错误时，Promise 会以这个 `Error` 子类拒绝。构造函数接收完整检查报告。实例的 `name` 为 `'AixCheckError'`，`message` 是摘要，`report` 是传入的报告，`diagnostics` 返回 `report.diagnostics`，其中也包括提醒。

<div id="完整类型定义"></div>

## 类型参考 {#type-reference}

展开相应分组即可查看 TypeScript 定义。

### 文件输入和打包结果 {#file-and-build-types}

::: details 文件、读取上限和打包选项
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
    signal?: AbortSignal;
    onProgressiveAsset?: (
        candidate: ProgressiveAssetCandidate,
        context: ProgressiveAssetPackContext,
    ) => ProgressiveAssetDecision | Promise<ProgressiveAssetDecision>;
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
    progressiveAssets: ProgressiveAssetReport[];
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
:::

### 渐进资源类型 {#progressive-asset-types}

::: details Progressive Asset 类型
~~~typescript
type AssetRef = `aix-asset:${string}`;
type ProgressiveAssetKind = 'audio' | 'video';

interface ProgressiveAssetCandidate {
  id: string;
  kind: ProgressiveAssetKind;
  sourcePath: string;
  sourceSha256: string;
  sourceSize: number;
  references: { path: string; line: number; column: number }[];
  format: string;
  dependencies: string[];
  progressiveSupport: {
    supported: boolean;
    profile?: string;
    requiredCapabilities: string[];
    features: string[];
    reason?: string;
  };
}

interface ProgressiveAssetObject {
  id: string;
  size: number;
  sha256: string;
}

interface ProgressiveAssetPlan {
  id: string;
  assetVersion: string;
  profile: string;
  localMetadata: Record<string, Omit<ProgressiveAssetObject, 'id'>>;
  layout: {
    init: string;
    tracks: Record<string, { kind: ProgressiveAssetKind; timescale: number }>;
    fragments: { object: string; ranges: Record<string, { start: number; end: number; presentation_start: number; presentation_end: number }> }[];
  };
  remoteObjects: (ProgressiveAssetObject & { bytes: Uint8Array })[];
}

interface ProgressiveAssetUploadReceipt {
  uploadId: string;
  assetVersion: string;
  objects: (ProgressiveAssetObject & { url: string })[];
}

type ProgressiveAssetDecision =
  | { delivery: 'embedded' }
  | { delivery: 'progressive'; planId: string; receipt: ProgressiveAssetUploadReceipt };

interface ProgressiveAssetPackContext {
  signal: AbortSignal;
  prepare(): Promise<ProgressiveAssetPlan>;
}

interface ProgressiveAssetReport {
  id: string;
  kind: ProgressiveAssetKind;
  sourcePath: string;
  delivery: 'embedded' | 'progressive';
  assetVersion?: string;
  retainedSourceBytes: number;
}
~~~
:::

### 打包进度事件 {#packprogressevent}

::: details PackProgressEvent
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

`file_finished` 包含刚处理完的条目报告，其中也包括生成的 `VERSION`。
:::

### 源码检查类型 {#source-check-types}

::: details 检查报告和进度事件
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
:::

### 包和派生模型 {#package-and-derived-models}

::: details 归档条目、页面、小组件和工具
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

页面的 `size` 和工具的 `layout` 包含宽高约束。工具的 `type` 为 `'function'`，目标是 `'_current'` 或 `'_blank'`。`JsonSchema` 可以是任意 JSON 值，使用前可以通过源码检查验证 Schema 声明。

这些元数据对象中缺失的字符串为 `null`，包括 WASM 的原始输出。`getTitle()` 和 `getVersion()` 等独立方法在缺少相应值时返回 `undefined`。
:::

## 本地构建 SDK {#build-from-source}

~~~bash
cd crates/aix-web
npm install
npm run build
~~~

构建产物位于 `crates/aix-web/dist`。自行托管时，请提供完整的产物目录。SDK 使用 `pkg-reader/` 打开包，检查、打包和优化时按需加载 `pkg/`。参见 [WASM 加载说明（英文）](/wasm?lang=en)。
