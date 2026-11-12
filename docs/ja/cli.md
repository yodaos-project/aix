# @yodaos-pkg/aix-cli

**AIX**（AI eXecutable）パッケージの作成、検査、インストール、プレビューを行うコマンドラインツールです。

## 目次

- [インストール](#インストール)
- [パッケージコマンド](#パッケージコマンド)
- [デバイスコマンド](#デバイスコマンド)
- [プレビューコマンド](#プレビューコマンド)
- [開発](#開発)
- [ライセンス](#ライセンス)

## インストール

```bash
npm install -g @yodaos-pkg/aix-cli
```

## パッケージコマンド

### `aix pack <INPUT_DIR>`

ディレクトリを `.aix` にパッケージ化します。JSON の検証、対応テキストの UTF-8 変換、UUID v4 の `VERSION` 生成、`META-INF/aix/manifest.json` の書き込みを行います。

```bash
aix pack ./my-agent
aix pack ./my-agent -o my-app.aix
aix pack ./my-agent --engine '^0.14.0'
aix pack ./my-agent --log-time
```

JS/TS はソースファイルを変更せず、既定で minify されます。`--optimize` を指定すると JSON、PNG、JPEG も最適化します。

```bash
aix pack ./my-agent --optimize
aix pack ./my-agent -O --opt-level 3
```

`--engine` を省略した場合は `app.json.engine`、次に `*` を使用します。`.aixignore` は `.gitignore` 構文に従います。`--log-time` はパッケージログに時刻を付けます。

### `aix show <INPUT>`

ADB を使わず、有効な Agent Definition JSON を出力します。`show` と `install` は同じリゾルバーを使用します。

```bash
aix show ./my-agent
aix show ./bundle.aix --compact
aix show ./my-agent -o ./agent.json
```

### `aix list <AIX_FILE>`

アーカイブのエントリーとサイズを一覧表示します。`aix ls` はエイリアスです。

```bash
aix list bundle.aix
aix ls bundle.aix
```

### `aix optimize <AIX_FILE>`

既存パッケージ内の JSON、PNG、JPEG を最適化します。

```bash
aix optimize input.aix -o output.aix --level 2
```

## デバイスコマンド

すべてのデバイスコマンドは `-s, --serial <serial>` を受け付けます。オンラインで認証済みの ADB デバイスが 1 台なら自動選択し、複数台なら対話式セレクターを開きます。非対話シェルでは `--serial` が必須です。

実行中の段階はスピナーで表示されます。完了した中間段階はチェックマーク 1 行に折りたたまれ、最新結果だけが詳細を保持します。

### `aix device [ACTION]`

```bash
aix device
aix device set-dev
aix device unset-dev
```

読み取り専用形式では Developer Mode、Widget の準備状態とレイアウト、インストール済み `.aix` Agent を表示します。Developer Mode を変更するとすべての Widget が再読み込みされるため、動的 Widget は再度起動する必要があります。

### `aix install <INPUT>`

プロジェクトをパッケージ化するか `.aix` を受け取り、ADB 経由で AIUI DEVELOP に送信します。

```bash
aix install ./my-agent
aix install ./bundle.aix
aix install ./my-agent --definition ./agent.json --serial <serial>
```

ディレクトリは `.aix/agent-id` を再利用し、成果物は `VERSION` を使用します。`agent.json` は `agentId` 以外の生成済みメタデータを上書きできます。適用結果と端末へのアップロード結果は検証されますが、端末側の確認だけではクラウドへのインデックス登録を保証しません。

### `aix launch-page <INPUT> [PATH]`

プロジェクトディレクトリまたは `.aix` ファイルを指定して、インストール済み Agent のページを開きます。

```bash
aix launch-page ./my-agent
aix launch-page ./bundle.aix
aix launch-page ./my-agent pages/index/index
```

単一ファイルをデバッグするには、ADB 認証済みのデバイスを接続し、`aix device set-dev` で AIUI Developer Mode を有効にしてから実行します。

```bash
aix launch-page ./counter.ink
aix launch-page ./counter.ink --card --params '{"count":7}'
aix launch-page ./counter.ink --params-file ./params.json --serial DEVICE_SERIAL
```

既定では全画面で開き、`--card` でカード表示に切り替えます。初期パラメーターには `--params <JSON>` または `--params-file <FILE>` を使用し、JSON オブジェクトを指定してください。`--serial <SERIAL>` でデバイスを選択できます。

`.ink` ファイルは空でない UTF-8、128 KiB 以下で、`<page>` ブロックを 1 個含む必要があります。`<style>` と `<script setup>` は省略でき、`lang="ts"` で TypeScript を使用できます。単一ファイル入力では追加の `[PATH]` と Widget はサポートしません。

編集後は同じコマンドを再実行して変更を確認してください。ページは再起動し、メモリー上の状態はリセットされます。

### `aix launch-widget <INPUT> <PATH>`

```bash
aix launch-widget ./my-agent widgets/order/index
aix launch-widget ./my-agent widgets/order/index --position 2
```

サイズは `app.json.widgets[].family` から取得します。互換性のある配置は維持されます。位置が競合した場合は重なっている overlay Widget を置き換え、空きがない場合は古い overlay 配置を消去します。persistent Widget は削除しません。

必要に応じて `prepare -> push widget-config.json -> widget-apply -> open` を実行します。ランタイム配置が見つからない場合はレイアウトを 1 回だけ再適用し、Open を再試行します。

### `aix widget-layout [show]`

```bash
aix widget-layout
aix widget-layout show
aix widget-layout --clear
aix widget-layout --clear --yes
```

Show は読み取り専用です。Clear はグリッドを維持したまま両方のモジュール配列を空にし、`--yes` がなければ確認を求めます。

## プレビューコマンド

### `aix preview <INPUT>`

`@yodaos-pkg/ink` を使って成果物またはソースディレクトリをプレビューします。

```bash
aix preview bundle.aix
aix preview ./my-agent
aix preview bundle.aix --launch
aix preview bundle.aix --launch --launch-target current
```

既定ではローカルサーバーを起動して URL を表示します。`blank` ビューポートは `480x352`、`current` は `448x150` です。

サーバーを起動せず静的 HTML を出力するには次を実行します。

```bash
aix preview bundle.aix --html-out ./artifacts/preview.html
```

`--html-out` と `--launch` は併用できません。WebSocket によるライブリロードには次を使用します。

```bash
aix preview ./my-agent --dev
aix preview ./my-agent --dev --launch
```

### `aix runtime`

```bash
aix runtime versions
aix runtime current
aix runtime select
```

- `versions` は安定版と有効な選択結果を一覧表示します。
- `current` は解決済みバージョンだけを表示します。
- `select` は対話式の選択結果を `~/.aix/runtime.json` に保存します。

レジストリーは `AIX_NPM_REGISTRY=npm`（既定）または `AIX_NPM_REGISTRY=npmmirror` で選択します。

## 開発

```bash
npm install
npm run build
node dist/cli.js --help
```

ビルドは Rust エンジンを Node.js WASM にコンパイルし、TypeScript CLI をバンドルします。パッケージ化、最適化、読み取りのロジックは Web サーフェスと共有されます。

## ライセンス

MIT
