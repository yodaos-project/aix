# AIX 格式规范

| 规范元数据 | 值 |
| --- | --- |
| 状态 | 实现草案规范 |
| 格式标识 | `aix` |
| 文档范围 | 包字节及其规范性解释 |

本文规定 AIX 文件格式，独立于编程语言、命令行接口、读取库、浏览器绑定和宿主 Agent Runtime。**MUST**、**MUST NOT**、**REQUIRED**、**SHALL**、**SHALL NOT**、**SHOULD**、**SHOULD NOT**、**RECOMMENDED**、**MAY** 和 **OPTIONAL** 按 RFC 2119 与 RFC 8174 解释。

AIX（AI eXecutable）是面向 AI Agent、具有可检查用户界面的可移植包格式。它是 [Open Agent Format（OAF）](https://openagentformat.com/spec.html) 的 ZIP 扩展：OAF 定义 Agent 身份、指令、组合关系和与 Harness 无关的资源；AIX 增加确定性归档、Ink Mini Program 页面、页面 Schema、布局提示以及面向 Agent 的工具表面。

AIX Producer 创建 `.aix` 归档，AIX Consumer 解析和使用归档。符合规范的 Producer/Consumer 必须满足各自要求。只有 ZIP 序列化和 AIX 必需数据模型通过本文验证规则时，包才是有效的；只有签名根据受信任策略验证通过后，包才是可信的，有效不代表可信。

## 0. 一致性语言

本文的规范关键词含义同 RFC 2119/8174。AIX Producer 创建归档，AIX Consumer 解析归档；符合规范的实现必须执行本文所有 MUST 规则。

## 0.1 格式版本

字符串 `aix` 标识格式族。`VERSION` 是构建产物标识而不是格式版本；Manifest 的 `format` 标识格式族。Consumer MUST 拒绝不支持的 `format`。Producer 增加字段时 MUST 保留已有语义；修改字段、路径规则、规范化哈希输入或签名域时，必须使用新的格式标识或协商 profile。

## 0.2 数据模型与字符编码

路径名和文本条目均为 UTF-8。ZIP 路径排序及 `package_id` 排序使用原始 UTF-8 字节，不使用本地化排序、Unicode 归一化或大小写折叠。JSON MUST 是 UTF-8 有效 JSON；OAF `AGENTS.md` front matter MUST 是 UTF-8 有效 YAML。计算摘要或签名时不得静默转码。

## 1. OAF 基线

OAF 将文件系统作为 Agent 的事实来源，最小结构为：

```text
agent/
└── AGENTS.md
```

根 `AGENTS.md` 是 Agent manifest 和指令文档，Markdown 正文是 prompt，可选 YAML front matter 承载身份和组合元数据。常见资源包括：

```text
agent/
├── AGENTS.md
├── skills/{name}/SKILL.md
├── mcp-configs/{name}/
├── sub-agents/{name}/
├── versions/{version}/
└── README.md, LICENSE
```

OAF manifest 可声明 `name`、`description`、`version`、`author`、`license`、`skills`、`packs`、`weblets`、`mcp-servers`、`sub-agents`、`model`、`memory` 和 Harness 配置。OAF 与 Harness 无关，也可使用 `PACKAGE.yaml` 封装 Agent 目录。

### OAF 与 AIX 的边界

AIX 原样携带 OAF 文件，不重新解释 Markdown 指令、Skill、MCP 配置或模型策略；这些文件仍由宿主 Harness 使用。AIX 只在 `app.json`、页面文件和 `META-INF/aix/` 中增加机器可读元数据。

| 关注点 | OAF | AIX |
| --- | --- | --- |
| 身份与指令 | `AGENTS.md` front matter + Markdown | 作为条目保留，由宿主解释 |
| Skills、MCP、子 Agent | 可选 OAF 目录 | 作为条目保留 |
| 包模型 | 目录或 `PACKAGE.yaml` | 规范化路径的 ZIP |
| 交互 UI | 未规定 | Ink 页面、模板、样式、Schema |
| Agent 表面 | Harness 定义 | 从页面派生 OpenAI 风格工具 |
| 完整性 | OAF 打包指导 | 可选 Ed25519 Manifest 和摘要 |

## 2. AIX 包

### 2.1 抽象包模型

AIX 是有序条目集合；每个条目有路径、未压缩字节序列和 ZIP 压缩表示。central directory 只是序列化索引：

```text
aix-package   = zip-archive
app-entry     = "app.json"
version-entry = "VERSION"
metadata-dir  = "META-INF/aix/"
page-path     = 1*(path-char)
path-char     = %x21-7E / UTF8-NONASCII
```

路径 MUST 非空、不得以 `/` 开头、不得含 `\`、空段、`.`、`..` 或 NUL；规范化后不得重复。应用输入不得以 `META-INF/aix/` 开头。

### 2.2 条目分类

| 类别 | 判断方式 | Manifest 处理 |
| --- | --- | --- |
| 应用 | `META-INF/aix/` 外的非目录路径 | MUST 列出并摘要 |
| 目录 | 路径以 `/` 结尾 | MAY 省略 |
| AIX 元数据 | 以 `META-INF/aix/` 开头 | MUST NOT 放入 `entries` |

保留命名空间可扩展；未由 profile 定义的元数据，应用 Consumer MUST 忽略。

### 2.3 ZIP 序列化要求

归档 MUST 使用 UTF-8 条目名，Consumer MUST 支持 `stored` 和 `deflate`。文本 SHOULD 使用 `deflate`，PNG/JPEG MAY 使用 `stored`。CRC-32 和未压缩大小必须与提取字节一致；大小或 CRC 校验失败时 MUST 拒绝。时间戳、创建者、权限和压缩级别不得影响 `package_id`。

### 2.4 必需条目处理

生成包 MUST 写入非空 UTF-8 `VERSION` 和 `app.json`；`VERSION` SHOULD 是 UUID v4 或其他全局唯一构建标识，Consumer 除 Manifest 相等校验外将其视为不透明值。解析页面或 Widget 前 MUST 先解析 `app.json`。

```text
VERSION                         # 构建标识
app.json                        # 应用元数据与页面列表
AGENTS.md                       # 提供时的 OAF manifest
pages/...                       # 页面资源
META-INF/aix/manifest.json      # 推荐的生成 Manifest
```

目录不是逻辑文件条目；`META-INF/aix/` 是排除在签名应用集合外的保留空间。JSON、JavaScript、Ink、模板和样式通常 deflate，PNG/JPEG 通常 stored。Collector 会规范化路径并拒绝路径穿越。

### 必需与约定条目

生成包需要 `VERSION` 和 `app.json`。未签名或旧包可省略 `META-INF/aix/manifest.json`；签名包必须包含第 7 节的 Manifest 和签名条目。

## 3. 应用元数据

`app.json` 要求 `pages` 为字符串数组；当前消费字段为：

```json
{
  "pages": ["pages/index/index", "pages/settings/index"],
  "widgets": [{"path":"widgets/clock/index","family":"1x1","placement":"persistent","displayName":"Clock","description":"Current time"}],
  "window": {"navigationBarTitleText":"Example agent"}
}
```

`pages` 是无扩展名逻辑路径且顺序有意义。Widget 的 `placement` 默认为 `persistent`，也可为 `overlay`；locale overlay 可替换 Widget 名称和描述。

## 4. 页面与 OAF 资源

每个页面支持：`{path}.json` + `{path}.wxml` + `{path}.wxss` 多文件形式，或包含 raw-text `page`/`template` 和 `style` 块的 `{path}.ink` 单文件形式。

```json
{
  "navigationBarTitleText":"Search",
  "description":"Search the knowledge base",
  "schema":{"data":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}
}
```

`schema.data` 原样成为 `data_schema`。缺少 Schema 或 `data` 时使用 `{}` 并产生警告；页面标题和描述可选。`PageInfo` 为 `name`、`title`、`description`、`data_schema` 和 `size {width,height}`。`AGENTS.md`、`skills/`、`mcp-configs/` 是普通可读条目，不会并入页面 Schema 或工具描述。

## 5. 布局约束

`PageAnalyzer` 识别 `.wxml`/`.wxss` 或 Ink 块中的 `width: Npx`、`height: Npx`。行内样式覆盖外部 `#id` 和 `.class` 规则。

- 宽度取根元素固定宽度最大值；
- 高度累加根元素固定高度；
- 缺失维度默认为 `480 × 168` 像素；
- 非像素值、嵌套布局和动态 CSS 不计算。

该尺寸是展示提示，宿主仍须处理响应式内容。

## 6. 页面到工具的派生

```json
{"type":"function","target":"_current","layout":{"width":480,"height":168},"function":{"name":"pages/search/index","description":"Search the knowledge base","parameters":{"type":"object","properties":{"query":{"type":"string"}}}}}
```

`function.name` 是逻辑页面路径，描述优先页面 `description` 再回退标题，参数严格等于 `schema.data`，格式不做 Schema 转换或验证。第一个声明页面若 Schema 为 null、`{}` 或空 `properties` 对象，则一次性使用 `_blank` 和 `{}`；否则及其余页面均为 `_current`。改变 `app.json.pages` 顺序可能改变初始工具目标。

## 7. Manifest、完整性与签名

```json
{"format":"aix","version":"VERSION_VALUE","engine":"^0.14.0","algorithm":"ed25519","digest":"sha256","key_id":"sha256:PUBLIC_KEY_DIGEST","package_id":"sha256:ENTRY_LIST_DIGEST","entries":[{"path":"app.json","size":42,"sha256":"HEX_DIGEST"}]}
```

Manifest `entries` 必须严格按 UTF-8 字节递增，排除 `META-INF/aix/*`，覆盖每个非目录、非元数据条目。`package_id` 对每条依次编码大端路径长度、路径字节、大端大小、大端摘要字符串长度和摘要字节，再计算 SHA-256。

```text
META-INF/aix/signature.ed25519   # 64 字节签名
META-INF/aix/public-key.ed25519  # 32 字节公钥
```

签名使用 `package-manifest` 上下文和 `AIX-SIGNATURE\0` 域前缀。验证必须检查算法、摘要、engine、受信任 `key_id`、签名、排序、每项摘要/大小、未签名应用条目、`version == VERSION` 和 package id。有效 ZIP 不自动可信。

## 8. Engine 兼容性

`engine` 是 semver requirement。裸 `0.14.0` 等于 `=0.14.0`；`^0.14.0`、`>=0.14.0`、`*` 按标准 semver 匹配；无效范围使包不符合规范。

## 9. 序列化与兼容性

Producer 必须按 UTF-8 字节排序应用路径；输入不得提供 `VERSION`。序列化后的 JSON、JavaScript、TypeScript、Ink、模板和样式必须 UTF-8。PNG/JPEG 可优化但不得改变逻辑路径或媒体类型。`.aixignore` 遵循 gitignore 语法且自身不打包。API 表面不属于本格式规范，OAF 资源序列化后保持不变。

## 10. 一致性清单

实现必须能够解析规范化 ZIP 并验证 CRC/大小，要求 `app.json.pages` 和 `VERSION`，解析多文件及 `.ink` 页面，保留 Schema，实现 `480 × 168`、最大宽度、高度累加及首页面 `_blank` 规则，保留 OAF 资源，拒绝无效 engine 或不匹配签名，并复现 Manifest 排序与 package-id 编码。

## 11. 字段定义

除非另有说明，未知字段保留为字节但忽略其语义。Producer SHOULD 使用规定名称和大小写，Consumer 不得从未知拼写推断值。

### 12.1 OAF `AGENTS.md` front matter

front matter 由 `---` 分隔。首个非空正文行以 `#` 开头时是结构化正文，否则是专用子 Agent prompt。

| 字段 | 类型 | 必需 | 约束与含义 |
| --- | --- | --- | --- |
| `name` | string | 是 | 展示名，1–100 字符。 |
| `vendorKey` | string | 是 | kebab-case 发布者命名空间。 |
| `agentKey` | string | 是 | kebab-case Agent 标识。 |
| `version` | string | 是 | 语义化版本。 |
| `slug` | string | 是 | 稳定标识，通常为 `vendorKey/agentKey`。 |
| `description` | string | 是 | 用途与能力摘要。 |
| `author` | string | 是 | 个人、组织或句柄。 |
| `license` | string | 是 | SPDX 或项目许可证。 |
| `tags` | string 数组 | 是 | 搜索和分类标签。 |
| `skills` / `packs` / `weblets` / `mcpServers` / `agents` | 对象数组 | 否 | Skill、集合、Weblet、MCP、子 Agent。 |
| `orchestration` | 对象 | 否 | 入口、回退和事件触发。 |
| `tools` | string 数组 | 否 | Harness 工具名。 |
| `config` / `memory` / `model` | 对象或 string | 否 | Runtime 策略、记忆和模型选择。 |
| `harnessConfig` | string 到对象 map | 否 | Harness 专用设置。 |

组合对象中的 Skill 使用 `name/source/version/required`，Pack 使用 `vendor/pack/version/required`，Weblet 使用 `vendor/weblet/version/launch`（`onDemand`、`background`、`foreground`），MCP 使用 `vendor/server/version/configDir/required`，Agent 引用使用 `vendor/agent/version/role/delegations/required`，Model 使用 `provider/name/embedding`，Memory 使用 `type/blocks`（`editable` 或 `read-only`）。AIX 不解析 URL、安装 Skill、启动 MCP 或执行模型策略。`config` 可包含 `temperature`、`max_tokens`、`require_confirmation` 以及 `tools.allowed/denied`。

### 12.2 AIX `app.json`

| 字段 | 类型 | 必需 | 默认值 | 约束与行为 |
| --- | --- | --- | --- | --- |
| `pages` | string 数组 | 是 | 无 | 非空逻辑路径，顺序有意义。 |
| `widgets` | Widget 数组 | 否 | `[]` | 解析时需匹配 `{path}.ink`。 |
| `window` | Window 对象 | 否 | 无 | 当前只消费 `navigationBarTitleText`。 |

Widget 字段：`path` 为必需无扩展名路径，`family` 为必需的宿主族，`placement` 可选且默认为 `persistent`（或 `overlay`），`displayName` 与 `description` 为可空字符串并可被 locale overlay 替换。Window 仅包含可选的 `navigationBarTitleText`。`app.{locale}.json` 的 `widgets` 以 Widget 路径为键；locale 先精确匹配，再逐步移除子标签，最后回退主语言。

### 12.3 页面 JSON 与 Schema

逻辑路径 `p` 优先读取 `p.ink`，否则读取 `p.json`、`p.wxml` 和 `p.wcss`/`p.wxss`。

| 字段 | 类型 | 必需 | 默认值 | 含义 |
| --- | --- | --- | --- | --- |
| `navigationBarTitleText` | string/null | 否 | null | 页面标题。 |
| `description` | string/null | 否 | null | 工具元数据描述。 |
| `schema` | object/null | 否 | 无 | 输入 Schema 容器。 |
| `schema.data` | 任意 JSON | 否 | `{}` | 原样作为工具参数。 |

只有首页面目标选择把 null、空对象和空 `properties` 当作无参数；其他值均保留。Ink 的 `script def` 提供 JSON，`page`/`template` 提供标记，`style` 提供 CSS。可选页面损坏可回退并诊断。

### 12.4 派生 `PageInfo` 字段

| 字段 | 类型 | 始终存在 | 含义 |
| --- | --- | --- | --- |
| `name` | string | 是 | `app.json.pages` 的精确路径。 |
| `title` | string/null | 是 | 页面标题。 |
| `description` | string/null | 是 | 页面描述。 |
| `data_schema` | JSON | 是 | `schema.data`，默认 `{}`。 |
| `size.width` | number | 是 | 最大固定像素宽度，默认 `480`。 |
| `size.height` | number | 是 | 固定像素高度总和，默认 `168`。 |

只检查根 XML 元素；外部选择器按 id、class 应用，行内声明覆盖外部声明；无 `px` 后缀的值忽略。

### 12.5 派生 Tool 字段

| 字段 | 类型 | 必需 | 值与行为 |
| --- | --- | --- | --- |
| `type` | string | 是 | `function`。 |
| `target` | enum | 是 | 首个无参数页面 `_blank`，其余 `_current`。 |
| `layout` | PageConstraint | 是 | 页面推导的 size。 |
| `function.name` | string | 是 | 逻辑页面路径。 |
| `function.description` | string/null | 是 | description，缺失回退 title。 |
| `function.parameters` | JSON | 是 | 精确 `schema.data`，首个 `_blank` 使用 `{}`。 |

`target` 是渲染指令，不是安全边界；宿主决定 `_blank` 的实际展示方式。

### 12.6 AIX Manifest 字段

| 字段 | 类型 | 必需 | 约束 |
| --- | --- | --- | --- |
| `format` | string | 是 | 必须为 `aix`。 |
| `version` | string | 是 | 必须等于 `VERSION`。 |
| `engine` | string | 是 | 有效 semver；裸版本精确匹配。 |
| `algorithm` | string | 是 | 当前为 `ed25519`。 |
| `digest` | string | 是 | 当前为 `sha256`。 |
| `key_id` | string | 是 | `sha256:` 加受信任公钥摘要；未签名为空。 |
| `package_id` | string | 是 | 规范条目序列摘要。 |
| `entries` | ManifestEntry 数组 | 是 | 严格字节排序，排除 `META-INF/aix/`。 |

ManifestEntry 的 `path` 为规范化路径，`size` 为无符号未压缩字节数，`sha256` 为未压缩内容的小写十六进制 SHA-256。规范 package id 编码 `u32-be(path.length)`、路径、`u64-be(size)`、`u32-be(sha256.length)` 和摘要字符串后计算 SHA-256。

### 12.7 签名记录

`signature.ed25519` 恰好 64 字节，`public-key.ed25519` 恰好 32 字节。签名是带 `package-manifest` 上下文的 Manifest JSON Ed25519 签名；消息以 `AIX-SIGNATURE\0`、大端上下文长度、上下文、大端消息长度和 Manifest 字节组成。内嵌公钥只是元数据，验证必须使用调用方提供的受信任密钥。

## 错误与兼容性要求

Reader 必须拒绝非法/重复路径、不支持压缩、CRC/大小失败、必需 JSON 或 Manifest 损坏、未排序条目、摘要/版本/package-id 不匹配、未签名应用条目、无效 engine 和不受信任 key id。可选页面失败可回退默认值。只有未知元数据字段向前兼容；字段语义、枚举拼写、路径规范化、签名域和哈希编码均是兼容契约。

## 13. 符合规范的处理模型

Consumer SHOULD 依次：解析并规范化 ZIP 路径；拒绝重复/非法路径、压缩和 CRC/大小错误；分类应用、目录、元数据；解析并验证 `app.json`；存在 Manifest 时验证类型、排序和覆盖范围；需要信任时先验证签名和摘要；最后解析页面、Widget、OAF、Schema 和布局并派生工具。步骤 1–5 失败使包无效；可选资源失败不得创建路径、Schema 属性或签名声明。

## 14. 安全注意事项

解压 MUST 受资源上限约束并防止路径穿越。签名只认证 Manifest 和列出的应用字节，不授权执行、网络、MCP、模型选择或 OAF 指令；这些属于宿主信任策略。应拒绝过多条目、过大解压、过深嵌套或过复杂 Schema。JSON Schema 是数据；OAF Markdown、Skill 和脚本在明确授权前是不可信内容。`public-key.ed25519` 不是信任根，`key_id` 必须与调用方密钥比较；密码库支持时应使用常量时间比较。

## 15. 互操作要求

相同规范化条目、构建标识、engine 范围和签名配置必须产生语义等价包：路径、Manifest 字段、摘要和 package id 相同，ZIP 时间戳和压缩方式可以不同。测试 SHOULD 覆盖未签名最小包、签名文本/二进制包、多文件和 Ink 页面、空 Schema/参数化首页、本地化 Widget fallback、非法/重复路径、缺少 VERSION、无效 app.json、Manifest 重排、未签名额外条目、字节或 VERSION 变化、错误密钥及 ZIP CRC/大小/压缩失败。

## 16. 完整最小示例

```text
VERSION                 = "550e8400-e29b-41d4-a716-446655440000"
app.json                = {"pages":["pages/index/index"]}
pages/index/index.json  = {
  "navigationBarTitleText":"首页",
  "schema":{"data":{"type":"object","properties":{}}}
}
pages/index/index.wxml  = "<view style=\"width: 480px; height: 168px\" />"
```

页面约束为 `480 × 168`。它是首个页面且对象 Schema 没有属性，因此派生工具使用空参数对象和 `_blank` 目标；该结论来自格式数据，不依赖特定 API 或 Runtime。
