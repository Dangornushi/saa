mod calendar;
mod cli;
mod config;
mod interactive;
mod llm;
mod models;
mod scheduler;
mod storage;
mod tui;

#[cfg(test)]
mod tests;

use anyhow::Result;
use cli::{Cli, CliApp};
use config::ConfigManager;
use interactive::InteractiveMode;
use llm::{LLMClient, MockLLMClient, LLM};
use scheduler::Scheduler;
use std::sync::Arc;
use tui::ChatApp;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🏁 プログラム開始");
    
    let cli = Cli::parse();
    
    let use_mock_llm = cli.mock_llm;
    let verbose = cli.verbose;

    // TUIモードの場合
    if cli.matches.subcommand_name().is_none() || cli.matches.subcommand_name() == Some("tui") {
        return tui_mode(use_mock_llm).await;
    }

    // その他のコマンドは従来のCLIAppを使用
    let mut app = CliApp::new(verbose).await?;
    app.run(cli).await?;

    Ok(())
}

async fn tui_mode(use_mock_llm: bool) -> Result<()> {
    
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
    let scheduler = match Scheduler::new_with_calendar(
        llm.clone(),
        "client_secret.json",
        "token_cache.json"
    ).await {
        Ok(scheduler) => scheduler,
        Err(_) => Scheduler::new(llm)?,
    };

    // TUIアプリケーションを起動
    let mut app = ChatApp::new(scheduler);
    app.run().await?;

    Ok(())
}
