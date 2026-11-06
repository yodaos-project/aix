# Web API 使用指南

`@yodaos-pkg/aix` 让你在网页中检查 AIX 项目、生成 `.aix` 文件，并读取包里的页面、工具和其他文件。

本文适用于在前端项目中接入 AIX 的开发者。示例使用 JavaScript：HTML 放到页面中，JavaScript 放到项目的入口脚本中。
如果你想先体验功能，可以直接打开 [Play](/play?lang=zh-CN)；如果你想在终端里操作项目，请看 [CLI 使用指南](/cli?lang=zh-CN)。

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
