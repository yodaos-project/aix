# Web API 使用指南

`@yodaos-pkg/aix` 让你在网页中检查 AIX 项目、生成 `.aix` 文件，并读取包里的页面、工具和其他文件。

本文适用于在前端项目中接入 AIX 的开发者。示例使用 JavaScript：HTML 放到页面中，JavaScript 放到项目的入口脚本中。
如果你想先体验功能，可以直接打开 [Play](/play?lang=zh-CN)；如果你想在终端里操作项目，请看 [CLI 使用指南](/cli?lang=zh-CN)。

需要查阅具体参数、返回值和类型时，可直接跳到文末的 [API Reference](#api-reference)。

## 1. 安装并导入

在已有的前端项目中安装：

~~~bash
npm install @yodaos-pkg/aix
~~~

在需要使用的脚本中导入：

~~~javascript
import { AIX } from '@yodaos-pkg/aix';
~~~

检查、打包、读取和优化文件时，需要等待操作完成，所以示例会使用 `async` 和 `await`。
导入后直接调用 `AIX` 上的方法即可。

## 2. 检查项目并下载打包结果

准备好你的 AIX 项目，选择包含 `app.json` 的那一层目录。例如：

~~~text
my-agent/
├── app.json
└── pages/
    └── home.ink
~~~

下面做一个“检查并打包”按钮：先找出源码中的问题，检查没有错误后，再生成可以下载的 `.aix` 文件。

### 添加文件选择框和按钮

~~~html
<input id="project-directory" type="file" webkitdirectory multiple>
<button id="build-package" type="button">检查并打包</button>
<pre id="build-result">请选择 AIX 项目目录。</pre>
~~~

### 添加处理代码

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

// 将生成的文件下载到电脑。后面的优化示例也可以复用这个函数。
function downloadPackage(data, filename) {
  const blob = new Blob([new Uint8Array(data)], {
    type: 'application/octet-stream',
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

const directoryInput = document.getElementById('project-directory');
const buildButton = document.getElementById('build-package');
const buildOutput = document.getElementById('build-result');

buildButton.addEventListener('click', async () => {
  const selectedFiles = Array.from(directoryInput.files ?? []);
  if (selectedFiles.length === 0) {
    buildOutput.textContent = '请先选择项目目录。';
    return;
  }

  buildButton.disabled = true;
  buildOutput.textContent = '正在检查项目…';

  try {
    const report = await AIX.checkFromFiles(selectedFiles);
    const errors = report.diagnostics.filter(item => item.severity === 'error');

    if (errors.length > 0) {
      buildOutput.textContent = errors.map(item =>
        `${item.path} 第 ${item.line} 行：${item.message}`
      ).join('\n');
      return;
    }

    // 路径从项目内部开始，例如 app.json、pages/home.ink。
    const files = await Promise.all(selectedFiles.map(async file => ({
      path: file.webkitRelativePath.split('/').slice(1).join('/'),
      data: new Uint8Array(await file.arrayBuffer()),
    })));

    buildOutput.textContent = '检查通过，正在打包…';
    const result = await AIX.packFromSource(files);
    downloadPackage(result.data, 'my-agent.aix');

    const warnings = [
      ...report.diagnostics.filter(item => item.severity === 'warning')
        .map(item => item.message),
      ...result.warnings,
    ];
    buildOutput.textContent = warnings.length > 0
      ? `已生成 my-agent.aix。请留意：\n${warnings.join('\n')}`
      : '已生成 my-agent.aix，可以在下载目录中找到它。';
  } catch (error) {
    buildOutput.textContent = `操作失败：${String(error)}`;
  } finally {
    buildButton.disabled = false;
  }
});
~~~

选择目录并点击按钮后，浏览器会下载 `my-agent.aix`。你也可以把生成的文件放到 [Play](/play?lang=zh-CN) 中，查看里面的页面和文件。

### 检查结果怎么看

`checkFromFiles()` 返回的 `diagnostics` 是问题列表。列表为空，表示本次检查没有发现问题。

| 字段 | 含义 |
| --- | --- |
| `severity` | `error` 表示需要修复的错误，`warning` 表示需要留意的提醒。 |
| `path` | 问题所在的项目文件。 |
| `line` / `column` | 问题所在的行和列。 |
| `message` | 问题说明。 |
| `code` | 问题编号，方便查找和记录。 |

检查能帮助你发现配置错误、缺失页面、组件引用问题、脚本语法错误，以及部分权限和页面参数配置问题。
检查通过后，仍需在实际运行环境中验证项目效果。

源码里有错误时，检查通常会正常返回问题列表，所以要像示例一样查看 `severity`。
文件无法读取或操作本身失败时，会进入 `catch`。

### 文件已经在代码中时

如果你的应用自己生成文件内容，可以用 `checkFromSource()` 检查。每个文件包含两个字段：

| 字段 | 怎么填写 |
| --- | --- |
| `path` | 项目内的相对路径，例如 `app.json`。 |
| `data` | 文件内容，使用 `Uint8Array`。文字内容可以按下面的方式转换。 |

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

async function checkExample() {
  const files = [
    {
      path: 'app.json',
      data: new TextEncoder().encode('{"pages":[]}'),
    },
  ];

  const report = await AIX.checkFromSource(files);
  console.table(report.diagnostics);
  return report;
}

checkExample().catch(console.error);
~~~

同样的 `files` 数组可以传给 `AIX.packFromSource(files)` 来打包。

## 3. 读取一个已有的 AIX 文件

如果你已经有 `.aix` 文件，可以让用户选择文件，然后显示它的基本信息。

~~~html
<input id="package-file" type="file" accept=".aix">
<pre id="package-info">请选择一个 .aix 文件。</pre>
~~~

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

const packageInput = document.getElementById('package-file');
const packageOutput = document.getElementById('package-info');

packageInput.addEventListener('change', async () => {
  const file = packageInput.files?.[0];
  if (!file) return;

  packageOutput.textContent = '正在读取…';

  try {
    const pkg = await AIX.From(file);
    packageOutput.textContent = JSON.stringify({
      标题: pkg.getTitle() ?? '未设置标题',
      构建标识: pkg.getVersion() ?? '未设置',
      声明的权限: pkg.getPermissions(),
      页面: pkg.getPages(),
      工具: pkg.getTools(),
    }, null, 2);
  } catch (error) {
    packageOutput.textContent = `读取失败：${String(error)}`;
  }
});
~~~

`AIX.From()` 返回一个可以继续查看内容的 `pkg`。例如，`pkg.getPages()` 查看页面，`pkg.list()` 查看文件列表。
`getVersion()` 返回这个 `.aix` 文件的构建标识，不是 npm 包的版本号。

### 从网址读取

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

async function loadPackage(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`下载失败：${response.status}`);
  }
  const data = new Uint8Array(await response.arrayBuffer());
  return AIX.From(data);
}

loadPackage('/agents/my-agent.aix')
  .then(pkg => console.log(pkg.getTitle(), pkg.getPages()))
  .catch(console.error);
~~~

把示例地址替换成你实际存放 `.aix` 文件的网址。

### 查看包里的文件

拿到 `pkg` 后，先用 `list()` 找到文件名，再用 `readFile()` 读取。例如，读取项目配置：

~~~javascript
function readAppConfig(pkg) {
  const data = pkg.readFile('app.json');
  const text = new TextDecoder().decode(data);
  return JSON.parse(text);
}
~~~

图片等文件可以把 `readFile()` 返回的内容交给 `Blob`，按普通浏览器文件的方式使用。

## 4. 按需设置打包选项

刚开始可以直接使用 `AIX.packFromSource(files)`。需要固定项目标识、指定运行版本或优化图片时，再传入第二个参数：

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

async function packProject(files) {
  return AIX.packFromSource(files, {
    agentId: 'my-agent',
    engine: '>=0.17.0',
    optimize: { level: 2 },
  });
}
~~~

| 选项 | 用途 |
| --- | --- |
| `agentId` | 项目标识。同一项目多次打包时复用相同的值。 |
| `engine` | 指定兼容的运行版本。不传时采用 `app.json` 中的 `engine`；项目也没配置时不限制版本。 |
| `optimize` | 传入 `{ level: 2 }` 可开启 JSON 和图片优化；`level` 可以是 `1`、`2`、`3`。 |
| `buildId` | 本次构建的标识。不传会自动生成，一般不用手动设置。 |
| `onProgress` | 接收打包步骤提示，可用于展示当前操作。适用于 `packFromSource()` 和 `packFromFiles()`。 |

`agentId` 使用小写英文字母、数字、`_` 或 `-`，不能以 `-` 开头或结尾，也不能为空。
省略 `optimize` 或设为 `false` 时，不额外优化 JSON 和图片；独立的 JS/TS 文件仍会进行代码压缩。

打包结果中，`data` 是生成的文件内容，`warnings` 是提醒列表，`report` 是文件大小变化报告。

## 5. 优化已有的包

`AIX.optimize()` 尝试缩小已有包的体积。下面的函数接收用户选择的 `.aix` 文件，下载优化后的结果：

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

async function optimizePackage(file) {
  const result = await AIX.optimize(file, { level: 2 });
  downloadPackage(result.data, 'my-agent-optimized.aix');
}
~~~

这里的 `downloadPackage()` 复用第 2 步中的下载函数。
如果想单独选择优化哪些内容，可以传入 `json`、`png`、`jpeg`，例如 `{ level: 2, png: false }`。

## 6. 常用方法速查

### 检查、打包和读取入口

以下方法直接通过导入的 `AIX` 调用，操作时需要 `await`：

| 方法 | 什么时候用 | 返回什么 |
| --- | --- | --- |
| `AIX.checkFromFiles(files, options?)` | 检查用户选择的项目文件。 | 含 `diagnostics` 的检查结果。 |
| `AIX.checkFromSource(files, options?)` | 检查已经组织为 `path`、`data` 的文件。 | 含 `diagnostics` 的检查结果。 |
| `AIX.packFromSource(files, options?)` | 打包项目源码，通常选它。 | 含 `data`、`warnings`、`report` 的打包结果。 |
| `AIX.packFromFiles(files, options?)` | 按浏览器 `File` 自带的相对路径打包。须保证 `app.json` 位于根目录。 | 打包结果。 |
| `AIX.pack(files, options?)` | 打包已整理好的文件，不应用项目的 `.aixignore`。 | 打包结果。 |
| `AIX.From(fileOrData, limits?)` | 打开已有的 `.aix` 文件。 | 用于查看包内容的对象。 |
| `AIX.optimize(fileOrData, options?)` | 优化已有的 `.aix` 文件。 | 优化后的打包结果。 |

目录选择器的完整打包流程见第 2 步，其中已把文件路径整理成项目内路径。
检查方法也接受 `options.onProgress` 回调，用来接收检查步骤提示。

### 打开包后查看内容

下表的 `pkg` 是 `await AIX.From(...)` 返回的结果。这些方法直接返回结果，不需要再写 `await`：

| 方法 | 可以查看什么 |
| --- | --- |
| `pkg.getTitle()` | 项目标题。 |
| `pkg.getVersion()` | 包的构建标识。 |
| `pkg.getAgentId()` | 项目标识。 |
| `pkg.getPermissions()` | 包里声明的权限列表，例如 `GEOLOCATION`、`CAMERA`。 |
| `pkg.getPages()` | 页面列表及页面参数说明。 |
| `pkg.getWidgets(locale?)` | 小组件列表。传入 `en-US` 等语言名称可读取对应的展示文案。 |
| `pkg.getTools()` | 提供给 AI 的工具定义，例如工具名称和可接收的参数。 |
| `pkg.list()` | 包里的文件名称和大小。 |
| `pkg.readFile(name)` | 指定文件的内容。文件名使用 `list()` 中的名称。 |
| `pkg.resolveMetadata()` | 页面、工具及它们的读取问题。页面列表不符合预期时，可以用它排查。 |
| `pkg.supportsEngine(version)` | 是否兼容指定运行版本，例如 `0.17.0`。 |

没有填写的标题或标识可能为空，展示时可以像示例一样提供默认文字。
读取损坏文件、查询不存在的文件或检查无效的版本信息时，方法可能报错，可使用 `try/catch` 处理。

### 限制读取大小

读取外部用户上传的包时，可以按你的业务需要设置文件数量和大小上限：

~~~javascript
import { AIX } from '@yodaos-pkg/aix';

async function openUploadedPackage(file) {
  return AIX.From(file, {
    maxEntries: 1000,
    maxEntryBytes: 8 * 1024 * 1024,
    maxTotalBytes: 32 * 1024 * 1024,
  });
}
~~~

这组示例允许最多 1000 个条目，单个条目最大 8 MiB，全部条目的大小合计最大 32 MiB。
不填写的项目没有上限；超过设置的上限会报错。

## 常见问题

### 检查通过，是否代表一定能正常运行？

检查用于提前发现源码和配置问题。页面交互、设备能力和实际效果仍需要运行项目后验证。
TypeScript 的类型是否正确，还需要由你项目自己的类型检查工具确认。

### 为什么检查结果有问题，但没有进入 catch？

源码问题通过 `diagnostics` 返回。检查结果里有 `error` 时，应先修复对应文件，再继续打包。
`catch` 用于处理文件读取失败、输入无法处理等操作失败。

### 为什么项目里没有找到 app.json？

请选择包含 `app.json` 的项目根目录。自己提供文件列表时，路径应写成 `app.json`，
而不是 `my-agent/app.json`，也不是电脑上的绝对路径。

### 只想试一试，不想写代码怎么办？

打开 [Play](/play?lang=zh-CN)，可以在网页中选择项目或 `.aix` 文件进行体验。

## API Reference

以下是 v0.11.0 的完整公开 API 参考。`?` 表示参数或字段可省略，`Promise<T>` 表示需要使用 `await` 等待得到 `T`。
本节的类型名称都可以通过 `import type { 类型名 } from '@yodaos-pkg/aix'` 导入。

### 静态方法

这组方法通过导入的 `AIX` 调用。

#### AIX.From(data, limits?)

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits): Promise<AIX>
~~~

打开一个完整的 `.aix` 文件，得到用于查看内容的包对象。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | 用户选择的 `.aix` 文件，或已经取得的文件内容；不能直接传网址。 |
| `limits` | `AixReadLimits` | 可选的读取上限。不填写的项目没有上限。 |

**返回：** `Promise<AIX>`。返回后可调用 `getPages()`、`readFile()` 等实例方法。

**错误处理：** 文件不是有效的 AIX 包、读取上限不合法或文件超出上限时，操作失败并进入 `catch`。打开成功后，读取某个文件时仍可能报错。

#### AIX.checkFromSource(files, options?)

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

检查以文件列表表示的项目源码。每个文件使用项目内的相对路径，根目录需要包含 `app.json`。检查会应用 `.aixignore`，不会生成包或运行项目代码。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 要检查的文件，结构为 `{ path, data }`。 |
| `options` | `CheckOptions` | 可选，通过 `onProgress` 接收检查步骤和分类结果。 |

**返回：** `Promise<CheckReport>`。源码问题放在 `diagnostics` 中；是否继续打包，应检查其中是否有 `severity: "error"`。检查包含项目结构、JS/TS 语法、页面参数说明和可识别的权限使用；不检查 TypeScript 类型和实际运行效果。

**错误处理：** 源码错误和警告会正常返回。路径、忽略规则等输入无法处理，或进度回调抛错时，进入 `catch`。

#### AIX.checkFromFiles(files, options?)

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

检查浏览器选择的项目文件。优先使用文件的 `webkitRelativePath`，没有时使用 `name`。所有文件来自同一个所选目录时，会去掉这一层目录名。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `File[]` | 浏览器文件数组，可通过 `Array.from(input.files ?? [])` 获取。 |
| `options` | `CheckOptions` | 可选的检查进度回调。 |

**返回：** `Promise<CheckReport>`，问题列表与 `checkFromSource()` 的返回结构相同。

**错误处理：** 源码问题通过 `diagnostics` 返回；文件读取失败或输入无法处理时，进入 `catch`。

#### AIX.packFromSource(files, options?)

~~~typescript
static packFromSource(
    files: AixInputFile[],
    options?: PackFromSourceOptions,
  ): Promise<PackResult>
~~~

打包项目源码，通常应选用本方法。它会整理项目内路径，应用 `.aixignore`，并排除规则文件和 `.aix/` 项目缓存。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 项目内文件列表。根目录必须有 `app.json`；不应包含已有包的生成信息文件。 |
| `options` | `PackFromSourceOptions` | 可选的打包设置和进度回调，默认值见下文。 |

**返回：** `Promise<PackResult>`。`data` 可以下载或上传，`warnings` 是非致命提醒，`report` 是文件大小变化报告。

**错误处理：** 文件路径重复、必需文件缺失、JSON 或脚本语法有误、打包选项无效等情况会进入 `catch`。进度回调抛错也会使操作失败。打包不会自动执行完整源码检查。

#### AIX.packFromFiles(files, options?)

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

把浏览器的 `File` 数组打包。优先使用 `webkitRelativePath`，没有时使用 `name`，并按这些路径组织文件。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `File[]` | 文件相对路径需要保证 `app.json` 位于项目根目录。 |
| `options` | `PackFromSourceOptions` | 与 `packFromSource()` 相同。 |

**返回：** `Promise<PackResult>`。

**错误处理：** 文件读取失败或打包失败时，进入 `catch`。本方法保留文件路径中的外层目录名；通过目录选择器打包完整项目时，请使用第 2 步的路径整理方式，再调用 `packFromSource()`。

#### AIX.pack(files, options?)

~~~typescript
static pack(
    files: AixInputFile[],
    options?: PackOptions,
  ): Promise<PackResult>
~~~

打包已经整理和筛选好的文件列表。本方法不应用 `.aixignore`；普通项目源码推荐使用 `packFromSource()`。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 已准备好的项目文件，根目录必须有 `app.json`。 |
| `options` | `PackOptions` | 可选的打包设置，不包含 `onProgress`。 |

**返回：** `Promise<PackResult>`。

**错误处理：** 输入文件不符合打包要求、配置或选项无效时，进入 `catch`。提醒通过结果中的 `warnings` 返回。

#### AIX.optimize(data, options?)

~~~typescript
static optimize(
    data: Uint8Array | File,
    options?: OptimizeOptions,
  ): Promise<PackResult>
~~~

优化一个已有的 `.aix` 文件，返回新的文件内容；原文件不会被修改。项目标识和构建标识会保留。已签名的包需要在优化后重新签名。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | 已有的 `.aix` 文件或文件内容。 |
| `options` | `OptimizeOptions` | 可选，默认使用 `level: 2` 并启用 JSON、PNG、JPEG 优化。 |

**返回：** `Promise<PackResult>`。保存 `data` 即可取得优化后的包。`report` 的字节数表示文件内容的大小，不是最终下载文件的体积。

**错误处理：** 包无法读取、文件损坏或选项无效时，进入 `catch`。

<span id="实例方法"></span>

### AIX 实例方法（pkg） {#aix-instance}

`AIX.From()` 返回的 `Promise<AIX>` 表示：等待操作完成后，得到一个 `AIX` 类型的包对象。
下面把这个对象命名为 `pkg`；`pkg` 是变量名，`AIX` 是它的类型，二者对应同一个包对象。

~~~javascript
const pkg = await AIX.From(file);
pkg.list();     // 列出这个包里的文件
pkg.getPages(); // 查看这个包里的页面
~~~

因此，`pkg.list()`、`pkg.readFile()` 等都是返回的 `AIX` 对象上的方法。实例方法同步返回，不需要 `await`。
标明“抛错”的情况可以用 `try/catch` 处理。

#### pkg.list()

~~~typescript
list(): AixEntry[]
~~~

列出包里的文件。

**返回：** `AixEntry[]`。每项包含 `name`、`size` 和 `compressed_size`，大小单位为字节。

**错误处理：** 本方法只列出文件信息；能列出文件，不代表它的内容一定可以成功读取。

#### pkg.readFile(name)

~~~typescript
readFile(name: string): Uint8Array
~~~

读取包内指定文件的内容。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `name` | `string` | 准确的包内文件名，建议使用 `list()` 返回的 `name`。 |

**返回：** `Uint8Array`。读取文字可用 `new TextDecoder().decode(data)`，读取图片等内容可交给 `Blob`。

**错误处理：** 文件不存在、内容损坏或读取时超出上限，会抛错。

#### pkg.getTitle()

~~~typescript
getTitle(): string | undefined
~~~

读取项目在 `app.json.window.navigationBarTitleText` 中填写的标题。

**返回：** `string | undefined`。标题未填写，或相关配置无法读取时，返回 `undefined`。

**错误处理：** 标题缺失或配置无效时，使用 `undefined` 表示没有可用标题。

#### pkg.getVersion()

~~~typescript
getVersion(): string | undefined
~~~

读取包的构建标识。它不是 `@yodaos-pkg/aix` 的 npm 版本号。

**返回：** `string | undefined`。构建信息缺失或无法读取时，返回 `undefined`。

**错误处理：** 无法取得有效构建信息时，使用 `undefined` 表示没有可用值。

#### pkg.getAgentId()

~~~typescript
getAgentId(): string | undefined
~~~

读取打包时设置的项目标识。

**返回：** `string | undefined`。没有设置标识的包返回 `undefined`。

**错误处理：** 已保存的标识或相关包信息无效时，会抛错。

#### pkg.getPermissions()

~~~typescript
getPermissions(): string[]
~~~

查看当前包在 `app.json.permissions` 中声明的权限名称。

~~~javascript
const permissions = pkg.getPermissions();
// 例如：["GEOLOCATION", "CAMERA"]
~~~

**返回：** `string[]`。按声明顺序返回，保留重复项和自定义权限名称。未声明权限或声明为空数组时，返回 `[]`。

**错误处理：** `app.json` 缺失、无法读取、格式无效，或 `permissions` 不是字符串数组时，会抛错。

这份列表表示项目声明需要哪些权限，不表示设备已经授权。调用此方法不会弹出授权提示。

#### pkg.getPages()

~~~typescript
getPages(): PageInfo[]
~~~

查看页面列表。每个页面包含名称、标题、说明、参数说明和显示尺寸。

**返回：** `PageInfo[]`。没有可用的页面列表时返回空数组；页面信息不完整时，部分字段可能采用默认值。

**错误处理：** 需要区分“没有页面”和“页面读取失败”时，使用 `resolveMetadata()` 查看诊断。

#### pkg.getWidgets(locale?)

~~~typescript
getWidgets(locale?: string): WidgetInfo[]
~~~

查看小组件列表及其展示文案。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `locale` | `string` | 可选，例如 `zh-CN`、`en-US`。不传时使用项目默认文案。 |

**返回：** `WidgetInfo[]`。指定语言的文案不存在时，使用默认文案；项目没有小组件时返回空数组。

**错误处理：** 小组件配置、已提供的语言文件无效，或声明的小组件文件不存在时，会抛错。

#### pkg.getTools()

~~~typescript
getTools(): Tool[]
~~~

查看提供给 AI 的工具定义。工具定义说明页面名称和可接收的参数，不会直接执行页面。

**返回：** `Tool[]`。每项含 `function`、`target` 和 `layout`。`function.name` 是页面路径，`function.parameters` 是该工具接受的参数说明。

**错误处理：** 页面读取行为与 `getPages()` 一致。需要定位读取问题时，使用 `resolveMetadata()`。

#### pkg.resolveMetadata()

~~~typescript
resolveMetadata(): MetadataResolution
~~~

一起读取页面和工具，并报告读取过程中发现的问题。

**返回：** `MetadataResolution`，包含 `pages`、`tools`、`diagnostics`。读取失败的页面会从前两个列表中排除，其余页面按项目中的顺序返回。每个失败页面返回一个问题，包含 `path` 和 `message`。

**错误处理：** 页面配置或文件读取问题通过 `diagnostics` 返回。小组件不在此报告范围内；本方法也不替代 `checkFromSource()` 或 `checkFromFiles()` 的源码检查。

#### pkg.supportsEngine(currentVersion)

~~~typescript
supportsEngine(currentVersion: string): boolean
~~~

判断包是否兼容某个运行版本。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `currentVersion` | `string` | 单个版本号，例如 `0.17.0`，不要传版本范围。 |

**返回：** `boolean`，兼容时为 `true`，不兼容时为 `false`。

**错误处理：** 包缺少有效的兼容版本信息，或传入的版本号无效时，会抛错。

### 可选参数和默认值

#### PackOptions / PackFromSourceOptions

| 字段 | 类型 | 默认值与约束 |
| --- | --- | --- |
| `buildId` | `string` | 自动生成。手动填写时不能为空；之后可用 `getVersion()` 取得。 |
| `agentId` | `string` | 默认不设置。只能包含小写英文字母、数字、`_`、`-`，不能以 `-` 开头或结尾，也不能为空。同一项目可复用相同的值。 |
| `engine` | `string` | 采用 `app.json.engine`，项目没有配置时使用 `*`。显式传入时覆盖项目配置，格式为版本范围，例如 `>=0.17.0`。 |
| `optimize` | `false` 或 `OptimizeOptions` | 默认不额外优化 JSON 和图片；传 `{}` 时启用下面的全部默认优化。独立 JS/TS 文件始终会进行代码压缩；这不会将 TypeScript 转为 JavaScript。 |
| `onProgress` | `(event: PackProgressEvent) => void` | 可选回调，仅 `PackFromSourceOptions` 提供。回调中可读取当前步骤和已处理文件的信息。 |

#### OptimizeOptions

| 字段 | 类型 | 默认值 |
| --- | --- | --- |
| `level` | `1`、`2` 或 `3` | `2`，优化级别。 |
| `json` | `boolean` | `true`，是否优化 JSON。 |
| `png` | `boolean` | `true`，是否优化 PNG 图片。 |
| `jpeg` | `boolean` | `true`，是否优化 JPEG 图片。 |

#### CheckOptions

`onProgress?: (event: CheckProgressEvent) => void` 接收检查步骤。
`check_finished` 事件中的 `check` 包含分类名称、状态、错误数和提醒数。
`status` 为 `passed`、`warning`、`failed` 或 `skipped`；后者表示这一类没有适用内容，或暂时无法检查。
这些是步骤提示，不是完成百分比。最终问题列表始终从返回值的 `diagnostics` 读取。

#### AixReadLimits

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `maxEntries` | `number` | 包内项目的数量上限，包含文件、目录等记录。 |
| `maxEntryBytes` | `number` | 单项内容的最大字节数。 |
| `maxTotalBytes` | `number` | 所有项内容的总字节数上限，不按重复读取次数累计。 |

不传的字段没有上限。数值必须是非负安全整数；`0` 表示不允许对应数量或大小，等于上限时允许读取。
无效数值和无法识别的字段会使打开操作失败。这些限制不用于 `AIX.optimize()`。

### 完整类型定义

下面只列出调用 API 时会用到的公开类型。可按需要展开。

<details>
<summary>文件输入与可选参数</summary>

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

interface PackFromSourceOptions extends PackOptions {
    onProgress?: (event: PackProgressEvent) => void;
}

interface OptimizeOptions {
    level?: 1 | 2 | 3;
    json?: boolean;
    png?: boolean;
    jpeg?: boolean;
}

interface CheckOptions {
    onProgress?: (event: CheckProgressEvent) => void;
}
~~~

</details>

<details>
<summary>检查结果和进度</summary>

~~~typescript
interface CheckReport {
    diagnostics: CheckDiagnostic[];
}

interface CheckDiagnostic {
    code: string;
    severity: 'error' | 'warning';
    path: string;
    line: number;
    column: number;
    message: string;
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

</details>

<details>
<summary>打包结果和进度</summary>

~~~typescript
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

</details>

<details>
<summary>包内文件、页面、小组件和工具</summary>

~~~typescript
interface AixEntry {
    name: string;
    size: number;
    compressed_size: number;
}

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

interface MetadataDiagnostic {
    path: string;
    message: string;
}

interface MetadataResolution {
    pages: PageInfo[];
    tools: Tool[];
    diagnostics: MetadataDiagnostic[];
}
~~~

</details>

<details>
<summary>JSON 和页面参数说明</summary>

~~~typescript
type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;

interface JsonObject {
    [key: string]: JsonValue;
}

type JsonSchema = JsonValue;
~~~

</details>

字段补充说明：

- `PageInfo.name` 是页面路径；`data_schema` 是页面接受的参数说明；`size` 是显示宽高。
- `WidgetInfo.path` 是小组件路径，`family` 是项目声明的尺寸类型，`placement` 表示常驻或浮层展示。
- `Tool.function` 是提供给 AI 的函数定义；`target` 是展示位置提示，`layout` 是显示宽高。
- `OptimizeReport` 和 `FileOptimizeReport` 的大小字段以字节为单位，`saved_bytes` 表示减少的大小；`converted_to_utf8` 表示是否调整过文本编码。
- 页面、小组件和工具的可空字段使用 `null`。这与 `getTitle()`、`getVersion()`、`getAgentId()` 在没有可用值时返回 `undefined` 不同。
- `JsonSchema` 保留读取到的 JSON 值。需要验证页面参数说明是否合法时，使用源码检查方法。
