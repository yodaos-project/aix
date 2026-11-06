# @yodaos-pkg/aix

`@yodaos-pkg/aix` は、AIX 成果物の読み取り、作成、最適化、検査を行うブラウザー向け
WebAssembly / TypeScript パッケージです。このリファレンスは v0.12.0 の公開 API に対応します。
7 つの静的メソッドは非同期、10 のインスタンスメソッドは同期です。
高水準 SDK は ES モジュール、fetch、WebAssembly を備えた最新ブラウザーと Web Worker 向けです。
npm CLI は独立した Node.js WASM ビルドを使用します。

## インストール

~~~bash
npm install @yodaos-pkg/aix
~~~

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

const response = await fetch('/agents/example.aix');
const aix = await AIX.From(new Uint8Array(await response.arrayBuffer()));
console.log(aix.getTitle(), aix.getPages(), aix.getTools());
~~~

読み取りは独立した reader WASM をロードします。パッケージ化、最適化、ソース検査は
初回呼び出し時に完全な tooling モジュールをロードします。自己ホスティングでは
`pkg-reader/` と `pkg/` の両方を、隣接する JS/WASM ファイルとともに配布してください。
既存の reader インスタンスは tooling ロード後も使用できます。
[WASM の読み込みと測定](/wasm?lang=en)も参照してください。

## API リファレンス

### AIX.From(data, limits?, check?) {#aix-from-data-limits}

~~~typescript
static From(data: Uint8Array | File, limits?: AixReadLimits, check?: boolean): Promise<AIX>
~~~

完全なアーカイブの `Uint8Array` またはブラウザーの `File` からインスタンスを作成します。
不正な ZIP、パス、正規化後の重複エントリー、または予算超過では Promise が reject されます。

