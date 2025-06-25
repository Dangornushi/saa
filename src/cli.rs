use crate::models::{ActionType, LLMRequest, Priority};
use crate::scheduler::Scheduler;
use crate::llm::{LLMClient, MockLLMClient};
use crate::storage::Storage;
use crate::config::{Config, ConfigManager};
use anyhow::Result;
// use chrono::{DateTime, Utc};
use clap::{App, Arg, SubCommand, ArgMatches};
use colored::*;
use dialoguer::{Confirm, Select};
use std::io::{self, Write};

pub struct Cli {
    pub command: Option<String>,
    pub mock_llm: bool,
    pub verbose: bool,
    pub matches: ArgMatches<'static>,
}

impl Cli {
    pub fn parse() -> Self {
        let matches = App::new("schedule-ai")
            .version("0.1.0")
            .about("AI-powered schedule management tool")
            .arg(Arg::with_name("mock-llm")
                .long("mock-llm")
                .help("Use mock LLM instead of real API")
                .takes_value(false))
            .arg(Arg::with_name("verbose")
                .long("verbose")
                .help("Enable verbose output")
                .takes_value(false))
            .subcommand(SubCommand::with_name("interactive")
                .about("Start interactive mode"))
            .subcommand(SubCommand::with_name("add")
                .about("Add a new event")
                .arg(Arg::with_name("title")
                    .help("Event title")
                    .required(true)
                    .index(1))
                .arg(Arg::with_name("description")
                    .long("description")
                    .help("Event description")
                    .takes_value(true))
                .arg(Arg::with_name("start")
                    .long("start")
                    .help("Start time (ISO 8601 format)")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("end")
                    .long("end")
                    .help("End time (ISO 8601 format)")
                    .takes_value(true)
                    .required(true))
                .arg(Arg::with_name("location")
                    .long("location")
                    .help("Location")
                    .takes_value(true))
                .arg(Arg::with_name("priority")
                    .long("priority")
                    .help("Priority (low, medium, high, urgent)")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("list")
                .about("List events")
                .arg(Arg::with_name("upcoming")
                    .long("upcoming")
                    .help("Show only upcoming events")
                    .takes_value(false))
                .arg(Arg::with_name("today")
                    .long("today")
                    .help("Show only today's events")
                    .takes_value(false))
                .arg(Arg::with_name("limit")
                    .long("limit")
                    .help("Limit number of events")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("search")
                .about("Search events")
                .arg(Arg::with_name("query")
                    .help("Search query")
                    .required(true)
                    .index(1)))
            .subcommand(SubCommand::with_name("stats")
                .about("Show statistics"))
            .subcommand(SubCommand::with_name("backup")
                .about("Backup schedule"))
            .subcommand(SubCommand::with_name("restore")
                .about("Restore from backup"))
            .subcommand(SubCommand::with_name("export")
                .about("Export schedule")
                .arg(Arg::with_name("path")
                    .help("Export file path")
                    .required(true)
                    .index(1)))
            .subcommand(SubCommand::with_name("import")
                .about("Import schedule")
                .arg(Arg::with_name("path")
                    .help("Import file path")
                    .required(true)
                    .index(1)))
            .subcommand(SubCommand::with_name("config")
                .about("Configuration management")
                .subcommand(SubCommand::with_name("init")
                    .about("Initialize configuration files"))
                .subcommand(SubCommand::with_name("show")
                    .about("Show current configuration"))
                .subcommand(SubCommand::with_name("path")
                    .about("Show configuration file path"))
                .subcommand(SubCommand::with_name("edit")
                    .about("Open configuration file in editor")))
            .get_matches();

        let command = matches.subcommand_name().map(|s| s.to_string());
        let mock_llm = matches.is_present("mock-llm");
        let verbose = matches.is_present("verbose");

        Self {
            command,
            mock_llm,
            verbose,
            matches,
        }
    }
}

pub struct CliApp {
    scheduler: Scheduler,
    storage: Storage,
    config: Config,
    config_manager: ConfigManager,
    llm_client: Option<LLMClient>,
    mock_llm_client: MockLLMClient,
    use_mock_llm: bool,
    verbose: bool,
}

impl CliApp {
    pub fn new(use_mock_llm: bool, verbose: bool) -> Result<Self> {
        let mut scheduler = Scheduler::new();
        let storage = Storage::new()?;
        
        // 設定管理を初期化
        let config_manager = ConfigManager::new()?;
        let config = config_manager.load_config()?;
        
        // 既存のスケジュールを読み込み
        match storage.load_schedule() {
            Ok(schedule) => {
                scheduler.load_schedule(schedule);
                if verbose {
                    println!("{}", "スケジュールを読み込みました。".green());
                }
            }
            Err(e) => {
                if verbose {
                    println!("{}: {}", "警告".yellow(), e);
                }
            }
        }

        let (llm_client, actual_use_mock_llm) = if !use_mock_llm {
            match LLMClient::from_config(&config) {
                Ok(client) => (Some(client), false),
                Err(e) => {
                    if verbose {
                        println!("{}: {}", "LLM接続エラー".red(), e);
                        println!("{}", "モックLLMを使用します。".yellow());
                    }
                    (None, true)
                }
            }
        } else {
            (None, true)
        };

        Ok(Self {
            scheduler,
            storage,
            config,
            config_manager,
            llm_client,
            mock_llm_client: MockLLMClient::new(),
            use_mock_llm: actual_use_mock_llm,
            verbose,
        })
    }

    pub fn run(&mut self, cli: Cli) -> Result<()> {
        match cli.command.as_deref() {
            Some("interactive") => self.interactive_mode(),
            Some("add") => {
                if let Some(add_matches) = cli.matches.subcommand_matches("add") {
                    let title = add_matches.value_of("title").unwrap().to_string();
                    let description = add_matches.value_of("description").map(|s| s.to_string());
                    let start = add_matches.value_of("start").unwrap().to_string();
                    let end = add_matches.value_of("end").unwrap().to_string();
                    let location = add_matches.value_of("location").map(|s| s.to_string());
                    let priority = add_matches.value_of("priority").map(|s| s.to_string());
                    self.add_event_command(title, description, start, end, location, priority)
                } else {
                    Err(anyhow::anyhow!("Invalid add command"))
                }
            }
            Some("list") => {
                if let Some(list_matches) = cli.matches.subcommand_matches("list") {
                    let upcoming = list_matches.is_present("upcoming");
                    let today = list_matches.is_present("today");
                    let limit = list_matches.value_of("limit").and_then(|s| s.parse().ok());
                    self.list_events_command(upcoming, today, limit)
                } else {
                    self.list_events_command(false, false, None)
                }
            }
            Some("search") => {
                if let Some(search_matches) = cli.matches.subcommand_matches("search") {
                    let query = search_matches.value_of("query").unwrap().to_string();
                    self.search_events_command(query)
                } else {
                    Err(anyhow::anyhow!("Invalid search command"))
                }
            }
            Some("stats") => {
                self.show_statistics()
            }
            Some("backup") => {
                self.backup_command()
            }
            Some("restore") => {
                self.restore_command()
            }
            Some("export") => {
                if let Some(export_matches) = cli.matches.subcommand_matches("export") {
                    let path = export_matches.value_of("path").unwrap().to_string();
                    self.export_command(path)
                } else {
                    Err(anyhow::anyhow!("Invalid export command"))
                }
            }
            Some("import") => {
                if let Some(import_matches) = cli.matches.subcommand_matches("import") {
                    let path = import_matches.value_of("path").unwrap().to_string();
                    self.import_command(path)
                } else {
                    Err(anyhow::anyhow!("Invalid import command"))
                }
            }
            Some("config") => {
                if let Some(config_matches) = cli.matches.subcommand_matches("config") {
                    match config_matches.subcommand() {
                        ("init", _) => self.config_init_command(),
                        ("show", _) => self.config_show_command(),
                        ("path", _) => self.config_path_command(),
                        _ => {
                            println!("Available config subcommands: init, show, path");
                            Ok(())
                        }
                    }
                } else {
                    println!("Available config subcommands: init, show, path");
                    Ok(())
                }
            }
            None => {
                self.interactive_mode()
            }
            _ => {
                Err(anyhow::anyhow!("Unknown command"))
            }
        }
    }

    fn interactive_mode(&mut self) -> Result<()> {
        println!("{}", "=== AI予定管理エージェント ===".bold().blue());
        println!("自然言語で予定を管理できます。'quit'または'exit'で終了します。");
        
        if self.use_mock_llm {
            println!("{}", "注意: モックLLMを使用しています。".yellow());
        }

        loop {
            print!("\n{} ", ">>>".bold().green());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input == "quit" || input == "exit" {
                println!("さようなら！");
                break;
            }

            if let Err(e) = self.process_natural_language_input(input) {
                println!("{}: {}", "エラー".red(), e);
            }
        }

        Ok(())
    }

    fn process_natural_language_input(&mut self, input: &str) -> Result<()> {
        let request = LLMRequest {
            user_input: input.to_string(),
            context: Some(self.get_context_info()),
        };

        let response = if self.use_mock_llm {
            self.mock_llm_client.process_request(request)?
        } else if let Some(ref client) = self.llm_client {
            client.process_request(request)?
        } else {
            return Err(anyhow::anyhow!("LLMクライアントが利用できません"));
        };

        match response.action {
            ActionType::CreateEvent => {
                if let Some(event_data) = response.event_data {
                    match self.scheduler.create_event(event_data) {
                        Ok(event_id) => {
                            println!("{}", response.response_text.green());
                            println!("イベントID: {}", event_id.to_string().cyan());
                            self.save_schedule()?;
                        }
                        Err(e) => {
                            println!("{}: {}", "予定作成エラー".red(), e);
                        }
                    }
                } else {
                    println!("{}", "予定データが不完全です。".red());
                }
            }
            ActionType::ListEvents => {
                println!("{}", response.response_text);
                self.display_events_list(self.scheduler.list_events());
            }
            ActionType::SearchEvents => {
                println!("{}", response.response_text);
                // 検索クエリを抽出（簡易実装）
                let events = self.scheduler.search_events(input);
                self.display_events_list(events);
            }
            _ => {
                println!("{}", response.response_text);
            }
        }

        Ok(())
    }

    fn get_context_info(&self) -> String {
        let stats = self.scheduler.get_statistics();
        let upcoming = self.scheduler.get_upcoming_events(3);
        
        let mut context = format!("現在の予定数: {}", stats.total_events);
        
        if !upcoming.is_empty() {
            context.push_str("\n直近の予定:");
            for event in upcoming {
                context.push_str(&format!(
                    "\n- {} ({})",
                    event.title,
                    event.start_time.format("%Y-%m-%d %H:%M")
                ));
            }
        }
        
        context
    }

    fn add_event_command(
        &mut self,
        title: String,
        description: Option<String>,
        start: String,
        end: String,
        location: Option<String>,
        priority: Option<String>,
    ) -> Result<()> {
        let priority = match priority.as_deref() {
            Some("low") => Priority::Low,
            Some("high") => Priority::High,
            Some("urgent") => Priority::Urgent,
            _ => Priority::Medium,
        };

        let event_data = crate::models::EventData {
            title,
            description,
            start_time: start,
            end_time: end,
            location,
            attendees: Vec::new(),
            priority,
        };

        match self.scheduler.create_event(event_data) {
            Ok(event_id) => {
                println!("{}", "予定を作成しました。".green());
                println!("イベントID: {}", event_id.to_string().cyan());
                self.save_schedule()?;
            }
            Err(e) => {
                println!("{}: {}", "エラー".red(), e);
            }
        }

        Ok(())
    }

    fn list_events_command(&self, upcoming: bool, today: bool, limit: Option<usize>) -> Result<()> {
        let events = if today {
            self.scheduler.get_today_events()
        } else if upcoming {
            self.scheduler.get_upcoming_events(limit.unwrap_or(10))
        } else {
            let mut all_events = self.scheduler.list_events();
            if let Some(limit) = limit {
                all_events.truncate(limit);
            }
            all_events
        };

        if events.is_empty() {
            println!("{}", "予定がありません。".yellow());
        } else {
            let title = if today {
                "今日の予定"
            } else if upcoming {
                "今後の予定"
            } else {
                "全ての予定"
            };
            
            println!("{}", format!("=== {} ===", title).bold().blue());
            self.display_events_list(events);
        }

        Ok(())
    }

    fn search_events_command(&self, query: String) -> Result<()> {
        let events = self.scheduler.search_events(&query);
        
        if events.is_empty() {
            println!("{}", format!("「{}」に一致する予定が見つかりませんでした。", query).yellow());
        } else {
            println!("{}", format!("=== 検索結果: {} ===", query).bold().blue());
            self.display_events_list(events);
        }

        Ok(())
    }

    fn show_statistics(&self) -> Result<()> {
        let stats = self.scheduler.get_statistics();
        
        println!("{}", "=== 予定統計 ===".bold().blue());
        println!("総予定数: {}", stats.total_events.to_string().cyan());
        println!("今後の予定: {}", stats.upcoming_events.to_string().green());
        println!("過去の予定: {}", stats.past_events.to_string().yellow());
        
        println!("\n{}", "優先度別:".bold());
        println!("  低: {}", stats.low_priority.to_string().white());
        println!("  中: {}", stats.medium_priority.to_string().blue());
        println!("  高: {}", stats.high_priority.to_string().yellow());
        println!("  緊急: {}", stats.urgent_priority.to_string().red());

        Ok(())
    }

    fn backup_command(&self) -> Result<()> {
        match self.storage.backup_schedule() {
            Ok(backup_path) => {
                println!("{}", "バックアップを作成しました。".green());
                println!("ファイル: {}", backup_path.display().to_string().cyan());
            }
            Err(e) => {
                println!("{}: {}", "バックアップエラー".red(), e);
            }
        }
        Ok(())
    }

    fn restore_command(&self) -> Result<()> {
        let backups = self.storage.list_backups()?;
        
        if backups.is_empty() {
            println!("{}", "利用可能なバックアップがありません。".yellow());
            return Ok(());
        }

        let backup_names: Vec<String> = backups
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        let selection = Select::new()
            .with_prompt("復元するバックアップを選択してください")
            .items(&backup_names)
            .interact()?;

        let confirm = Confirm::new()
            .with_prompt("現在のスケジュールが上書きされます。続行しますか？")
            .interact()?;

        if confirm {
            match self.storage.restore_schedule(&backups[selection]) {
                Ok(()) => {
                    println!("{}", "スケジュールを復元しました。".green());
                    println!("{}", "アプリケーションを再起動してください。".yellow());
                }
                Err(e) => {
                    println!("{}: {}", "復元エラー".red(), e);
                }
            }
        }

        Ok(())
    }

    fn export_command(&self, path: String) -> Result<()> {
        let export_path = std::path::Path::new(&path);
        
        match self.storage.export_schedule(export_path) {
            Ok(()) => {
                println!("{}", "スケジュールをエクスポートしました。".green());
                println!("ファイル: {}", path.cyan());
            }
            Err(e) => {
                println!("{}: {}", "エクスポートエラー".red(), e);
            }
        }

        Ok(())
    }

    fn import_command(&self, path: String) -> Result<()> {
        let import_path = std::path::Path::new(&path);
        
        let confirm = Confirm::new()
            .with_prompt("現在のスケジュールが上書きされます。続行しますか？")
            .interact()?;

        if confirm {
            match self.storage.import_schedule(import_path) {
                Ok(schedule) => {
                    self.storage.save_schedule(&schedule)?;
                    println!("{}", "スケジュールをインポートしました。".green());
                    println!("{}", "アプリケーションを再起動してください。".yellow());
                }
                Err(e) => {
                    println!("{}: {}", "インポートエラー".red(), e);
                }
            }
        }

        Ok(())
    }

    fn display_events_list(&self, events: Vec<&crate::models::Event>) {
        for (i, event) in events.iter().enumerate() {
            let priority_color = match event.priority {
                Priority::Low => "white",
                Priority::Medium => "blue",
                Priority::High => "yellow",
                Priority::Urgent => "red",
            };

            println!(
                "{}. {} {}",
                (i + 1).to_string().cyan(),
                event.title.bold(),
                format!("[{:?}]", event.priority).color(priority_color)
            );
            
            println!(
                "   {} ～ {}",
                event.start_time.format("%Y-%m-%d %H:%M").to_string().green(),
                event.end_time.format("%Y-%m-%d %H:%M").to_string().green()
            );

            if let Some(ref description) = event.description {
                println!("   {}", description.dimmed());
            }

            if let Some(ref location) = event.location {
                println!("   📍 {}", location.blue());
            }

            if !event.attendees.is_empty() {
                println!("   👥 {}", event.attendees.join(", ").purple());
            }

            println!("   ID: {}", event.id.to_string().dimmed());
            println!();
        }
    }

    fn save_schedule(&self) -> Result<()> {
        self.storage.save_schedule(self.scheduler.get_schedule())?;
        if self.verbose {
            println!("{}", "スケジュールを保存しました。".dimmed());
        }
        Ok(())
    }

    fn config_init_command(&self) -> Result<()> {
        if self.config_manager.config_exists() {
            let confirm = Confirm::new()
                .with_prompt("設定ファイルが既に存在します。上書きしますか？")
                .interact()?;
            
            if !confirm {
                println!("{}", "設定ファイルの初期化をキャンセルしました。".yellow());
                return Ok(());
            }
        }

        match self.config_manager.create_example_files() {
            Ok(files) => {
                println!("{}", "設定ファイルを作成しました:".green());
                for file in files {
                    println!("  {}", file.display().to_string().cyan());
                }
                println!("\n{}", "設定ファイルを編集してAPIキーを設定してください。".yellow());
            }
            Err(e) => {
                println!("{}: {}", "設定ファイル作成エラー".red(), e);
            }
        }

        Ok(())
    }

    fn config_show_command(&self) -> Result<()> {
        println!("{}", "=== 現在の設定 ===".bold().blue());
        
        // LLM設定
        println!("{}", "LLM設定:".bold());
        if let Some(provider) = &self.config.llm.provider {
            println!("  プロバイダー: {}", provider.cyan());
        }
        if let Some(model) = &self.config.llm.model {
            println!("  モデル: {}", model.cyan());
        }
        if let Some(temp) = self.config.llm.temperature {
            println!("  Temperature: {}", temp.to_string().cyan());
        }
        if let Some(tokens) = self.config.llm.max_tokens {
            println!("  Max Tokens: {}", tokens.to_string().cyan());
        }
        
        // APIキーの存在確認（値は表示しない）
        let has_api_key = self.config.llm.api_key.is_some();
        let has_github_token = self.config.llm.github_token.is_some();
        println!("  API Key: {}", if has_api_key { "設定済み".green() } else { "未設定".red() });
        println!("  GitHub Token: {}", if has_github_token { "設定済み".green() } else { "未設定".red() });

        Ok(())
    }

    fn config_path_command(&self) -> Result<()> {
        println!("{}", "=== 設定ファイルパス ===".bold().blue());
        println!("設定ディレクトリ: {}", self.config_manager.get_config_directory_path().display().to_string().cyan());
        println!("設定ファイル: {}", self.config_manager.get_config_file_path().display().to_string().cyan());
        Ok(())
    }
}