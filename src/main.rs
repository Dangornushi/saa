mod calendar;
mod cli;
mod config;
mod llm;
mod models;
mod scheduler;
mod storage;

#[cfg(test)]
mod tests;

use anyhow::Result;
use cli::{Cli, CliApp};
use config::ConfigManager;
use llm::{LLMClient, MockLLMClient, LLM};
use scheduler::Scheduler;
use std::sync::Arc;
use std::io::{self, Write};

async fn interactive_mode(use_mock_llm: bool) -> Result<()> {
        println!("🤖 AI予定管理アシスタントへようこそ！");
        println!("会話履歴を記録して、スムーズな対話を提供します。");
        println!("");
        println!("📋 利用可能なコマンド:");
        println!("  • 'history' - 会話履歴を表示");
        println!("  • 'save' - 会話ログをファイルに保存");
        println!("  • 'save <ファイル名>' - 指定したファイル名で保存");
        println!("  • 'clear' - 会話履歴をクリア");
        println!("  • 'sync' - Google Calendarと同期（Google Calendar設定済みの場合）");
        println!("  • 'exit' または 'quit' - 終了（会話ログを表示）");
        println!("");

        let config_manager = ConfigManager::new()?;
        let config = config_manager.load_config()?;

        let llm: Arc<dyn LLM> = if use_mock_llm {
            Arc::new(MockLLMClient::new())
        } else {
            Arc::new(LLMClient::from_config(&config)?)
        };

        // LLMとの接続テスト
        llm.test_connection().await?;

        // Google Calendar設定の確認
        let mut scheduler = match Scheduler::new_with_calendar(
            llm.clone(),
            "client_secret.json",
            "token_cach.json"
        ).await {
            Ok(scheduler) => {
                println!("✅ Google Calendarとの連携が有効になりました");
                scheduler
            }
            Err(e) => {
                println!("⚠️  Google Calendar設定が見つかりません。ローカルモードで動作します");
                println!("{}", e);
                Scheduler::new(llm)?
            }
        };

        loop {
            print!("💬 あなた: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                // 会話終了時に会話ログを表示
                println!("\n📋 === 会話ログ ===");
                println!("{}", scheduler.get_conversation_summary());
                println!("\n👋 さようなら！");
                break;
            }

            if input.eq_ignore_ascii_case("history") {
                println!("{}", scheduler.get_conversation_summary());
                continue;
            }

            if input.eq_ignore_ascii_case("save") || input.starts_with("save ") {
                let file_path = if input.starts_with("save ") {
                    Some(input.strip_prefix("save ").unwrap())
                } else {
                    None
                };
                
                match scheduler.save_conversation_log_to_file(file_path) {
                    Ok(saved_path) => {
                        println!("💾 会話ログを保存しました: {}", saved_path);
                    }
                    Err(e) => {
                        eprintln!("❌ ログ保存エラー: {}", e);
                    }
                }
                continue;
            }

            if input.eq_ignore_ascii_case("clear") {
                scheduler.clear_conversation_history()?;
                println!("🗑️ 会話履歴をクリアしました");
                continue;
            }

            if input.eq_ignore_ascii_case("sync") {
                match scheduler.sync_with_google_calendar().await {
                    Ok(sync_result) => {
                        println!("🔄 {}", sync_result);
                    }
                    Err(e) => {
                        eprintln!("❌ 同期エラー: {}", e);
                    }
                }
                continue;
            }

            match scheduler.process_user_input(input.to_string()).await {
                Ok(response) => {
                    println!("🤖 アシスタント: {}", response);
                }
                Err(e) => {
                    eprintln!("❌ エラー: {}", e);
                }
            }
            
            println!(); // 空行を追加
        }
        return Ok(());

}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let use_mock_llm = cli.mock_llm;
    let verbose = cli.verbose;

    // サブコマンドが指定されていない場合、またはinteractiveサブコマンドの場合は
    // 会話ログ機能付きのインタラクティブモードを使用
    if cli.matches.subcommand_name().is_none() || cli.matches.subcommand_name() == Some("interactive") {
        return interactive_mode(use_mock_llm).await;
    }

    // その他のコマンドは従来のCLIAppを使用
    let mut app = CliApp::new(use_mock_llm, verbose).await?;
    app.run(cli).await?;

    Ok(())
}