第 3 引数 `check` の既定値は `false` です。`true` にすると、パッケージの内容に
`checkFromSource()` と同じソース検査を行ってからインスタンスを返します。警告だけなら
成功します。`severity: 'error'` がある場合は [AixCheckError](#aix-check-error) をスローし、
`report` に完全な `CheckReport`、`diagnostics` に警告も含めた全診断を保持します。
アーカイブの読み取り、展開、検査の読み込みや入力処理に失敗した場合は元の操作エラーを
スローします。その場合、検査レポートがあるとは限りません。

~~~typescript
import { AIX, AixCheckError } from '@yodaos-pkg/aix';

async function openCheckedPackage(file: File) {
  try {
    return await AIX.From(file, undefined, true);
  } catch (error) {
    if (error instanceof AixCheckError) console.table(error.diagnostics);
    throw error;
  }
}
~~~

検査はプロジェクトのコードを実行せず、実行時の動作は検証しません。検査を有効にすると
開く処理時間が増えますが、読み取り上限は引き続き適用されます。

~~~typescript
const aix = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
~~~

上限値と等しい場合は読み取り可能で、未知の予算フィールドは拒否されます。各値は非負の安全な整数で、0 はエントリーまたは
バイトを許可しません。ディレクトリーと署名メタデータも数えます。解凍前にメタデータを
検査し、`readFile` でも出力サイズ、CRC、サイズを検証します。`maxTotalBytes` は宣言された
非圧縮サイズの合計で、繰り返し読み取りの累積ではありません。省略した項目と `{}` は
無制限です。信頼できないパッケージには 3 項目すべてを設定してください。入力バッファー、
解析済みオブジェクト、呼び出し元のコピー、`AIX.optimize` はこの予算に含まれません。

Rust は `AixReader::new_with_limits` と `Option<u64>` の `max_entries`、`max_entry_bytes`、
`max_total_bytes` を使用します。直接 WASM を使う場合は `AixReaderWasm.with_limits(data, limits)`
に camelCase の項目を渡します。既存のコンストラクターは無制限のままです。

### AIX.checkFromSource(files, options?)

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

メモリー内のソースツリーを検査します。アーカイブやファイルを生成せず、プロジェクトコードを
実行しません。パスはアプリケーションルートからの相対パスで、`app.json` はルートに置きます。
ルートおよび入れ子の `.aixignore` を適用し、ルールファイルと任意の深さの `.aix/` を除外します。

対象は JSON/テキスト、App/Page/Widget 構造、コンポーネント参照と循環、JS/TS 構文
（`.mts`、`.cts`、Ink の `lang="ts"` を含む）、静的に認識できる権限使用、Page の
`schema.data` 宣言です。Schema は `$schema` に従い、既定は 2020-12 です。ローカル参照を
解決しますが、外部リソースは取得しません。実行時データ、TypeScript の型、import 解決、
コンパイラー設定、動的 API 呼び出し、実行時動作は検証しません。

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

ソースのエラーと警告は `CheckReport.diagnostics` に返り、Promise の reject にはなりません。
`severity === 'error'` を確認して続行を判断します。不正なパスや ignore ルールなどの収集エラー、
WASM ロード失敗、進捗コールバックの例外では reject されます。`CheckReport` に成功フラグや
チェックリスト配列はありません。分類別結果は `check_finished` イベントで取得します。
`AIX.check()` またはインスタンスの `check()` はありません。

### AIX.checkFromFiles(files, options?)

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

ブラウザーの `File` 配列を検査します。`webkitRelativePath`、なければ `name` を使用します。
全ファイルが選択ディレクトリーの共通プレフィックスを持つ場合、その外側のディレクトリーを
取り除いて `checkFromSource` に渡します。ファイル読み取り失敗では reject されます。

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

### 検査の進捗と応答性

`onProgress` は `loading_checker`、`checking_sources`（収集前の入力ファイル数）、
5 つの `check_finished`、正常終了時の `collecting_diagnostics` を通知します。
分類 ID は `json`、`structure`、`syntax`、`permissions`、`schemas` です。
各結果には状態、エラー/警告数、任意の `detail` があり、対象がない場合や入力が不正な場合に
`skipped` になることがあります。パーセント表示ではありません。初期化後の検査は同期処理です。
UI の応答性が必要な場合は Web Worker を使用してください。npm CLI も同じ検査エンジンを
Node.js Worker 内で使用します。[`aix check`](/cli?lang=ja)も参照してください。

### AIX.pack(files, options?)

~~~typescript
static pack(
    files: AixInputFile[],
    options?: PackOptions,
  ): Promise<PackResult>
~~~

正規化済みエントリーをパッケージ化します。`app.json` が必要で、`VERSION` と予約済みの
`META-INF/aix/` を含めてはいけません。このメソッドはソース収集や `.aixignore` を適用しません。
`buildId` は既定で UUID v4 を生成し、`VERSION` に書き込みます。`engine` は明示的な指定、
`app.json.engine`、`*` の順に解決します。JSON を検証し、対応テキストを UTF-8 に変換します。

`optimize` を省略または `false` にしても、独立した `.js` / `.ts` ファイルの縮小は実行されます。
TypeScript から JavaScript への変換ではありません。JSON/画像の追加最適化は省略/`false` で
無効、`{}` で `level: 2`、`json: true`、`png: true`、`jpeg: true` を適用します。
完全なソース検査は自動実行されません。`warnings` は致命的でないパッケージ化の診断です。

同じ `agentId` を指定すると Manifest の `agent_id` をビルド間で保持できます。SDK は ID や
ファイルシステム上の状態を自動生成しません。ID は空でなく、小文字 ASCII、数字、`_`、`-`
のみで、先頭と末尾の `-` は不可です。`buildId` とは独立し、最適化時にも保持されます。
直接 WASM の pack 関数では末尾の任意引数、Rust では `PackOptions.agent_id` を使用します。

### AIX.packFromSource(files, options?)

~~~typescript
static packFromSource(
    files: AixInputFile[],
    options?: PackFromSourceOptions,
  ): Promise<PackResult>
~~~

パスを正規化し、入れ子の `.aixignore` を適用してルールファイルと `.aix/` キャッシュを
除外します。正規化後のパス重複や不正なルールでは reject されます。収集済みの入力には
`pack` を使用できます。`onProgress` は本メソッドと `packFromFiles` にあり、`pack` や
`optimize` にはありません。コールバック例外でも呼び出しは失敗します。

### AIX.packFromFiles(files, options?)

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

`webkitRelativePath`、なければ `name` で File をエントリーへ変換し、`packFromSource` に
委譲します。`checkFromFiles` と異なり、外側のディレクトリーを保持します。
`webkitdirectory` で選択した場合は、`app.json` がルートになるようプレフィックスを除きます。

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
static optimize(
    data: Uint8Array | File,
    options?: OptimizeOptions,
  ): Promise<PackResult>
~~~

既存パッケージの最適化済みコピーを返します。`VERSION` と `agent_id` は保持し、元の署名は
削除します。省略したオプションは上記の最適化既定値を使います。レポートのバイト数は
非圧縮エントリーデータであり、ZIP ダウンロードサイズではありません。

### インスタンスメソッド

~~~typescript
list(): AixEntry[]
readFile(name: string): Uint8Array
getVersion(): string | undefined
getAgentId(): string | undefined
supportsEngine(currentVersion: string): boolean
getTitle(): string | undefined
getPermissions(): string[]
getPages(): PageInfo[]
resolveMetadata(): MetadataResolution
getWidgets(locale?: string): WidgetInfo[]
getTools(): Tool[]
~~~

| メソッド | 動作 |
| --- | --- |
| `list()` | 正規化された名前と圧縮前後のバイト数を返します。 |
| `readFile(name)` | 正確な名前のエントリーを読み、CRC/サイズを検証します。欠落、破損、予算超過では例外です。 |
| `getVersion()` | `VERSION` を読みます。欠落、読み取り不能、不正 UTF-8 は `undefined` です。 |
| `getAgentId()` | Manifest の `agent_id` を読みます。未設定は `undefined`、不正な Manifest/ID は例外です。 |
| `supportsEngine(version)` | Manifest の semver 範囲を検査します。Manifest/範囲の欠落や不正、不正なバージョンは例外です。 |
| `getTitle()` | `app.json.window.navigationBarTitleText` を読みます。欠落や不正設定は `undefined` です。 |
| `getPermissions()` | `app.json.permissions` の文字列配列を宣言順に返します。重複と独自の名前を保持し、省略時や空配列は `[]` です。`app.json` の欠落、読み取り失敗、不正形式、文字列配列以外の値では例外です。 |
| `getPages()` | 下記の寛容な方針でページメタデータを解決します。 |
| `resolveMetadata()` | 解決済みのページ/ツールとリソース診断を返します。 |
| `getWidgets(locale?)` | Widget の入口を検証し、`app.<locale>.json` の表示名と説明を適用します。不正設定/言語ファイルや入口欠落は例外です。 |
| `getTools()` | 寛容に解決したページから OpenAI 互換の関数レコードを生成します。 |

`getPermissions()` は宣言された権限を返します。実行時やデバイスの許可状態を照会せず、
許可を求めるダイアログも表示しません。

~~~typescript
const permissions = aix.getPermissions(); // 例：['GEOLOCATION', 'CAMERA']
~~~

同期メソッドでもリソース読み取り時に例外が発生します。既定の `AIX.From` は全エントリーを即時に
読みません。任意のソース検査を有効にしても、発行者の暗号署名は検証しません。署名検証は Rust reader にあり、TypeScript の
公開メソッドではありません。[仕様](/spec?lang=ja)も参照してください。

### AixCheckError {#aix-check-error}

`From` の任意の検査がエラーを報告した場合にスローされる、公開された `Error` のサブクラスです。
`instanceof` で判定するには `import type` ではなく通常の `import` で読み込みます。

| プロパティ | 型 | 意味 |
| --- | --- | --- |
| `name` | `string` | `"AixCheckError"`。 |
| `message` | `string` | 検査失敗の短い説明。 |
| `report` | `CheckReport` | 完全な検査レポート。 |
| `diagnostics` | `CheckDiagnostic[]` | `report.diagnostics` と同じ配列。警告とソース位置も保持します。 |

警告だけではスローしません。独立したレポートが必要なら `checkFromSource()` または
`checkFromFiles()` を使用します。

### メタデータ解決と診断

`getPages()` / `getTools()` は寛容な動作を保持します。不正なページ一覧では空配列を返し、
欠落リソースや不正なメタデータには既定値を使うことがあります。Page、Widget、タイトルの
設定は独立して解析されます。`resolveMetadata()` は `{ pages, tools, diagnostics }` を返し、
失敗したページを両方の一覧から除外します。ほかのページの宣言順は保持します。

~~~typescript
const result = aix.resolveMetadata();
for (const diagnostic of result.diagnostics) {
  console.error(diagnostic.path, diagnostic.message);
}
const tools = result.tools;
~~~

有効な `pages: []` の診断は空です。`app.json.pages` が欠落、読み取り不能、不正の場合は
`app.json` の診断を返します。失敗ページごとに最初のリソース/メタデータエラーを返し、
ツールのターゲットは解決済みページに対して通常の規則で決まります。

読み取り可能な `.ink`、または従来ページの `.json` と `.wxml` が必要です。存在するリソースは
CRC/サイズ検証と UTF-8 デコードを通過する必要があり、ページ JSON と完全な Ink
`<script def>` はデシリアライズ可能でなければなりません。壊れた `.ink` から従来形式には
フォールバックしません。任意の定義、Schema、スタイルは省略でき、`.wcss` は `.wxss` より優先です。
Widget とタイトルは対象外です。Ink 構造、コンポーネント、Schema の完全なソース検査や
実行時動作検証の代わりではありません。Rust は `AixReader::resolve_metadata()`、
直接 WASM は `reader.resolve_metadata()` を使用します。

## 型リファレンス

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

pack のコールバックは転送、ソース収集、Engine 解決、ファイル準備、各ファイル完了、
アーカイブ最終化を通知します。`file_finished` は生成された `VERSION` を含む各エントリーの
`FileOptimizeReport` を渡します。同期の段階通知であり、パーセント表示ではありません。

### ソース検査の型

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

### パッケージおよび派生モデル

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

Page の `size` と Tool の `layout` は幅/高さの数値です。Tool は現在 `type: "function"` と
`_current` / `_blank` を出力します。下線なしの名前は入力時の別名のみです。
`JsonSchema` は任意の JSON（真偽値や未検証の旧データを含む）を保持します。
Schema の検証にはソース検査を使い、オブジェクトのプロパティ参照前に型を絞り込んでください。

メタデータのフィールドは必須です。未設定文字列と入れ子の JSON null は、Rust と同様に
WASM、SDK、`resolveMetadata()` で `null` を保持します。旧 WASM の `undefined` と異なるため、
明示的な比較を移行してください。独立した `getTitle()` / `getVersion()` は引き続き
`string | undefined`、`getAgentId()` も未設定時に `undefined` を返します。

## ブラウザーでの使用例

~~~typescript
import { AIX } from '@yodaos-pkg/aix';

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

## Play

[Play](/play?lang=ja) では、成果物のタイトル、バージョン、ページ、ツール、エントリー、
生の内容を確認できます。

## ソースからのビルド

~~~bash
cd crates/aix-web
npm install
npm run build
~~~

配布物は `crates/aix-web/dist` に生成されます。アプリケーションでは公開パッケージを使用し、
自己ホスティング時は生成された分配ディレクトリー全体を配布してください。

## ライセンス

MIT
