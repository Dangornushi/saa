# Schedule AI Agent

Rust言語で開発されたLLMを活用した予定管理AIエージェントです。自然言語での予定管理と外部カレンダーサービスとの連携機能を提供します。

## 機能

- 🤖 **自然言語処理**: LLMを使用して自然言語での予定管理
- 📅 **予定管理**: 予定の作成、更新、削除、検索
- 💾 **ローカルストレージ**: JSONファイルでの予定データ保存
- 🔄 **バックアップ・復元**: 予定データのバックアップと復元
- 📊 **統計情報**: 予定の統計情報表示
- 🌐 **カレンダー連携**: Google Calendar等との連携（基盤実装済み）
- 🎨 **カラフルなCLI**: 見やすいコマンドライン インターフェース

## インストール

### 前提条件

- Rust 1.70以上
- OpenAI API キー（オプション、モックLLMも利用可能）

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
```

### 自然言語での操作例

インタラクティブモードで以下のような自然言語入力が可能です：

```
>>> 明日の午後2時から3時まで歯医者の予定を追加して
>>> 来週の会議の一覧を見せて
>>> 今日の予定は何？
>>> 「プロジェクト」に関する予定を検索して
>>> 統計情報を教えて
```

## 設定

### 環境変数

```bash
# OpenAI API キー（LLM機能を使用する場合）
export OPENAI_API_KEY="your-api-key-here"

# カスタムOpenAI互換API（オプション）
export OPENAI_BASE_URL="https://api.openai.com/v1"

# Google Calendar連携（将来実装）
export GOOGLE_CALENDAR_ACCESS_TOKEN="your-token"
export GOOGLE_CALENDAR_ID="primary"
```

### データ保存場所

予定データは以下の場所に保存されます：
- Linux/macOS: `~/.schedule_ai_agent/schedule.json`
- Windows: `%USERPROFILE%\.schedule_ai_agent\schedule.json`

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

## カレンダー連携

### Google Calendar連携

Google Calendar連携を有効にするには：

1. Google Cloud Consoleでプロジェクトを作成
2. Calendar APIを有効化
3. OAuth 2.0認証情報を設定
4. アクセストークンを取得
5. 環境変数を設定

```bash
export GOOGLE_CALENDAR_ACCESS_TOKEN="your-access-token"
export GOOGLE_CALENDAR_ID="primary"  # または特定のカレンダーID
```

### Notion Calendar連携

Notion Calendar連携は将来実装予定です。基盤となるコードは既に含まれています。

## トラブルシューティング

### よくある問題

1. **LLM APIエラー**
   - API キーが正しく設定されているか確認
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

- [ ] Google Calendar完全連携
- [ ] Notion Calendar連携
- [ ] Outlook Calendar連携
- [ ] リマインダー機能
- [ ] 繰り返し予定
- [ ] Web UI
- [ ] モバイルアプリ連携
- [ ] 音声入力対応
- [ ] 多言語対応# saa
