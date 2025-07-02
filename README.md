# Schedule AI Agent

Rust言語で開発されたLLMを活用した予定管理AIエージェントです。自然言語での予定管理と外部カレンダーサービスとの連携機能を提供します。

## 機能

- 🤖 **自然言語でのAI対話**: LLMを使用して自然言語での予定管理
- 📅 **予定管理**: 予定の作成、更新、削除、検索
- 🗓️ **Google Calendar連携**: Google Calendarとの双方向同期
- 💾 **ローカルストレージ**: JSONファイルでの予定データ保存
- 🔄 **バックアップ・復元**: 予定データのバックアップと復元
- 📊 **統計情報**: 予定の統計情報表示
- 🎨 **カラフルなCLI**: 見やすいコマンドライン インターフェース

## インストール

### 前提条件

- Rust 1.70以上
- LLM API キー（Gemini API推奨、モックLLMも利用可能）
- Google Calendar API設定（オプション、カレンダー連携機能を使用する場合）

### ビルド

```bash
git clone <repository-url>
cd schedule_ai_agent
cargo build --release
```

## 使用方法

### 基本的な使用方法

```bash
# インタラクティブモードで起動
cargo run

# モックLLMを使用（API キー不要）
cargo run -- --mock-llm

# 詳細出力を有効化
cargo run -- --verbose
```

### コマンドライン操作

```bash
# 予定を直接追加
cargo run -- add "会議" --start "2024-01-15T10:00:00Z" --end "2024-01-15T11:00:00Z" --description "プロジェクト会議"

# 予定一覧を表示
cargo run -- list

# 今後の予定のみ表示
cargo run -- list --upcoming

# 今日の予定のみ表示
cargo run -- list --today

# 予定を検索
cargo run -- search "会議"

# 統計情報を表示
cargo run -- stats

# バックアップを作成
cargo run -- backup

# バックアップから復元
cargo run -- restore

# 予定をエクスポート
cargo run -- export schedule_backup.json

# 予定をインポート
cargo run -- import schedule_backup.json

# 統計を表示
cargo run -- stats
```

### Google Calendar連携コマンド

```bash
# Google Calendarで認証
cargo run -- calendar auth

# 今日のGoogle Calendarの予定を表示
cargo run -- calendar today

# 今週のGoogle Calendarの予定を表示
cargo run -- calendar week

# Google Calendarの情報を同期
cargo run -- calendar sync

# Google Calendarにイベントを作成
cargo run -- calendar create "会議" --start "2024-01-15T10:00:00Z" --end "2024-01-15T11:00:00Z" --description "重要な会議" --location "会議室A"

# 空き時間を検索（60分間の空き時間を7日先まで検索）
cargo run -- calendar find-free 60 --days 7
```

### Google Calendar設定

Google Calendar連携を使用するには、以下の手順が必要です：

