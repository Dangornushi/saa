use anyhow::Result;
use std::collections::HashMap;
use std::io::{self, Write, BufRead};
use std::sync::Arc;
use crate::scheduler::Scheduler;
use colored::Colorize;
use async_trait::async_trait;

/// コマンド実行結果
#[derive(Debug)]
pub enum CommandResult {
    Continue,
    Exit,
    ShowHelp,
}

/// インタラクティブモードのコマンドハンドラー
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult>;
    fn help(&self) -> &str;
    fn aliases(&self) -> Vec<&str> { vec![] }
}

/// 履歴表示コマンド
pub struct HistoryCommand;

#[async_trait]
impl CommandHandler for HistoryCommand {
    async fn execute(&self, _args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult> {
        println!("📋 === 会話履歴 ===");
        println!("{}", scheduler.get_conversation_summary());
        Ok(CommandResult::Continue)
    }

    fn help(&self) -> &str {
        "会話履歴を表示します"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["h", "hist"]
    }
}

/// 保存コマンド
pub struct SaveCommand;

#[async_trait]
impl CommandHandler for SaveCommand {
    async fn execute(&self, args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult> {
        let file_path = if args.len() > 1 {
            Some(args[1])
        } else {
            None
        };
        
        match scheduler.save_conversation_log_to_file(file_path) {
            Ok(saved_path) => {
                println!("💾 会話ログを保存しました: {}", saved_path.green());
            }
            Err(e) => {
                eprintln!("❌ ログ保存エラー: {}", e.to_string().red());
            }
        }
        Ok(CommandResult::Continue)
    }

    fn help(&self) -> &str {
        "会話ログをファイルに保存します。使用法: save [ファイル名]"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["s"]
    }
}

/// クリアコマンド
pub struct ClearCommand;

#[async_trait]
impl CommandHandler for ClearCommand {
    async fn execute(&self, _args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult> {
        scheduler.clear_conversation_history()?;
        println!("🗑️ 会話履歴をクリアしました");
        Ok(CommandResult::Continue)
    }

    fn help(&self) -> &str {
        "会話履歴をクリアします"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["c", "reset"]
    }
}

/// 同期コマンド
pub struct SyncCommand;

#[async_trait]
impl CommandHandler for SyncCommand {
    async fn execute(&self, _args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult> {
        match scheduler.sync_with_google_calendar().await {
            Ok(sync_result) => {
                println!("🔄 {}", sync_result.green());
            }
            Err(e) => {
                eprintln!("❌ 同期エラー: {}", e.to_string().red());
            }
        }
        Ok(CommandResult::Continue)
    }

    fn help(&self) -> &str {
        "Google Calendarと同期します"
    }
}

/// 終了コマンド
pub struct ExitCommand;

#[async_trait]
impl CommandHandler for ExitCommand {
    async fn execute(&self, _args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult> {
        println!("\n📋 === 会話ログ ===");
        println!("{}", scheduler.get_conversation_summary());
        println!("\n👋 さようなら！");
        Ok(CommandResult::Exit)
    }

    fn help(&self) -> &str {
        "アプリケーションを終了します"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["quit", "q", "bye"]
    }
}

/// ヘルプコマンド
pub struct HelpCommand;

#[async_trait]
impl CommandHandler for HelpCommand {
    async fn execute(&self, _args: Vec<&str>, _scheduler: &mut Scheduler) -> Result<CommandResult> {
        Ok(CommandResult::ShowHelp)
    }

    fn help(&self) -> &str {
        "このヘルプメッセージを表示します"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["?"]
    }
}

/// AI処理コマンド（デフォルトのコマンド）
pub struct AiCommand;

#[async_trait]
impl CommandHandler for AiCommand {
    async fn execute(&self, args: Vec<&str>, scheduler: &mut Scheduler) -> Result<CommandResult> {
        let input = args.join(" ");
        match scheduler.process_user_input(input).await {
            Ok(response) => {
                println!("🤖 アシスタント: {}", response);
            }
            Err(e) => {
                eprintln!("❌ エラー: {}", e.to_string().red());
            }
        }
        Ok(CommandResult::Continue)
    }

