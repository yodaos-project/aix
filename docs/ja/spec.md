# AIX 形式仕様

| 仕様メタデータ | 値 |
| --- | --- |
| 状態 | 実装ドラフト仕様 |
| 形式識別子 | `aix` |
| 文書の範囲 | パッケージのバイト列とその規範的解釈 |

本書は AIX ファイル形式を規定します。プログラミング言語、CLI、読み取りライブラリー、ブラウザーバインディング、ホスト Agent Runtime には依存しません。

## 0. 適合性を表す用語

本書の **MUST**、**MUST NOT**、**REQUIRED**、**SHALL**、**SHALL NOT**、**SHOULD**、**SHOULD NOT**、**RECOMMENDED**、**MAY**、**OPTIONAL** は、RFC 2119 および RFC 8174 に従って解釈します。

**AIX producer** は `.aix` アーカイブを作成し、**AIX consumer** はアーカイブを解析、利用します。適合 producer / consumer は、それぞれに課されたすべての要件を満たします。ZIP シリアライズと必須 AIX データモデルが本書の検証規則を満たす場合にパッケージは**有効**です。信頼ポリシーに対して署名を検証した後にのみ**信頼済み**となり、有効であることだけでは信頼を意味しません。

AIX（AI eXecutable）は、検査可能なユーザーインターフェースを備えた AI Agent 向けの移植可能なパッケージ形式です。[Open Agent Format（OAF）](https://openagentformat.com/spec.html) を ZIP ベースで拡張します。OAF は Agent の識別、指示、構成、Harness に依存しないリソースを定義し、AIX は決定的アーカイブ、Ink Mini Program ページ、ページ Schema、レイアウトヒント、Agent 向けツールサーフェスを追加します。

本書は `aiui-aix`、`aiui-aix-pack`、`aiui-aix-web` crate が実装する形式を説明します。**must** または **must not** と記載した箇所は規範的要件です。

## 0.1 形式のバージョン管理

文字列 `aix` は形式ファミリーを識別します。パッケージの `VERSION` エントリーは成果物またはビルドの識別子であり、形式バージョンではありません。Manifest の `format` フィールドが形式ファミリーを識別します。consumer は未対応の `format` を拒否しなければなりません。producer がフィールドを追加するときは既存フィールドの意味を保持しなければならず、既存フィールド、パス規則、正規ハッシュ入力、署名ドメインを変更する場合は、新しい形式識別子または明示的に合意された profile が必要です。

## 0.2 データモデルと文字エンコーディング

すべてのパス名とテキストエントリーは UTF-8 で符号化された Unicode スカラー値です。ZIP パスと package-id の順序は、ロケール照合や Unicode 正規化ではなく生の UTF-8 オクテットに基づきます。JSON は UTF-8 の有効な JSON、OAF `AGENTS.md` の front matter は UTF-8 の有効な YAML でなければなりません。digest または署名の計算中に JSON、YAML、Manifest のバイト列を暗黙に変換してはいけません。

## 1. OAF ベースライン

OAF はファイルシステムを Agent の信頼できる情報源として扱います。最小構成は次のとおりです。

```text
agent/
└── AGENTS.md
```

ルートの `AGENTS.md` は Agent の Manifest と指示文書です。Markdown 本文は Agent prompt、任意の YAML front matter は識別情報と構成メタデータです。OAF は自己完結した任意ディレクトリも定義します。

```text
agent/
├── AGENTS.md                 # 必須の OAF Manifest と指示
├── skills/{name}/SKILL.md    # AgentSkills 互換 Skill
├── mcp-configs/{name}/       # ActiveMCP.json と config.yaml
├── sub-agents/{name}/        # AGENTS.md を持つ入れ子の Agent
├── versions/{version}/       # 過去の Agent Manifest
└── README.md, LICENSE        # 人間向け情報と法的メタデータ
```

OAF Manifest は一般に `name`、`description`、`version`、`author`、`license`、`skills`、`packs`、`weblets`、`mcp-servers`、`sub-agents`、`model`、`memory`、Harness 固有設定を宣言します。OAF は Harness に依存せず、同じ標準 Manifest を保持したまま複数の Runtime に出力できます。1 つ以上の Agent ディレクトリを `PACKAGE.yaml` で包むこともできます。

### OAF と AIX の境界

AIX パッケージは OAF ファイルをそのまま格納できます。AIX は Markdown 指示言語、Skill 形式、MCP 設定、モデルポリシーを再解釈しません。これらはホスト Harness が使用するリソースです。AIX は `app.json`、ページファイル、`META-INF/aix/` 以下に機械可読なアプリケーションメタデータを追加します。

| 関心事 | OAF | AIX |
| --- | --- | --- |
| Agent の識別と指示 | `AGENTS.md` front matter + Markdown | パッケージエントリーとして保持し、ホストが解釈 |
| Skills、MCP、sub-agent | 任意の OAF ディレクトリ | パッケージエントリーとして保持 |
| ファイルシステム / パッケージモデル | ディレクトリまたは OAF `PACKAGE.yaml` | 正規化パスを持つ ZIP アーカイブ |
| 対話 UI | 規定なし | Ink ページ、テンプレート、スタイル、ページ Schema |
| Agent 呼び出しサーフェス | Harness が定義 | ページから OpenAI 形式のツールを生成 |
| 完全性 | OAF のパッケージ指針 | 任意の AIX Ed25519 Manifest とエントリー digest |

## 2. AIX パッケージ

### 2.1 抽象パッケージモデル

AIX パッケージは順序付きエントリー集合です。各エントリーはパス、非圧縮バイト列、ZIP 圧縮表現を持ちます。ZIP central directory はシリアライズ用インデックスであり、アプリケーションメタデータではありません。次の抽象文法は必須の名前を示します。

```text
aix-package   = zip-archive
app-entry     = "app.json"
version-entry = "VERSION"
metadata-dir  = "META-INF/aix/"
page-path     = 1*(path-char) ; ページ拡張子を除く論理パス
path-char     = %x21-7E / UTF8-NONASCII
```

エントリーパスは空であってはならず、`/` で始まってはならず、`\`、空セグメント、`.`、`..`、NUL を含んではなりません。ZIP パス正規化後に同一パスを持つエントリーが複数あってはいけません。producer はアプリケーション入力として渡された `META-INF/aix/` で始まるパスを拒否しなければなりません。

### 2.2 エントリー分類

| クラス | 判定 | Manifest での扱い |
| --- | --- | --- |
| アプリケーション | `META-INF/aix/` 外の非ディレクトリパス | 必ず列挙し、ハッシュ化する |
| ディレクトリ | `/` で終わるパス | Manifest から省略してよい |
| AIX メタデータ | `META-INF/aix/` で始まるパス | `entries` に含めてはならない |

予約メタデータ名前空間は拡張可能です。profile が定義しない未知のメタデータエントリーは、アプリケーション consumer が無視しなければなりません。

### 2.3 ZIP シリアライズ要件

アーカイブは UTF-8 名の ZIP エントリーを使用しなければなりません。consumer は `stored` と `deflate` をサポートしなければなりません。producer はテキストに `deflate` を使用するべきで、PNG や JPEG のように圧縮済みのメディアには `stored` を使用してもかまいません。ZIP CRC-32 と非圧縮サイズは抽出結果と一致しなければならず、宣言サイズを超える抽出または CRC / サイズ検証失敗を consumer は拒否しなければなりません。

タイムスタンプ、creator、権限、圧縮レベルなどの ZIP メタデータは AIX の意味モデルに含まれず、`package_id` に影響してはいけません。

### 2.4 必須エントリーの処理

生成パッケージでは producer が `VERSION` と `app.json` を出力しなければなりません。`VERSION` は空でない UTF-8 で、UUID v4 または別のグローバルに一意なビルド識別子であるべきです。consumer は Manifest との一致検証以外ではこの値を不透明な値として扱います。ページや Widget のメタデータを解釈する前に `app.json` を解析しなければなりません。

`.aix` は ZIP アーカイブです。パス区切りは `/` で、バイト順に比較します。producer は入力の `VERSION` を拒否し、ビルド ID から生成します。生成アーカイブは少なくとも次を含みます。

```text
VERSION                         # UTF-8 ビルド識別子
app.json                        # アプリケーションメタデータとページ一覧
AGENTS.md                       # 指定された場合の OAF Manifest
pages/...                       # ページリソース
META-INF/aix/manifest.json      # 推奨される生成 Manifest
```

ディレクトリは論理ファイルエントリーではありません。`META-INF/aix/` は AIX メタデータ用に予約され、署名対象のアプリケーションエントリー集合から除外されます。JSON、JavaScript、Ink、テンプレート、スタイルは通常 deflate、PNG と JPEG は通常 stored です。collector は入力パスを正規化し、不正なパスや traversal を拒否します。

### 必須エントリーと慣例エントリー

生成パッケージには `VERSION`、ページ検出には `app.json` が必要です。未署名または旧形式のアーカイブは `META-INF/aix/manifest.json` を省略できます。署名済みパッケージは第 7 節の Manifest と署名エントリーを含まなければなりません。

## 3. アプリケーションメタデータ

`app.json` の `pages` は文字列配列でなければなりません。現在 AIX が使用する任意フィールドは次のとおりです。

```json
{
  "pages": ["pages/index/index", "pages/settings/index"],
  "widgets": [{
    "path": "widgets/clock/index",
    "family": "1x1",
    "placement": "persistent",
    "displayName": "Clock",
    "description": "Current time"
  }],
  "window": { "navigationBarTitleText": "Example agent" }
}
```

`pages` は拡張子を除く論理パスです。`window.navigationBarTitleText` はアプリケーションタイトルです。Widget の `placement` は既定で `persistent`、任意で `overlay` です。ロケールオーバーレイは Widget の `displayName` と `description` を置換できます。

## 4. ページと OAF リソース

`app.json.pages` の各パスについて、AIX は次のいずれかでメタデータを解決します。

1. **複数ファイルページ:** `{path}.json`、`{path}.wxml`、`{path}.wxss`。
2. **単一ファイルコンポーネント:** raw-text の `page` または `template` と `style` を含む `{path}.ink`。

ページ JSON の例です。

```json
{
  "navigationBarTitleText": "Search",
  "description": "Search the knowledge base",
  "schema": { "data": {
    "type": "object",
    "properties": { "query": { "type": "string" } },
    "required": ["query"]
  }}
}
```

`schema.data` はページの `data_schema` としてそのまま使用します。Schema または `data` がない場合は空の Schema と警告になり、アーカイブ自体は読み取り可能です。`PageInfo` は `name`、`title`、`description`、`data_schema`、`size { width, height }` です。

`AGENTS.md`、`skills/`、`mcp-configs/` などの OAF ファイルは通常の読み取り可能なエントリーとして残ります。AIX はその指示をページ Schema やツール記述へ統合しません。

## 5. レイアウト制約

`PageAnalyzer` はページの markup と style にある固定 pixel 宣言から `PageConstraint` を生成します。`.wxml` / `.wxss` または対応する Ink block の `width: Npx` と `height: Npx` を認識します。inline style は外部の `#id` と `.class` 規則を上書きします。

- 幅はルート要素にある固定幅の最大値です。
- 高さはルート要素にある固定高の合計です。
- 指定がない寸法は既定の `480 × 168` pixel です。
- pixel 以外の値、入れ子のレイアウト意味論、動的 CSS は評価しません。

結果のサイズは助言的です。ホストは viewport やツール表示の選択に利用できますが、responsive content にも対応する必要があります。

## 6. ページからツールへの生成

各ページは OpenAI 形式の function tool として表現できます。

```json
{
  "type": "function",
  "target": "_current",
  "layout": { "width": 480, "height": 168 },
  "function": {
    "name": "pages/search/index",
    "description": "Search the knowledge base",
    "parameters": { "type": "object", "properties": { "query": { "type": "string" } } }
  }
}
```

function 名は論理ページパスです。description はページ `description`、次に title を優先します。parameters は `schema.data` そのもので、形式側では Schema の変換や検証を行いません。

最初に宣言されたページには特別な起動規則があります。Schema が null、`{}`、または properties のない object Schema なら、空 parameters と `target: "_blank"` で 1 回出力します。それ以外は `_current` です。後続ページは parameter の有無にかかわらず `_current` です。この規則は位置依存で、`app.json.pages` の順序変更により最初の tool target が変わる場合があります。

## 7. Manifest、完全性、署名

生成される `META-INF/aix/manifest.json` の形は次のとおりです。

```json
{
  "format": "aix",
  "version": "VERSION_VALUE",
  "engine": "^0.14.0",
  "algorithm": "ed25519",
  "digest": "sha256",
  "key_id": "sha256:PUBLIC_KEY_DIGEST",
  "package_id": "sha256:ENTRY_LIST_DIGEST",
  "entries": [{ "path": "app.json", "size": 42, "sha256": "HEX_DIGEST" }]
}
```

Manifest の entries は UTF-8 バイト順で厳密に昇順で、`META-INF/aix/*` を除外し、ディレクトリでもメタデータでもないすべての ZIP エントリーを含まなければなりません。`package_id` は、各エントリーの big-endian パス長、パス bytes、big-endian size、big-endian digest 文字列長、digest bytes / 文字列表現をエントリー順にハッシュします。このバイト配置は互換性に関わります。

署名鍵が指定された場合、producer は次も書き込みます。

```text
META-INF/aix/signature.ed25519   # 64-byte Ed25519 署名
META-INF/aix/public-key.ed25519  # 32-byte 公開鍵
```

署名対象は context `package-manifest`、domain prefix `AIX-SIGNATURE\0` を付けた Manifest です。検証では algorithm / digest、engine 範囲、信頼済み `key_id`、署名、エントリー順、各 digest と size、未署名アプリケーションエントリーがないこと、`manifest.version == VERSION`、計算済み package id を確認します。有効な ZIP でも、これらが成功するまでは信頼済み AIX パッケージではありません。

## 8. Engine 互換性

Manifest の `engine` は semver requirement です。`0.14.0` のような裸の version は `=0.14.0`、`^0.14.0`、`>=0.14.0`、`*` は通常の semver matching で解釈します。不正な範囲を持つパッケージは非適合です。

## 9. シリアライズと互換性

producer はアーカイブと Manifest の書き込み前にアプリケーションパスを UTF-8 バイト列で sort しなければなりません。`VERSION` は build id から生成し、入力パッケージが独自の `VERSION` を指定してはいけません。JSON、JavaScript、TypeScript、Ink、template、stylesheet はシリアライズ後に UTF-8 でなければなりません。PNG と JPEG は最適化できますが、論理パスや media type を変更してはいけません。`.aixignore` は gitignore 構文に従い、自身はパッケージ化しません。

これらはバイトレベルの相互運用性を定義します。ツールや Runtime が公開する API は本仕様の範囲外です。OAF の指示とリソースは AIX へのシリアライズ時も変更しません。

## 10. 適合性チェックリスト

AIX 実装は次を実行できる場合に適合します。

- 正規化されたパスを持つ ZIP を解析し、size / CRC を検証する。
- 生成パッケージに `app.json.pages` と `VERSION` を要求する。
- 複数ファイルページと `.ink` 単一ファイルページを解決する。
- parameter を暗黙に作らず、ページ Schema data を保持する。
- 既定 `480 × 168`、最大幅、高さ合計のレイアウト algorithm を実装する。
- 最初のページの `_blank` 規則を正確に適用する。
- OAF リソースを AIX metadata と解釈せず保持する。
- 不正な engine 範囲、未署名または不一致の署名エントリーを拒否する。
- Manifest の順序と package-id のバイト配置を再現する。

## 11. フィールド定義

特記がない限り、未知フィールドはバイトとして保持しますが、適合 AIX consumer は無視します。producer はここに示す名前と大小文字を使用し、consumer は未知の綴りから値を推測してはいけません。

### 12.1 OAF `AGENTS.md` front matter

front matter は `---` で区切る YAML です。OAF は構造化本文または直接の system prompt 本文を受け付けます。最初の空でない本文行が `#` で始まる場合は構造化形式、それ以外は特化 sub-agent の prompt として解釈します。

| フィールド | 型 | 必須 | 制約と意味 |
| --- | --- | --- | --- |
| `name` | string | yes | 表示名、1～100 文字。 |
| `vendorKey` | string | yes | publisher namespace、kebab-case。 |
| `agentKey` | string | yes | Agent 識別子、kebab-case。 |
| `version` | string | yes | semantic version。 |
| `slug` | string | yes | 安定識別子。慣例として `vendorKey/agentKey`。 |
| `description` | string | yes | 目的と機能の短い説明。 |
| `author` | string | yes | 人、組織、handle。 |
| `license` | string | yes | SPDX 識別子またはプロジェクト定義の名前。 |
| `tags` | string 配列 | yes | 検索と分類用 label。 |
| `skills` | Skill 配列 | no | local、registry、well-known Skill 参照。 |
| `packs` | Pack 配列 | no | Skill または tool の集合。 |
| `weblets` | Weblet 配列 | no | Web ベースの tool または interface。 |
| `mcpServers` | MCP 配列 | no | MCP server 宣言。 |
| `agents` | AgentRef 配列 | no | nested / sub-agent 宣言。 |
| `orchestration` | Orchestration | no | entrypoint、fallback、event trigger。 |
| `tools` | string 配列 | no | Harness が利用できる明示的な tool 名。 |
| `config` | Configuration | no | Runtime 制限と allow / deny policy。 |
| `memory` | Memory | no | memory mode と named block。 |
| `model` | string または Model | no | Harness alias または provider / model 選択。 |
| `harnessConfig` | string から object の map | no | Harness 名を key とする provider 固有設定。 |

OAF の構成フィールドは説明的で Harness に依存しません。AIX は通常のアーカイブエントリーとして保存し、URL 解決、Skill のインストール、MCP server 起動、model / config policy の適用は行いません。

#### OAF 構成オブジェクト

| オブジェクト | フィールド | 意味 |
| --- | --- | --- |
| Skill | `name`、`source`、`version`、`required` | `source: local` は `./skills/{name}/`、URL は remote Skill。`required` の既定値は `false`。 |
| Pack | `vendor`、`pack`、`version`、`required` | vendor が提供する名前付き集合。 |
| Weblet | `vendor`、`weblet`、`version`、`launch` | `launch` は `onDemand`、`background`、`foreground`。既定値は Harness が決定。 |
| MCP server | `vendor`、`server`、`version`、`configDir`、`required` | `configDir` は `ActiveMCP.json` と `config.yaml` を含むディレクトリ。 |
| Agent reference | `vendor`、`agent`、`version`、`role`、`delegations`、`required` | `delegations` は責務 label の配列。 |
| Model | `provider`、`name`、`embedding` | provider と model の識別子。`embedding` は任意。 |
| Memory | `type`、`blocks` | `type` は `editable` または `read-only`。`blocks` は名前から初期値または template への map。 |

OAF `config` は `temperature`、`max_tokens`、`require_confirmation`、入れ子の `tools.allowed` / `tools.denied` を含められます。OAF は特定の YAML parser や順序を要求しませんが、producer は有効な YAML と UTF-8 を出力しなければなりません。

### 12.2 AIX `app.json`

`app.json` は UTF-8 JSON で、生成されるすべての AIX パッケージに必要です。

| フィールド | 型 | 必須 | 既定値 | 制約と consumer の動作 |
| --- | --- | --- | --- | --- |
| `pages` | string 配列 | yes | none | 空でない論理ページパス。順序に意味がある。 |
| `widgets` | Widget 配列 | no | `[]` | Widget metadata 解決時に一致する `{path}.ink` が必要。 |
| `window` | Window | no | absent | 現在は `navigationBarTitleText` だけを使用。 |

`pages` は存在し、宣言された形でなければなりません。consumer は欠落または不正なフィールドを持つパッケージを拒否でき、ページパスを捏造してはいけません。

#### Widget オブジェクト

| フィールド | 型 | 必須 | 既定値 | 制約と consumer の動作 |
| --- | --- | --- | --- | --- |
| `path` | string | yes | none | 拡張子なしの論理パス。metadata には `{path}.ink` が必要。 |
| `family` | string | yes | none | `1x1`、`1x2` などホスト定義の family。 |
| `placement` | enum string | no | `persistent` | `persistent` または `overlay`。JSON は kebab-case。 |
| `displayName` | string または null | no | null | ユーザー向け label。locale overlay で置換可能。 |
| `description` | string または null | no | null | 短い説明。locale overlay で置換可能。 |

#### Window オブジェクト

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `navigationBarTitleText` | string または null | no | アプリケーションタイトル。 |

`app.{locale}.json` は widget path を key とする `widgets` map を含められます。locale matching は `_` を `-` に正規化し、大小文字を無視し、完全一致から subtag を順に除去し、同じ primary language へ fallback します。

### 12.3 ページ JSON と Schema

論理パス `p` について consumer はまず `p.ink`、なければ `p.json` を読みます。`p.wxml` と `p.wcss` または `p.wxss` はレイアウト入力です。

| フィールド | 型 | 必須 | 既定値 | 意味 |
| --- | --- | --- | --- | --- |
| `navigationBarTitleText` | string または null | no | null | ページタイトル。 |
| `description` | string または null | no | null | tool metadata 用の自然言語説明。 |
| `schema` | object または null | no | absent | input Schema の container。 |
| `schema.data` | 任意の JSON 値 | no | `{}` | tool parameters として渡す JSON Schema または互換値。 |

`schema.data` が正式な JSON Schema である必要はありません。null、空 object、`properties` が空の object Schema は、最初のページの target 選択時だけ「parameter なし」と扱います。配列、文字列、数値、空でない object は parameter 付き Schema です。

Ink ファイルは raw-text block を使用します。`script def` に同じページ JSON、`page` または `template` に markup、`style` に CSS を記述します。任意ページファイルが不正でも ZIP は無効にならず、consumer は既定値を保持して diagnostic を出せます。

### 12.4 派生 `PageInfo` フィールド

| フィールド | 型 | 常に存在 | 意味 |
| --- | --- | --- | --- |
| `name` | string | yes | `app.json.pages` の正確な論理パス。 |
| `title` | string または null | yes | 宣言されている場合のページタイトル。 |
| `description` | string または null | yes | 宣言されている場合の説明。 |
| `data_schema` | JSON 値 | yes | `schema.data`。既定値 `{}`。 |
| `size.width` | number | yes | 固定 pixel 幅の最大値。既定値 `480`。 |
| `size.height` | number | yes | 固定 pixel 高の合計。既定値 `168`。 |

size はルート XML 要素だけを検査します。外部 selector は id、次に class の順で適用し、inline 宣言が上書きします。幅は最大値、高さは合計値です。`px` suffix のない CSS 値は無視します。

### 12.5 派生 Tool フィールド

| フィールド | 型 | 必須 | 値と動作 |
| --- | --- | --- | --- |
| `type` | string | yes | `function`。 |
| `target` | enum string | yes | 最初の parameter なしページは `_blank`、それ以外は `_current`。 |
| `layout` | PageConstraint | yes | ページの推定 `size` をコピー。 |
| `function.name` | string | yes | 論理ページパス。 |
| `function.description` | string または null | yes | ページ description。なければ title。 |
| `function.parameters` | JSON 値 | yes | 正確な `schema.data`。最初の `_blank` tool だけ `{}`。 |

`target` は描画指示でありセキュリティ境界ではありません。`_blank` を新しい surface、分離 context、初期 navigation のいずれとして扱うかはホストが決めます。

### 12.6 AIX Manifest フィールド

| フィールド | 型 | 必須 | 制約 |
| --- | --- | --- | --- |
| `format` | string | yes | `aix` と一致。 |
| `version` | string | yes | UTF-8 の `VERSION` 内容と一致。 |
| `engine` | string | yes | 有効な semver requirement。裸の version は完全一致。 |
| `algorithm` | string | yes | 現在は `ed25519`。 |
| `digest` | string | yes | 現在は `sha256`。 |
| `key_id` | string | yes | `sha256:` + 信頼済み公開鍵の小文字 hex SHA-256。未署名なら空。 |
| `package_id` | string | yes | `sha256:` + 正規エントリー列の小文字 hex digest。 |
| `entries` | ManifestEntry 配列 | yes | UTF-8 バイト順で厳密に sort。すべての `META-INF/aix/` を除外。 |

#### Manifest エントリー

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `path` | string | yes | 正規化済み ZIP パス。 |
| `size` | unsigned integer | yes | 非圧縮 byte length。 |
| `sha256` | 小文字 hex string | yes | 非圧縮エントリー bytes の SHA-256。 |

正規 package id は各エントリーを `u32-be(path.length)`、path bytes、`u64-be(size)`、`u32-be(sha256.length)`、digest string bytes の順で符号化し、連結結果を SHA-256 でハッシュします。locale、Unicode 照合、大小文字を無視した順序で sort してはいけません。

### 12.7 署名レコード

`signature.ed25519` は正確に 64 bytes、`public-key.ed25519` は正確に 32 bytes です。署名は context `package-manifest` を使った Manifest JSON bytes の Ed25519 です。domain-separated message は `AIX-SIGNATURE\0` bytes、big-endian context length、context bytes、big-endian message length、Manifest bytes の順です。verifier は caller が渡した信頼済み key を使用しなければなりません。埋め込み公開鍵は metadata であり、自動的な信頼判断ではありません。

## 12. エラーと互換性の要件

reader は不正な ZIP パス、正規化後の重複、未対応圧縮、CRC / size 検証失敗、不正な必須 JSON、不正な Manifest JSON、未整列の署名エントリー、digest 不一致、未署名アプリケーションエントリー、version 不一致、package-id 不一致、不正な engine 範囲、信頼されない key id を拒否しなければなりません。任意ページメタデータの失敗は既定値に縮退できます。

未知のメタデータフィールドだけが forward-compatible です。既存フィールドの意味、enum の綴り、パス正規化、署名 domain、hash encoding は互換性契約であり、形式 version または移行規則なしに変更してはいけません。

## 13. 適合処理モデル

consumer は次の順序で処理するべきです。

1. ZIP central directory を解析し、すべてのエントリーパスを正規化する。
2. 重複、不正パス、未対応圧縮、ZIP size / CRC 不一致を拒否する。
3. エントリーを application、directory、reserved metadata に分類する。
4. `app.json` を解析し、必須フィールドを検証する。
5. Manifest があれば、型、順序、entry coverage を検証する。
6. 信頼が必要なら、content の実行や表示前に署名とすべての Manifest digest を検証する。
7. page、widget、OAF resource、Schema、layout hint を解決する。
8. source bytes を変更せず presentation または tool record を生成する。

手順 1～5 の失敗はパッケージを無効にします。任意ページリソースの解決失敗は diagnostic と既定値にできる一方、パス、Schema property、署名 claim を捏造してはいけません。

## 14. セキュリティ上の考慮事項

ZIP の展開は宣言値と実装のリソース上限で制限しなければなりません。consumer は path traversal を防ぎ、選択したパッケージディレクトリ外へ書き込んではなりません。署名は Manifest と列挙された application bytes を認証しますが、実行、network access、MCP 利用、model 選択、OAF 指示を許可するものではありません。これらはホストの trust policy が決定します。

consumer は過剰な entry 数、展開後 size、nesting、Schema complexity を持つパッケージを、無制限なリソース割り当て前に拒否するべきです。JSON Schema は data であり実行コードではありません。OAF Markdown、Skill、script はホストが明示的に許可するまで信頼できない content として扱います。

埋め込み `public-key.ed25519` は識別子と配布補助であり trust root ではありません。verifier は `key_id` を caller 提供の trusted key と比較し、不一致を拒否しなければなりません。暗号ライブラリーが提供する場合、署名と digest の比較には constant-time 処理を使うべきです。

## 15. 相互運用性要件

同じ正規化 application entries、build-id、engine range、signing configuration を与えられた 2 つの適合 producer は、同じ entry path、Manifest fields、entry digest、package id を持つ意味的に同等な package を生成しなければなりません。ZIP timestamp と compression choice は異なってもかまいません。

相互運用テストには次を含めるべきです。

- 未署名の最小パッケージ。
- text と binary entry を 1 つずつ持つ署名済みパッケージ。
- 複数ファイルページと Ink 単一ファイルページ。
- 空 Schema の最初のページと parameter 付きの最初のページ。
- locale fallback を持つ Widget metadata。
- 不正パス、正規化後の重複、`VERSION` 欠落、不正な `app.json`。
- Manifest entry の順序変更、未署名追加 entry、変更された byte、変更された `VERSION`、誤った trusted key。
- ZIP CRC、size、未対応 compression の失敗。

## 16. 完全な最小例

次は最小限で実用的な未署名パッケージモデルです。ZIP は次の application entries だけを含みます。未署名パッケージでは Manifest は任意です。

```text
VERSION                 = "550e8400-e29b-41d4-a716-446655440000"
app.json                = {"pages":["pages/index/index"]}
pages/index/index.json  = {
  "navigationBarTitleText": "Home",
  "schema": {"data": {"type": "object", "properties": {}}}
}
pages/index/index.wxml  = "<view style=\"width: 480px; height: 168px\" />"
```

ページの layout constraint は `480 × 168` です。最初のページで object Schema に property がないため、派生 tool record は空の parameter object と `_blank` target を持ちます。この結果は形式 data から決まり、特定の API や Runtime を必要としません。