1. **Google Cloud Console設定**
   - [Google Cloud Console](https://console.cloud.google.com/)でプロジェクトを作成
   - Google Calendar APIを有効化
   - OAuth 2.0認証情報を作成（デスクトップアプリケーション）
   - `client_secret.json`ファイルをダウンロード

2. **設定ファイル更新**
   ```toml
   [google_calendar]
   client_secret_path = "client_secret.json"
   token_cache_path = "token_cache.json"
   calendar_id = "primary"
   ```

3. **初回認証**
   ```bash
   cargo run -- calendar auth
   ```
   
   ブラウザが開き、Google認証が求められます。認証後、トークンが自動保存されます。

### 統計表示

予定の統計情報を表示します：

```bash
cargo run -- stats
```

詳細な設定方法については、設定ファイルのコメントを参照してください。

### 自然言語での操作例

インタラクティブモードで以下のような自然言語入力が可能です：

```
>>> 明日の午後2時から3時まで歯医者の予定を追加して
>>> 来週の会議の一覧を見せて
>>> 今日の予定は何？
>>> 「プロジェクト」に関する予定を検索して
>>> 統計情報を教えて
```

### TUI（Terminal User Interface）モード

```bash
# TUIチャットインターフェースで起動
cargo run -- tui

# TUIモードでモックLLMを使用
cargo run -- tui --mock-llm
```

TUIモードでは、Gemini CLIやRovodev、Claude codeのような洗練されたCLIチャットインターフェースを提供します：

**機能:**
- 📱 リアルタイムチャット形式でのAI対話
- ⌨️ 直感的なキーボード操作
- 🎨 カラフルで見やすいUI
- 📜 メッセージ履歴の表示
- 🔄 リアルタイム処理状況表示
- ❓ 内蔵ヘルプシステム

**キーボードショートカット:**
- `Enter`: メッセージ送信
- `Ctrl+H`: ヘルプの表示/非表示
- `Ctrl+C` / `Esc`: アプリケーション終了
- `←/→`: カーソル移動
- `Backspace`: 文字削除

**使用例:**
```
👤 You: 明日の3時に会議を追加して
🤖 AI: 明日の15:00に会議を追加しました！
👤 You: 来週の予定を教えて
🤖 AI: 来週の予定をお調べします...
```

## 設定

### 環境変数

```bash
# Gemini API キー（LLM機能を使用する場合）
export GEMINI_API_KEY="your-api-key-here"

# カスタムGemini API URL（オプション）
export GEMINI_BASE_URL="https://generativelanguage.googleapis.com/v1beta"

# Google Calendar連携
export GOOGLE_CALENDAR_ACCESS_TOKEN="your-token"
export GOOGLE_CALENDAR_ID="primary"
```

### データ保存場所

予定データは以下の場所に保存されます：
- Linux/macOS: `~/.schedule_ai_agent/schedule.json`
- Windows: `%USERPROFILE%\.schedule_ai_agent\schedule.json`

設定ファイルは以下の場所に保存されます：
- Linux/macOS: `~/.schedule_ai_agent/config.toml`
- Windows: `%USERPROFILE%\.schedule_ai_agent\config.toml`

## 設定ファイル

設定ファイルを初期化するには：

```bash
cargo run -- config init
```

設定ファイルの例：

```toml
[llm]
base_url = "https://generativelanguage.googleapis.com/v1beta"
model = "gemini-2.5-flash"
temperature = 0.7
max_tokens = 1000
gemini_api_key = "your-gemini-api-key"

[app]
data_dir = "~/.schedule_ai_agent"
backup_count = 5
auto_backup = true
verbose = false
```

## 開発

### プロジェクト構造

```
src/
├── main.rs          # エントリーポイント
├── models.rs        # データ構造定義
├── llm.rs          # LLM連携
├── scheduler.rs     # 予定管理コア機能
├── storage.rs       # ローカルストレージ
├── cli.rs          # コマンドライン インターフェース
├── config.rs        # 設定管理
└── calendar.rs      # カレンダー連携（基盤）
```

### 依存関係

主要な依存関係：
- `tokio`: 非同期ランタイム
- `reqwest`: HTTP クライアント
- `serde`: シリアライゼーション
- `chrono`: 日時処理
- `clap`: コマンドライン引数解析
- `dialoguer`: インタラクティブUI
- `colored`: カラー出力

### テスト

```bash
# 単体テストを実行
cargo test

# 統合テストを実行
cargo test --test integration_tests
```

## トラブルシューティング

### よくある問題

1. **LLM APIエラー**
   - `Date` (日付)
   - `Status` (選択、オプション)
   - `Priority` (選択、オプション)
   - `Description` (リッチテキスト、オプション)
   - `Location` (リッチテキスト、オプション)
   - `Attendees` (マルチセレクト、オプション)
   - `Tags` (マルチセレクト、オプション)

4. **データベースIDを取得**
   - データベースページで「共有」→「リンクをコピー」
   - URLから32文字のIDを抽出
   - 例：`https://www.notion.so/yourworkspace/database-id?v=view-id`

5. **環境変数を設定**
   ```bash
   export NOTION_API_TOKEN="secret_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   export NOTION_DATABASE_ID="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   ```

6. **設定ファイルに追加（オプション）**
   ```toml
   [calendar.notion]
   api_key = "secret_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   database_id = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   ```

7. **連携をテスト**
   ```bash
   cargo run -- notion test
   - Gemini API キーが正しく設定されているか確認
   - `--mock-llm` フラグでモックLLMを使用

2. **日時解析エラー**
   - ISO 8601形式（`2024-01-15T10:00:00Z`）を使用
   - 相対的な表現（「明日」「来週」）はLLMが解析

3. **ファイル権限エラー**
   - ホームディレクトリの書き込み権限を確認

### ログ出力

詳細なログを確認するには：

```bash
cargo run -- --verbose
```

## ライセンス

MIT License

## 貢献

プルリクエストやイシューの報告を歓迎します。

## 今後の予定

- [ ] 外部カレンダー連携
- [ ] リマインダー機能
- [ ] 繰り返し予定
- [ ] Web UI
- [ ] モバイルアプリ連携
- [ ] 音声入力対応
- [ ] 多言語対応