    fn help(&self) -> &str {
        "AIアシスタントに質問や依頼を送信します"
    }
}

/// インタラクティブモードの管理構造体
pub struct InteractiveMode {
    commands: HashMap<String, Arc<dyn CommandHandler>>,
    default_handler: Arc<dyn CommandHandler>,
}

impl InteractiveMode {
    pub fn new() -> Self {
        let mut commands: HashMap<String, Arc<dyn CommandHandler>> = HashMap::new();
        
        // コマンドを登録
        let history_cmd = Arc::new(HistoryCommand);
        commands.insert("history".to_string(), history_cmd.clone());
        for alias in history_cmd.aliases() {
            commands.insert(alias.to_string(), history_cmd.clone());
        }

        let save_cmd = Arc::new(SaveCommand);
        commands.insert("save".to_string(), save_cmd.clone());
        for alias in save_cmd.aliases() {
            commands.insert(alias.to_string(), save_cmd.clone());
        }

        let clear_cmd = Arc::new(ClearCommand);
        commands.insert("clear".to_string(), clear_cmd.clone());
        for alias in clear_cmd.aliases() {
            commands.insert(alias.to_string(), clear_cmd.clone());
        }

        let sync_cmd = Arc::new(SyncCommand);
        commands.insert("sync".to_string(), sync_cmd);

        let exit_cmd = Arc::new(ExitCommand);
        commands.insert("exit".to_string(), exit_cmd.clone());
        for alias in exit_cmd.aliases() {
            commands.insert(alias.to_string(), exit_cmd.clone());
        }

        let help_cmd = Arc::new(HelpCommand);
        commands.insert("help".to_string(), help_cmd.clone());
        for alias in help_cmd.aliases() {
            commands.insert(alias.to_string(), help_cmd.clone());
        }

        Self {
            commands,
            default_handler: Arc::new(AiCommand),
        }
    }

    pub fn show_welcome(&self) {
        println!("{}", "🤖 AI予定管理アシスタントへようこそ！".bold().cyan());
        println!("会話履歴を記録して、スムーズな対話を提供します。");
        println!();
        self.show_help();
        println!();
    }

    pub fn show_help(&self) {
        println!("{}", "📋 利用可能なコマンド:".bold().blue());
        
        // コマンドを収集して重複を除去
        let mut unique_commands: Vec<_> = self.commands.iter()
            .filter_map(|(name, handler)| {
                // エイリアスではなく、主要なコマンド名のみを表示
                if !handler.aliases().contains(&name.as_str()) {
                    Some((name, handler))
                } else {
                    None
                }
            })
            .collect();
        unique_commands.sort_by_key(|(name, _)| name.as_str());

        for (name, handler) in unique_commands {
            let aliases = handler.aliases();
            let alias_text = if aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", aliases.join(", "))
            };
            println!("  • '{}'{} - {}", name.green(), alias_text.dimmed(), handler.help());
        }
        println!("  • {} - {}", "その他のテキスト".green(), self.default_handler.help());
    }

    pub async fn run(&self, scheduler: &mut Scheduler) -> Result<()> {
        self.show_welcome();

        let stdin = io::stdin();
        let mut lines = stdin.lock().lines();

        loop {
            print!("{} ", "💬 あなた:".bold().cyan());
            io::stdout().flush()?;

            let input = match lines.next() {
                Some(Ok(line)) => line,
                Some(Err(e)) => return Err(e.into()),
                None => {
                    // EOF（パイプが閉じられた場合など）
                    println!("\n👋 セッションを終了します。");
                    break;
                }
            };

            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            let args: Vec<&str> = input.split_whitespace().collect();
            if args.is_empty() {
                continue;
            }

            let command_name = args[0].to_lowercase();
            let result = if let Some(handler) = self.commands.get(&command_name) {
                handler.execute(args, scheduler).await?
            } else {
                self.default_handler.execute(args, scheduler).await?
            };

            match result {
                CommandResult::Continue => {
                    println!(); // 空行を追加
                }
                CommandResult::Exit => break,
                CommandResult::ShowHelp => {
                    self.show_help();
                    println!();
                }
            }
        }

        Ok(())
    }

    /// 新しいコマンドを追加
    pub fn register_command(&mut self, name: String, handler: Arc<dyn CommandHandler>) {
        self.commands.insert(name, handler);
    }
}

impl Default for InteractiveMode {
    fn default() -> Self {
        Self::new()
    }
}
