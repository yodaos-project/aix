---
outline: [2, 4]
---

# AIX Web SDK {#yodaos-pkg-aix}

Web アプリで `@yodaos-pkg/aix` を使うと、AI エージェントのプロジェクトを検査し、`.aix` ファイルを作成したり、既存のパッケージを読み取ったりできます。AIX パッケージは、エージェントのファイルとメタデータをまとめた ZIP アーカイブです。

コードを書かずに試すには [Play](/ja/play) を使います。ターミナルで操作する場合は [CLI ガイド](/ja/cli) を参照してください。メソッドの引数や型は [API リファレンス](#api-reference) で確認できます。

## クイックスタート {#quick-start}

<div id="インストール"></div>

### SDK をインストールする {#installation}

フロントエンドプロジェクトで次のコマンドを実行します。

~~~bash
npm install @yodaos-pkg/aix
~~~

### 最初のパッケージを作成して開く {#build-and-open-your-first-package}

以下の JavaScript をアプリのモジュールスクリプトに追加します。最小構成のプロジェクトを作成し、検査してパッケージにまとめ、そのパッケージを開きます。

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

各ソースファイルには、プロジェクトのルートからの相対パス `path` と、`Uint8Array` 形式のデータ `data` が必要です。テキストは `TextEncoder` でバイト列に変換できます。パッケージのバイト列は `result.data` に入ります。`AIX.From()` で開いた後は、`pkg.getTitle()` や `pkg.list()` がすぐに結果を返します。

## プロジェクトを検査してパッケージ化する {#check-and-build-a-project}

### ソースファイルを検査する {#aix-checkfromsource-files-options}

プロジェクト内のパスとファイルのバイト列が手元にある場合は、`AIX.checkFromSource(files)` を使います。ファイル一覧のルートに `app.json` を含めます。

戻り値の `diagnostics` には、見つかった問題が入ります。各項目には重大度（`error` または `warning`）、説明、ファイルのパス、行、列が含まれます。配列が空なら、今回の検査で問題は見つかりませんでした。ファイルの読み込みや収集、検査機能の読み込み、進捗コールバックに失敗した場合は、Promise が拒否されます。

::: details 検査の対象
JSON とテキストの文字コード、アプリやページ、ウィジェットの構成、コンポーネント参照、JavaScript と TypeScript の構文、静的に判別できる権限の利用、ページデータのスキーマ宣言を検査します。`.mts`、`.cts`、および `lang="ts"` を指定した `.ink` のスクリプトブロックにも対応しています。

スキーマ検査は既定で JSON Schema 2020-12 を使用し、ローカル参照を解決します。型は TypeScript の型チェックで、実際の動作は AIUI 上で確認してください。各段階の通知については[検査の進捗コールバック](#check-progress-and-browser-responsiveness)を参照してください。
:::

### ブラウザーで選択したフォルダーを検査する {#aix-checkfromfiles-files-options}

ユーザーからプロジェクトを受け取る場合は、フォルダー選択欄を使えます。

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

`app.json` が入っているフォルダーを選択します。`checkFromFiles()` はブラウザーが渡す相対パスから、全ファイルに共通する最上位のフォルダー名を取り除きます。相対パスがない場合は、ファイル名を使います。

### ソースからパッケージ化する {#aix-packfromsource-files-options}

プロジェクトのパッケージ化には `AIX.packFromSource(files)` を使います。パスを正規化し、ルートと各サブフォルダーの `.aixignore` を適用します。除外ルールのファイルと `.aix/` キャッシュはパッケージから除外されます。パスや除外ルールが不正な場合や、正規化後のパスが重複する場合は、処理が失敗します。

パッケージ化では、アーカイブの作成に必要なファイルを確認します。ソースの検査結果も必要な場合は、クイックスタートの例のように先に `checkFromSource()` を実行してください。

| フィールド | 内容 |
| --- | --- |
| `data` | 生成された `.aix` ファイルのバイト列。 |
| `warnings` | パッケージ化を中断しなかった警告。 |
| `report` | 処理前と処理後のファイルサイズ。ZIP 圧縮前のバイト数で比較します。 |
| `progressiveAssets` | 各メディアの配信方式と、パッケージに残った元データのバイト数。 |

ブラウザーでダウンロードするには、`downloadPackage(result.data, 'my-agent.aix')` を呼び出します。

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

### ブラウザーのファイルをパッケージ化する {#aix-packfromfiles-files-options}

ブラウザーから受け取ったファイルのパスがすでにプロジェクトのルートを基準としている場合は、`AIX.packFromFiles(files)` を使います。パスには `webkitRelativePath` を使い、それがなければ `name` を使います。

フォルダー選択欄では、通常 `my-agent/app.json` のようなパスが返ります。`packFromFiles()` は最上位のフォルダー名を保持します。この場合は、例のように先頭のフォルダー名を取り除いてから `packFromSource()` に渡します。

~~~javascript
// Call after the user selects the project folder.
const files = await Promise.all(Array.from(input.files, async file => ({
  path: file.webkitRelativePath.split('/').slice(1).join('/'),
  data: new Uint8Array(await file.arrayBuffer()),
})));
const result = await AIX.packFromSource(files);
downloadPackage(result.data, 'my-agent.aix');
~~~

### 準備済みのファイル一覧をパッケージ化する {#aix-pack-files-options}

対象ファイルの選別とパスの正規化が済んでいる場合は、`AIX.pack(files)` にそのまま渡せます。AIX に除外ルールを適用させる場合は `packFromSource()` を使います。

すべてのパッケージ化メソッドで、ルートに `app.json` が必要です。`VERSION` と `META-INF/aix/` 配下のメタデータは AIX が作成するため、入力ファイル一覧から除外してください。

### ビルドオプションを設定する {#set-build-options}

オプションはパッケージ化メソッドの第 2 引数に渡します。例えば `AIX.packFromSource(files, { agentId: 'my-agent', optimize: {} })` は、プロジェクト ID を設定し、最適化を有効にします。

| オプション | 用途 |
| --- | --- |
| `agentId` | マニフェストに保存するプロジェクト ID。同じプロジェクトのビルドで同じ値を使います。 |
| `buildId` | ビルドの `VERSION`。既定では新しい UUID を生成します。 |
| `engine` | 対応する AIUI のバージョン範囲。省略時は `app.json.engine`、それもなければ全バージョンを許可する `*` を使用します。 |
| `optimize` | `{}` を渡すと JSON、PNG、JPEG をレベル 2 で最適化します。`level` は 1、2、3 から選べ、形式ごとに無効化できます。 |
| `onProgress` | `packFromSource()` と `packFromFiles()` のビルド進捗を受け取ります。 |
| `onProgressiveAsset` | [段階配信アセット](#progressive-assets)ごとに配信方式を選びます。 |
| `signal` | `AbortSignal` でビルドを取り消します。 |

`agentId` は、ASCII の小文字、数字、`_`、`-` からなる空でない文字列です。先頭や末尾に `-` は使えません。安定したプロジェクト ID が必要な場合は、呼び出し側で指定してください。

`optimize` を省略するか `false` にすると、JSON と画像のオプションの最適化は無効になります。JSON の検証、対応テキストの UTF-8 への変換、単独の JavaScript と TypeScript ファイルの圧縮は引き続き行われます。TypeScript ファイルは TypeScript のままです。

## 既存のパッケージを開く {#aix-from-data-limits}

`AIX.From()` は、ブラウザーの `File` またはパッケージ全体の `Uint8Array` を受け取ります。メソッド名の **F** は大文字です。

<div id="ブラウザーでの使用例"></div>

### ローカルファイルを開く {#browser-example}

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

### URL からパッケージを開く {#open-a-package-from-a-url}

~~~javascript
const response = await fetch('/agents/example.aix');
if (!response.ok) throw new Error(`Download failed (${response.status})`);

const pkg = await AIX.From(new Uint8Array(await response.arrayBuffer()));
const text = new TextDecoder().decode(pkg.readFile('app.json'));
console.log(JSON.parse(text));
~~~

`pkg.readFile()` は、指定したエントリのバイト列を返します。JSON やスクリプトはテキストに変換します。画像などのバイナリファイルは、バイト列のまま使えます。

### 開くときにソースを検査する {#aix-check-error}

第 3 引数に `true` を渡すと、パッケージを開く前にソースを検査します。警告だけならパッケージを開けます。エラーが見つかると、検査結果の `report` と `diagnostics` 配列を持つ `AixCheckError` が発生します。

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

通常の `AIX.From(file)` はアーカイブの構造を確認し、個別のファイルは必要になったときに読み込みます。アーカイブが不正な場合や展開に失敗した場合は、それぞれのエラーが発生します。発行者の署名は Rust のリーダーで検証できます。

<div id="メタデータ解決と診断"></div>

### ページとツールの情報を取得する {#metadata-resolution-and-diagnostics}

`pkg.getPages()` はページのメタデータを、`pkg.getTools()` はページから生成した OpenAI 互換のツール定義を返します。ページの情報が不足している場合は、既定値が使われることがあります。

どのページを読み取れたか確認するには、`pkg.resolveMetadata()` を使います。`pages`、`tools`、`diagnostics` が返ります。読み取りに失敗したページは診断に記録され、ページとツールの一覧から除外されます。

::: details ページを読み取る条件
ページには、読み取れる `.ink` ファイルか、`.json` と `.wxml` の両方が必要です。`.ink` ファイルが壊れていると、そのページはエラーになります。任意の定義、スキーマ、スタイルは省略できます。スタイルシートが両方ある場合は `.wcss` を優先します。

`app.json.pages` がないか不正な場合は、`app.json` の診断が返ります。空のページ一覧は有効です。読み取りに成功したページの順序は宣言順で、ツールの対象は通常のルールに従います。ウィジェットとタイトルのメタデータは別に読み取ります。
:::

## Progressive Assets（段階配信アセット） {#progressive-assets}

段階配信アセットを使うと、AIUI はメディアを小さな断片に分けて再生できます。リモート配信では、再生開始に必要な情報をパッケージ内に保存し、AIUI が必要な断片をダウンロードします。

JavaScript モジュールまたは `.ink` ファイルのスクリプトでメディアを宣言します。インポートした値は `aix-asset:<id>` のような `AssetRef` です。AIUI の動画または音声コンポーネントに渡します。

~~~javascript
import clip from './assets/clip.mp4' with { type: 'video', progressive: 'prefer' };
~~~

音声には `type: 'audio'` を使います。現在のプロファイルは H.264 の MP4 と AAC-LC の M4A に対応しています。条件を満たす通常のファイルは、再エンコードせずにフラグメント化された MP4 に変換できます。

### 断片の保存先を選ぶ {#choose-where-the-pieces-are-stored}

既定では元のメディアをパッケージに含めます。リモート配信するには、`packFromSource()` に `onProgressiveAsset` コールバックを渡します。

1. メディアをパッケージに含める場合は `{ delivery: 'embedded' }` を返します。
2. リモート配信する場合は `prepare()` を呼び出し、`plan.remoteObjects` を利用するストレージにアップロードします。
3. プラン ID とアップロード結果を返します。AIX が結果を検証してからパッケージを書き出します。

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

`uploadToStorage(plan, signal)` は呼び出し側で実装します。すべてのアップロードが成功した後に `ProgressiveAssetUploadReceipt` を返します。例えば、以下のように結果を組み立てられます。`uploadObject` は利用するストレージに合わせて実装し、バイト列をアップロードして、アクセス可能な HTTPS URL を返してください。

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

| フィールド | 設定する値 |
| --- | --- |
| `uploadId` | 今回のアップロードを表す空でない識別子。例ではローカルで生成していますが、サービスから取得しても構いません。 |
| `assetVersion` | `prepare()` が返した `plan.assetVersion` をそのまま使います。 |
| `objects` | `plan.remoteObjects` の各オブジェクトに対応する項目。重複や不足がないようにします。 |
| `objects[].id`、`size`、`sha256` | 対応する準備済みオブジェクトの値をそのまま使います。`size` の単位はバイトです。 |
| `objects[].url` | アップロードしたオブジェクトの完全で安定した HTTPS URL。認証情報、クエリ文字列、フラグメントは使えません。 |

キャンセルに対応できるよう、`signal` をアップロード関数に渡します。AIX はパッケージを書き出す前に、結果の識別子、サイズ、ハッシュ、URL の形式を検証します。アップロードの失敗、無効な結果、キャンセルが発生すると、ビルドは中断されます。

### パッケージ内で断片の URL を確認する {#find-the-urls-in-the-package}

リモート配信を選ぶと、AIX は `META-INF/assets/progressive_assets/index.json` にメディアの索引を保存します。索引は `aix-asset:<id>` の ID を `asset.json` に対応付けます。各断片の完全な HTTPS URL は、記述ファイルの `locators` に保存されます。

再生時に AIUI は `layout.fragments[n].object` からオブジェクト ID を取得し、`locators[objectId].url` を読み取ります。必要な断片を取得し、サイズと SHA-256 を検証します。初期化用の `init.mp4` はパッケージから読み込みます。詳しくは[パッケージ仕様](/ja/spec#_7-1-段階配信メディアのメタデータ)を参照してください。

### Play でビルドを試す {#play}

ビルド前に `AIX.listProgressiveAssets(files)` を呼び出すと、メディアのパス、種類、元のサイズ、形式の対応状況が分かります。`AIX.pack()` と同様に、対象ファイルを選別した一覧を渡します。

[Play のビルド画面](/ja/play?mode=build) でプロジェクトフォルダーを選ぶと、メディアの一覧が表示されます。**作成してダウンロード**を選ぶと、対応ファイルの断片が作成され、アップロードがシミュレートされます。ダウンロードしたパッケージを**検査**画面で開くと、保存された仮の URL を確認できます。

## 既存のパッケージを最適化する {#aix-optimize-data-options}

`AIX.optimize()` に `File` またはパッケージのバイト列を渡すと、最適化したコピーが作成されます。パッケージ化の例にあるダウンロード関数を再利用できます。

~~~javascript
const result = await AIX.optimize(file, { level: 2 });
downloadPackage(result.data, 'my-agent-optimized.aix');
~~~

オプションを省略すると、既定の JSON、PNG、JPEG の最適化が有効になります。コピーには `VERSION` と `agent_id` が引き継がれます。アーカイブの内容が変わる可能性があるため、元の署名は削除されます。

<div id="api-リファレンス"></div>

## API リファレンス {#api-reference}

[静的メソッド](#static-methods) · [インスタンスメソッド](#instance-methods) ·
[オプションとコールバック](#options-and-callbacks) · [型](#type-reference)

`?` は省略できる引数を表します。括弧内が空のメソッドには引数がありません。`Promise` を返すメソッドは、`await` で結果を待てます。

### 静的メソッド {#static-methods}

インポートした `AIX` クラスから呼び出します。

| メソッド | Promise の結果 |
| --- | --- |
| [`AIX.From(data, limits?, check?)`](#aix-from) | 開いた `AIX` パッケージ |
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

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | 完全な `.aix` のバイト列、またはブラウザーのファイル。URL から開く場合は先にダウンロードします。 |
| `limits` | `AixReadLimits` | 任意の[アーカイブ読み込み上限](#limit-archive-reads)。 |
| `check` | `boolean` | 既定値は `false`。`true` を渡すと、展開したソースファイルを検査してから結果を返します。 |

**戻り値**は、[インスタンスメソッド](#instance-methods)を持つパッケージに解決される Promise です。

**エラー**には、不正なアーカイブ、上限超過、読み込みの失敗が含まれます。`check` が `true` で検査エラーがあると、[`AixCheckError`](#reference-aix-check-error) で拒否されます。警告だけなら開けます。

#### AIX.checkFromSource(files, options?) {#aix-check-from-source}

~~~typescript
static checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | プロジェクト内の相対パス `path` と `Uint8Array` の `data` を持つファイル一覧。ルートに `app.json` を含めます。 |
| `options` | `CheckOptions` | 任意の進捗通知設定。 |
| `options.onProgress` | `(event: CheckProgressEvent) => void` | [検査の段階と結果](#check-progress-and-browser-responsiveness)を受け取ります。 |

プロジェクトの `.aixignore` を適用し、残ったソースファイルを検査します。

**戻り値**は、[`CheckReport`](#source-check-types) に解決される Promise です。ソースのエラーと警告はどちらも `diagnostics` に含まれます。問題が見つからなければ配列は空です。

**エラー**として、ファイルの収集、検査機能の読み込み、コールバックの実行が失敗すると、Promise が拒否されます。

#### AIX.checkFromFiles(files, options?) {#aix-check-from-files}

~~~typescript
static checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `files` | `File[]` | ブラウザーのファイル。`webkitRelativePath` を優先し、なければ `name` を使います。フォルダーを選択した場合は共通の最上位フォルダー名を取り除きます。 |
| `options` | `CheckOptions` | 任意の進捗通知設定。 |
| `options.onProgress` | `(event: CheckProgressEvent) => void` | [検査の段階と結果](#check-progress-and-browser-responsiveness)を受け取ります。 |

**戻り値**は、[`CheckReport`](#source-check-types) に解決される Promise です。`diagnostics` にソースのエラーと警告が入ります。

**エラー**として、ファイルを読み取れない場合やコールバックが例外を投げた場合は、Promise が拒否されます。

#### AIX.packFromSource(files, options?) {#aix-pack-from-source}

~~~typescript
static packFromSource(files: AixInputFile[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | プロジェクトのパスとバイト列。ルートに `app.json` を含めます。パスを正規化し、ルートとサブフォルダーの `.aixignore` を適用して、ルールファイルと `.aix/` キャッシュを除外します。 |
| `options` | `PackFromSourceOptions` | 任意の[ビルド設定](#pack-options)。 |
| `options.onProgress` | `(event: PackProgressEvent) => void` | [ビルドの段階とファイルごとの結果](#build-progress-callback)を受け取ります。 |
| `options.onProgressiveAsset` | [段階配信アセットのコールバック](#progressive-asset-callback) | `ProgressiveAssetCandidate` と `ProgressiveAssetPackContext` を受け取り、`ProgressiveAssetDecision` またはその Promise を返します。 |

**戻り値**は、[`PackResult`](#pack-result) に解決される Promise です。`data` はアーカイブ、`report` はファイルサイズ、`warnings` は警告、`progressiveAssets` はメディアの配信結果です。

**エラー**として、無効なパスや除外ルール、正規化後のパスの重複、コールバックの失敗、キャンセルが発生すると、Promise が拒否されます。

#### AIX.packFromFiles(files, options?) {#aix-pack-from-files}

~~~typescript
static packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `files` | `File[]` | ブラウザーのファイル。`webkitRelativePath` を優先し、なければ `name` を使います。このパスでパッケージ化したときに、`app.json` がルートに来る必要があります。 |
| `options` | `PackFromSourceOptions` | 任意の[ビルド設定](#pack-options)。 |
| `options.onProgress` | `(event: PackProgressEvent) => void` | [ビルドの段階とファイルごとの結果](#build-progress-callback)を受け取ります。 |
| `options.onProgressiveAsset` | [段階配信アセットのコールバック](#progressive-asset-callback) | `ProgressiveAssetCandidate` と `ProgressiveAssetPackContext` を受け取り、`ProgressiveAssetDecision` またはその Promise を返します。 |

ブラウザーが渡すパスの最上位フォルダー名は保持されます。取り除く必要がある場合は、[フォルダー選択の例](#aix-packfromfiles-files-options)のように変換してから `packFromSource()` を呼び出します。

**戻り値**は、アーカイブのバイト列、サイズのレポート、警告、配信結果を持つ [`PackResult`](#pack-result) に解決される Promise です。

**エラー**として、ファイルの読み込みやビルドに失敗すると、Promise が拒否されます。

#### AIX.pack(files, options?) {#aix-pack}

~~~typescript
static pack(files: AixInputFile[], options?: PackOptions): Promise<PackResult>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 選別済みで、パスを正規化したファイル一覧。ルートに `app.json` を含めます。除外ルールは呼び出す前に適用します。 |
| `options` | `PackOptions` | 任意の[ビルド設定](#pack-options)。 |
| `options.onProgressiveAsset` | [段階配信アセットのコールバック](#progressive-asset-callback) | `ProgressiveAssetCandidate` と `ProgressiveAssetPackContext` を受け取り、`ProgressiveAssetDecision` またはその Promise を返します。 |

`VERSION` とパッケージマニフェストは AIX が生成します。入力ファイル一覧から `VERSION` と予約済みのメタデータパスを除外してください。

**戻り値**は、アーカイブのバイト列、サイズのレポート、警告、配信結果を持つ [`PackResult`](#pack-result) に解決される Promise です。

**エラー**として、入力が無効な場合やコールバックの失敗、キャンセルが発生した場合は、Promise が拒否されます。

#### AIX.listProgressiveAssets(files, options?) {#aix-list-progressive-assets}

~~~typescript
static listProgressiveAssets(
  files: AixInputFile[],
  options?: { signal?: AbortSignal },
): Promise<ProgressiveAssetCandidate[]>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `files` | `AixInputFile[]` | 選別し、パスを正規化したファイル一覧。`pack()` と同じ形式です。 |
| `options` | `{ signal?: AbortSignal }` | 任意の検索設定。 |
| `options.signal` | `AbortSignal` | アセットの検索を取り消します。 |

**戻り値**は、[`ProgressiveAssetCandidate`](#progressive-asset-types) の配列に解決される Promise です。各項目にソースパス、メディアの種類、バイト数、参照元、リモート配信の対応状況が含まれます。アセットの宣言がなければ空の配列を返します。

**エラー**として、宣言が無効な場合、ソースファイルが見つからない場合、キャンセルした場合は、Promise が拒否されます。

#### AIX.optimize(data, options?) {#aix-optimize}

~~~typescript
static optimize(data: Uint8Array | File, options?: OptimizeOptions): Promise<PackResult>
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `data` | `Uint8Array \| File` | 既存の `.aix` パッケージ。 |
| `options` | `OptimizeOptions` | 任意の[最適化設定](#optimize-options)。省略すると既定値を使います。 |

**戻り値**は、最適化したコピーの [`PackResult`](#pack-result) に解決される Promise です。`data` に新しいアーカイブ、`report` にサイズの比較が入り、`progressiveAssets` は空です。コピーは `VERSION` と `agent_id` を保持し、既存の署名は削除します。

**エラー**として、アーカイブが不正な場合や、読み込みや最適化が失敗した場合は、Promise が拒否されます。

<div id="インスタンスメソッド"></div>

### インスタンスメソッド {#instance-methods}

`AIX.From()` が返したパッケージに対して呼び出します。いずれも同期的に結果を返します。

| 用途 | メソッド |
| --- | --- |
| アーカイブ内のファイル | [`list()`](#pkg-list)、[`readFile()`](#pkg-read-file) |
| 識別情報と互換性 | [`getVersion()`](#pkg-get-version)、[`getAgentId()`](#pkg-get-agent-id)、[`supportsEngine()`](#pkg-supports-engine) |
| アプリ設定 | [`getTitle()`](#pkg-get-title)、[`getPermissions()`](#pkg-get-permissions) |
| ページ、ウィジェット、ツール | [`getPages()`](#pkg-get-pages)、[`resolveMetadata()`](#pkg-resolve-metadata)、[`getWidgets()`](#pkg-get-widgets)、[`getTools()`](#pkg-get-tools) |

#### list() {#pkg-list}

~~~typescript
list(): AixEntry[]
~~~

**引数**はありません。

**戻り値**は、アーカイブの各エントリを表す `AixEntry` の配列です。各項目には正規化した `name`、圧縮前の `size`、圧縮後の `compressed_size` が含まれます。サイズはバイト単位です。

#### readFile(name) {#pkg-read-file}

~~~typescript
readFile(name: string): Uint8Array
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `name` | `string` | `app.json` など、正規化済みの正確なエントリ名。利用できる名前は [`list()`](#pkg-list) で確認できます。 |

**戻り値**は、展開したエントリのバイト列を表す `Uint8Array` です。

**エラー**には、エントリの欠落や破損、未対応の圧縮方式、読み込み上限超過が含まれます。

#### getVersion() {#pkg-get-version}

~~~typescript
getVersion(): string | undefined
~~~

**引数**はありません。

**戻り値**は、`VERSION` に保存されたビルド ID です。エントリがない、読み取れない、または UTF-8 として無効な場合は `undefined` を返します。

#### getAgentId() {#pkg-get-agent-id}

~~~typescript
getAgentId(): string | undefined
~~~

**引数**はありません。

**戻り値**は、マニフェストのプロジェクト ID `agent_id` です。ID がなければ `undefined` を返します。

**エラー**には、マニフェストを読み取れない場合、マニフェストが不正な場合、ID が無効な場合が含まれます。

#### supportsEngine(currentVersion) {#pkg-supports-engine}

~~~typescript
supportsEngine(currentVersion: string): boolean
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `currentVersion` | `string` | `0.18.0` など、具体的な AIUI のバージョン。 |

**戻り値**は、バージョンがマニフェストの `engine` の範囲内なら `true`、範囲外なら `false` です。

**エラー**には、マニフェストの欠落や不正、バージョン範囲や指定したバージョンの不正が含まれます。

#### getTitle() {#pkg-get-title}

~~~typescript
getTitle(): string | undefined
~~~

**引数**はありません。

**戻り値**は、`app.json.window.navigationBarTitleText` の文字列です。タイトルがない場合や設定を読み取れない場合は、`undefined` を返します。

#### getPermissions() {#pkg-get-permissions}

~~~typescript
getPermissions(): string[]
~~~

**引数**はありません。

**戻り値**は、プロジェクトが宣言した権限の配列です。宣言順、重複、独自の名前はそのまま保持します。権限が宣言されていないか空の場合は `[]` を返します。

**エラー**には、`app.json` を読み取れない場合や内容が不正な場合、`permissions` が文字列の配列でない場合が含まれます。

#### getPages() {#pkg-get-pages}

~~~typescript
getPages(): PageInfo[]
~~~

**引数**はありません。

**戻り値**は、宣言順に並んだページの配列です。ページのファイルが欠けている場合や不正な場合は、既定値を使うことがあります。ページ一覧を読み取れないか不正な場合は `[]` を返します。診断を確認するには [`resolveMetadata()`](#pkg-resolve-metadata) を使います。

#### resolveMetadata() {#pkg-resolve-metadata}

~~~typescript
resolveMetadata(): MetadataResolution
~~~

**引数**はありません。

**戻り値**は `{ pages, tools, diagnostics }` です。ページとツールの一覧には、読み取りに成功したページだけが含まれます。失敗したページは、ファイルパスとメッセージを持つ診断になります。ページ一覧を読み取れないか不正な場合は、`app.json` の診断を返します。

#### getWidgets(locale?) {#pkg-get-widgets}

~~~typescript
getWidgets(locale?: string): WidgetInfo[]
~~~

| 引数 | 型 | 説明 |
| --- | --- | --- |
| `locale` | `string` | `en-US` などの任意のロケール。該当する `app.<locale>.json` でウィジェット名と説明を上書きできます。上書きがなければ `app.json` の値を使います。 |

**戻り値**は、宣言されたウィジェットの配列です。宣言がなければ `[]` を返します。

**エラー**には、不正な設定、不正なロケールファイル、ウィジェットのエントリファイルの欠落が含まれます。

#### getTools() {#pkg-get-tools}

~~~typescript
getTools(): Tool[]
~~~

**引数**はありません。

**戻り値**は、ページ一覧から生成した OpenAI 互換のツール定義です。ページの既定値は `getPages()` と同じです。読み取りに成功したページのツールだけが必要な場合は、`resolveMetadata().tools` を使います。

### オプションとコールバック {#options-and-callbacks}

#### アーカイブの読み込み上限 {#limit-archive-reads}

`AixReadLimits` のフィールドはすべて省略できます。各値は 0 以上の安全な整数にします。上限と同じ値までは許可されます。`0` を指定すると、そのフィールドのエントリやバイトを許可しません。省略したフィールドには上限がありません。未定義のフィールドがあると `AIX.From()` が拒否されます。

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `maxEntries` | `number` | フォルダーや署名を含むアーカイブエントリ数の上限。 |
| `maxEntryBytes` | `number` | エントリ 1 件の展開後のサイズ上限。 |
| `maxTotalBytes` | `number` | 各エントリが宣言する展開後のバイト数の合計上限。同じエントリを繰り返し読んでも加算されません。 |

~~~javascript
const pkg = await AIX.From(file, {
  maxEntries: 1024,
  maxEntryBytes: 8 * 1024 * 1024,
  maxTotalBytes: 32 * 1024 * 1024,
});
~~~

::: details 上限とメモリ使用量
上限の対象はアーカイブ内のエントリです。入力バッファ、解析後のオブジェクト、呼び出し側が保持するコピー、ソース検査や最適化が使うメモリは、この上限には含まれません。

Rust では `AixReader::new_with_limits()` を使います。WASM のリーダーを直接呼び出す場合は `AixReaderWasm.with_limits(data, limits)` を使います。
:::

<div id="検査の進捗と応答性"></div>

#### 検査の進捗コールバック {#check-progress-and-browser-responsiveness}

~~~typescript
onProgress?: (event: CheckProgressEvent) => void
~~~

`checkFromSource()` または `checkFromFiles()` の `CheckOptions` に設定します。SDK はコールバックを同期的に呼び出し、イベントを 1 件渡します。戻り値は使いません。例外を投げると検査の Promise が拒否されます。

| `event.type` | 追加のフィールド | 発生するタイミング |
| --- | --- | --- |
| `loading_checker` | なし | SDK が検査機能を読み込み始めます。 |
| `checking_sources` | `fileCount: number` | 検査が始まります。件数は除外ルール適用前の入力ファイル数です。 |
| `check_finished` | `check: CheckResult` | 1 種類の検査が完了します。ID、ラベル、状態、エラーと警告の件数、任意の詳細を返します。 |
| `collecting_diagnostics` | なし | 診断レポートを返す直前です。 |

検査 ID は `json`、`structure`、`syntax`、`permissions`、`schemas` です。状態は `passed`、`warning`、`failed`、`skipped` のいずれかです。読み込み後のソース検査は同期実行されます。検査中も画面を操作できるようにするには、Web Worker で実行します。

#### ビルドオプション {#pack-options}

`PackOptions` のフィールドはすべて省略できます。

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `buildId` | `string` | `VERSION` に保存するビルド ID。既定では UUID v4 を生成します。 |
| `agentId` | `string` | `agent_id` として保存するプロジェクト ID。ASCII の小文字、数字、`_`、`-` からなる空でない文字列で、先頭や末尾に `-` は使えません。省略すると ID は設定されません。 |
| `engine` | `string` | AIUI のバージョン範囲。省略時は `app.json.engine`、それもなければ `*` を使います。 |
| `optimize` | `false \| OptimizeOptions` | 省略または `false` で JSON と画像の任意の最適化を無効にします。オブジェクトを渡すと[最適化設定](#optimize-options)が有効になります。 |
| `signal` | `AbortSignal` | ビルドと、同じシグナルを使うアップロードを取り消します。 |
| `onProgressiveAsset` | `(candidate, context) => ProgressiveAssetDecision \| Promise<ProgressiveAssetDecision>` | [アセットごとに配信方式を選びます](#progressive-asset-callback)。 |

`PackFromSourceOptions` には、`packFromSource()` と `packFromFiles()` で使う `onProgress?: (event: PackProgressEvent) => void` が加わります。

#### ビルドの進捗コールバック {#build-progress-callback}

~~~typescript
onProgress?: (event: PackProgressEvent) => void
~~~

SDK がビルドの段階を通知するたびに、コールバックはイベントを 1 件受け取ります。戻り値は使いません。例外を投げるとビルドの Promise が拒否されます。ビルドが途中で終了すると、最後のイベントが届かないことがあります。

| `event.type` | 追加のフィールド | 発生するタイミング |
| --- | --- | --- |
| `transferring_files_to_wasm` | なし | SDK がパッケージ作成処理に入力を渡し始めます。 |
| `collecting_source_inputs` | なし | ソースファイルの収集を始めます。 |
| `resolving_engine` | なし | 対応バージョンの範囲を決めます。 |
| `preparing_files` | なし | 対象ファイルの処理を始めます。 |
| `file_finished` | `report: FileOptimizeReport` | 1 件の処理が完了します。生成した `VERSION` も対象です。 |
| `finalizing_archive` | なし | 最終的なアーカイブとマニフェストの書き込みを始めます。 |

#### 段階配信アセットのコールバック {#progressive-asset-callback}

~~~typescript
onProgressiveAsset?: (
  candidate: ProgressiveAssetCandidate,
  context: ProgressiveAssetPackContext,
) => ProgressiveAssetDecision | Promise<ProgressiveAssetDecision>
~~~

SDK は宣言されたアセットごとにコールバックを 1 回呼び出します。現在のアセットの配信方式が決まってから、次のアセットを処理します。

| 引数 | 型 | 受け取る内容 |
| --- | --- | --- |
| `candidate` | `ProgressiveAssetCandidate` | ソースパス、サイズ、メディアの種類、参照元、形式の対応状況。 |
| `context.signal` | `AbortSignal` | アップロード処理にも渡せるビルドのキャンセルシグナル。 |
| `context.prepare` | `() => Promise<ProgressiveAssetPlan>` | リモート配信を準備します。コールバック内で呼び出します。プランには `id`、`assetVersion`、タイムライン情報、`remoteObjects` 内のバイト列が含まれます。未対応のメディアの場合は Promise が拒否されます。 |

次のいずれかの結果を直接返すか、その結果に解決される Promise を返します。

| 結果 | 動作 |
| --- | --- |
| `{ delivery: 'embedded' }` | 元のメディアをパッケージに含めます。コールバックを省略した場合の既定値です。 |
| `{ delivery: 'progressive', planId, receipt }` | 準備した断片をリモート配信します。`prepare()` が返した `id` を `planId` に設定し、アップロード結果を渡します。 |

アップロード結果には、空でない `uploadId`、プランの `assetVersion`、`plan.remoteObjects` の各オブジェクトに対応する項目が必要です。各項目には一致する `id`、`size`、`sha256` と、安定した HTTPS の `url` を含めます。URL に認証情報、クエリ文字列、フラグメントは使えません。コールバックの失敗、無効なアップロード結果、キャンセルが発生すると、ビルドの Promise が拒否されます。[アップロードの例](#progressive-assets)と[型定義](#progressive-asset-types)も参照してください。

#### 最適化オプション {#optimize-options}

| フィールド | 型 | 既定値 |
| --- | --- | --- |
| `level` | `1 \| 2 \| 3` | `2` |
| `json` | `boolean` | `true`。JSON を最適化します。 |
| `png` | `boolean` | `true`。PNG 画像を最適化します。 |
| `jpeg` | `boolean` | `true`。JPEG 画像を最適化します。 |

パッケージ化では、オプションオブジェクトを渡すと最適化が有効になります。`AIX.optimize()` はオプションを省略した場合にも上記の既定値を使います。

#### パッケージ化と最適化の結果 {#pack-result}

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `data` | `Uint8Array` | 新しいアーカイブのバイト列。 |
| `report` | `OptimizeReport` | エントリごとのサイズと合計。ZIP 圧縮前のバイト数です。 |
| `warnings` | `string[]` | 処理中の警告。 |
| `progressiveAssets` | `ProgressiveAssetReport[]` | 宣言されたアセットの配信方式と、パッケージに残った元データのバイト数。既存のパッケージを最適化した場合は空の配列を返します。 |

#### AixCheckError {#reference-aix-check-error}

~~~typescript
constructor(report: CheckReport)
~~~

`AIX.From()` で任意のソース検査を有効にしてエラーが見つかると、この `Error` のサブクラスで Promise が拒否されます。コンストラクターは検査レポート全体を受け取ります。インスタンスの `name` は `'AixCheckError'`、`message` はエラーの概要、`report` は渡された検査レポートです。`diagnostics` は警告を含む `report.diagnostics` を返します。

<div id="型リファレンス"></div>

## 型リファレンス {#type-reference}

各グループを開くと TypeScript の型定義を確認できます。

### 入力ファイルとビルド結果 {#file-and-build-types}

::: details ファイル、読み込み上限、ビルドオプション
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

### 段階配信アセットの型 {#progressive-asset-types}

::: details Progressive Asset の型
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

### ビルド進捗のイベント {#packprogressevent}

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

`file_finished` は処理を終えたエントリのレポートを持ちます。生成した `VERSION` も対象です。
:::

<div id="ソース検査の型"></div>

### ソース検査の型 {#source-check-types}

::: details 検査レポートと進捗イベント
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

<div id="パッケージおよび派生モデル"></div>

### パッケージと派生モデル {#package-and-derived-models}

::: details アーカイブエントリ、ページ、ウィジェット、ツール
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

ページの `size` とツールの `layout` は幅と高さの制約を表します。ツールの `type` は `'function'`、対象は `'_current'` または `'_blank'` です。`JsonSchema` は任意の JSON 値です。利用する前に、ソース検査でスキーマ宣言を検証できます。

これらのメタデータにない文字列は `null` です。WASM の生の出力も同じです。`getTitle()` や `getVersion()` などの個別メソッドは、値がない場合に `undefined` を返します。
:::

<div id="ソースからのビルド"></div>

## SDK をローカルでビルドする {#build-from-source}

~~~bash
cd crates/aix-web
npm install
npm run build
~~~

成果物は `crates/aix-web/dist` に出力されます。自分で配信する場合は、成果物のディレクトリ全体を公開してください。パッケージを開くときは `pkg-reader/` を、検査、パッケージ化、最適化を行うときは必要に応じて `pkg/` を読み込みます。[WASM の読み込み（英語）](/wasm?lang=en)も参照してください。
