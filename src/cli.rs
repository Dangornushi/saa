use crate::calendar::CalendarService;
use crate::config::{Config, ConfigManager};
use crate::models::{Priority, Schedule};
use crate::storage::Storage;
use anyhow::Result;
use chrono_tz::Asia::Tokyo;
use clap::{App, Arg, ArgMatches, SubCommand};
use colored::*;
use dialoguer::{Confirm, Select};
use schedule_ai_agent::GoogleCalendarClient;

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
            .arg(
                Arg::with_name("mock-llm")
                    .long("mock-llm")
                    .help("Use mock LLM instead of real API")
                    .takes_value(false),
            )
            .arg(
                Arg::with_name("verbose")
                    .long("verbose")
                    .help("Enable verbose output")
                    .takes_value(false),
            )
            .subcommand(SubCommand::with_name("interactive").about("Start interactive mode"))
            .subcommand(SubCommand::with_name("tui").about("Start TUI chat mode"))
            .subcommand(
                SubCommand::with_name("add")
                    .about("Add a new event")
                    .arg(
                        Arg::with_name("title")
                            .help("Event title")
                            .required(true)
                            .index(1),
                    )
                    .arg(
                        Arg::with_name("description")
                            .long("description")
                            .help("Event description")
                            .takes_value(true),
                    )
                    .arg(
                        Arg::with_name("start")
                            .long("start")
                            .help("Start time (ISO 8601 format)")
                            .takes_value(true)
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("end")
                            .long("end")
                            .help("End time (ISO 8601 format)")
                            .takes_value(true)
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("location")
                            .long("location")
                            .help("Location")
                            .takes_value(true),
                    )
                    .arg(
                        Arg::with_name("priority")
                            .long("priority")
                            .help("Priority (low, medium, high, urgent)")
                            .takes_value(true),
                    ),
            )
            .subcommand(
                SubCommand::with_name("list")
                    .about("List events")
                    .arg(
                        Arg::with_name("upcoming")
                            .long("upcoming")
                            .help("Show only upcoming events")
                            .takes_value(false),
                    )
                    .arg(
                        Arg::with_name("today")
                            .long("today")
                            .help("Show only today's events")
                            .takes_value(false),
                    )
                    .arg(
                        Arg::with_name("limit")
                            .long("limit")
                            .help("Limit number of events")
                            .takes_value(true),
                    ),
            )
            .subcommand(
                SubCommand::with_name("search").about("Search events").arg(
                    Arg::with_name("query")
                        .help("Search query")
                        .required(true)
                        .index(1),
                ),
            )
            .subcommand(SubCommand::with_name("stats").about("Show statistics"))
            .subcommand(SubCommand::with_name("backup").about("Backup schedule"))
            .subcommand(SubCommand::with_name("restore").about("Restore from backup"))
            .subcommand(
                SubCommand::with_name("conversation")
                    .about("Conversation history management")
                    .subcommand(
                        SubCommand::with_name("show").about("Show conversation history"),
                    )
                    .subcommand(
                        SubCommand::with_name("clear").about("Clear conversation history"),
                    )
                    .subcommand(
                        SubCommand::with_name("summary").about("Show conversation summary"),
                    ),
            )
            .subcommand(
                SubCommand::with_name("export")
                    .about("Export schedule")
                    .arg(
                        Arg::with_name("path")
                            .help("Export file path")
                            .required(true)
                            .index(1),
                    ),
            )
            .subcommand(
                SubCommand::with_name("import")
                    .about("Import schedule")
                    .arg(
                        Arg::with_name("path")
                            .help("Import file path")
                            .required(true)
                            .index(1),
                    ),
            )
            .subcommand(
                SubCommand::with_name("config")
                    .about("Configuration management")
                    .subcommand(
                        SubCommand::with_name("init").about("Initialize configuration files"),
                    )
                    .subcommand(SubCommand::with_name("show").about("Show current configuration"))
                    .subcommand(SubCommand::with_name("path").about("Show configuration file path"))
                    .subcommand(
                        SubCommand::with_name("edit").about("Open configuration file in editor"),
                    ),
            )
            .subcommand(
                SubCommand::with_name("calendar")
                    .about("Google Calendar integration")
                    .subcommand(
                        SubCommand::with_name("auth").about("Authenticate with Google Calendar"),
                    )
                    .subcommand(
                        SubCommand::with_name("today")
                            .about("Show today's events from Google Calendar"),
                    )
                    .subcommand(
                        SubCommand::with_name("week")
                            .about("Show this week's events from Google Calendar"),
                    )
                    .subcommand(
                        SubCommand::with_name("sync").about("Sync events with Google Calendar"),
                    )
                    .subcommand(
                        SubCommand::with_name("create")
                            .about("Create event in Google Calendar")
                            .arg(
                                Arg::with_name("title")
                                    .help("Event title")
                                    .required(true)
                                    .index(1),
                            )
                            .arg(
                                Arg::with_name("start")
                                    .long("start")
                                    .help("Start time (ISO 8601 format)")
                                    .takes_value(true)
                                    .required(true),
                            )
                            .arg(
                                Arg::with_name("end")
                                    .long("end")
                                    .help("End time (ISO 8601 format)")
                                    .takes_value(true)
                                    .required(true),
                            )
                            .arg(
                                Arg::with_name("description")
                                    .long("description")
                                    .help("Event description")
                                    .takes_value(true),
                            )
                            .arg(
                                Arg::with_name("location")
                                    .long("location")
                                    .help("Location")
                                    .takes_value(true),
                            ),
                    )
                    .subcommand(
                        SubCommand::with_name("find-free")
                            .about("Find free time slots")
                            .arg(
                                Arg::with_name("duration")
                                    .help("Duration in minutes")
                                    .required(true)
                                    .index(1),
                            )
                            .arg(
                                Arg::with_name("days")
                                    .long("days")
                                    .help("Number of days to search ahead")
                                    .takes_value(true)
                                    .default_value("7"),
                            ),
                    ),
            )
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
    local_schedule: Schedule,
    storage: Storage,
    config: Config,
    config_manager: ConfigManager,
    calendar_service: Option<CalendarService>,
    #[allow(dead_code)]
    verbose: bool,
}

impl CliApp {
    // === ヘルパーメソッド ===

    /// Google Calendar認証をチェックし、必要に応じて認証を実行
    async fn ensure_calendar_auth(&mut self) -> Result<()> {
        if self.calendar_service.is_none() {
            self.calendar_auth_command().await?;
        }
        Ok(())
    }

    /// 成功メッセージを表示
    fn print_success(&self, message: &str) {
        println!("{}", message.green());
    }

    /// エラーメッセージを表示
    fn print_error(&self, prefix: &str, error: &dyn std::fmt::Display) {
        println!("{}: {}", prefix.red(), error);
    }

    /// 警告メッセージを表示
    fn print_warning(&self, message: &str) {
        println!("{}", message.yellow());
    }

    /// 日時解析のヘルパー関数
    fn parse_datetime(
        &self,
        datetime_str: &str,
    ) -> Result<chrono::DateTime<chrono::Utc>, crate::models::SchedulerError> {
        use chrono::TimeZone;
        use chrono_tz::Asia::Tokyo;
        
        // ISO 8601形式の解析を試行
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(datetime_str) {
            return Ok(dt.with_timezone(&chrono::Utc));
        }

        // タイムゾーン付きフォーマット
        let formats_with_tz = [
            "%Y-%m-%dT%H:%M:%S%.fZ",
            "%Y-%m-%dT%H:%M:%SZ",
            "%Y-%m-%dT%H:%M:%S%z",
            "%Y-%m-%dT%H:%M:%S%.f%z",
        ];

        for format in &formats_with_tz {
            if let Ok(dt) = chrono::DateTime::parse_from_str(datetime_str, format) {
                return Ok(dt.with_timezone(&chrono::Utc));
            }
        }

        // タイムゾーンなしの形式（日本時間として解釈）
        let formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M",
            "%m/%d/%Y %H:%M:%S",
            "%m/%d/%Y %H:%M",
            "%Y年%m月%d日 %H:%M:%S",
            "%Y年%m月%d日 %H:%M",
            "%Y年%m月%d日",
            "%Y-%m-%d",
            "%m/%d/%Y",
        ];

        for format in &formats {
            if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(datetime_str, format) {
                // 日本時間として解釈してUTCに変換
                let jst_dt = Tokyo.from_local_datetime(&naive_dt).single()
                    .ok_or_else(|| crate::models::SchedulerError::ParseError(format!("日本時間への変換に失敗: {}", datetime_str)))?;
                return Ok(jst_dt.with_timezone(&chrono::Utc));
            }
            if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(datetime_str, format) {
                let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap();
                let jst_dt = Tokyo.from_local_datetime(&naive_dt).single()
                    .ok_or_else(|| crate::models::SchedulerError::ParseError(format!("日本時間への変換に失敗: {}", datetime_str)))?;
                return Ok(jst_dt.with_timezone(&chrono::Utc));
            }
        }

        Err(crate::models::SchedulerError::ParseError(format!(
            "日時の形式が認識できません。対応フォーマット例: '2025-07-01 15:30'、'2025年07月01日 15:30'、'2025-07-01T15:30:00' など: {}",
            datetime_str
        )))
    }

    /// Google Calendarイベントを表示する共通メソッド
    fn display_calendar_events(&self, events: &google_calendar3::api::Events, title: &str) {
        println!("{}", title.bold().blue());
        if let Some(items) = &events.items {
            if items.is_empty() {
                self.print_warning("予定はありません。");
            } else {
                for (i, event) in items.iter().enumerate() {
                    self.display_google_calendar_event(event, i + 1);
                }
            }
        } else {
            self.print_warning("予定はありません。");
        }
    }

    pub async fn new(verbose: bool) -> Result<Self> {
        let storage = Storage::new()?;
        let mut local_schedule = Schedule::new();

        // 設定管理を初期化
        let config_manager = ConfigManager::new()?;
        let config = config_manager.load_config()?;

        // 既存のスケジュールを読み込み
        match storage.load_schedule() {
            Ok(schedule) => {
                local_schedule = schedule;
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

        // Google Calendar初期化を試行
        if let Some(ref google_config) = config.google_calendar {
            match GoogleCalendarClient::new(
                google_config
                    .client_secret_path
                    .as_deref()
                    .unwrap_or("~/.schedule_ai_agent/client_secret.json"),
                google_config
                    .token_cache_path
                    .as_deref()
                    .unwrap_or("token_cache.json"),
            )
            .await
            {
                Ok(client) => {
                    if verbose {
                        println!("{}", "Google Calendarに接続しました。".green());
                    }
                    Some(client)
                }
                Err(e) => {
                    if verbose {
                        println!("{}: {}", "Google Calendar接続エラー".yellow(), e);
                        println!("{}", "ローカルスケジュールのみ使用します。".yellow());
                    }
                    None
                }
            }
        } else {
            if verbose {
                println!("{}", "Google Calendar設定が見つかりません。".yellow());
            }
            None
        };

        Ok(Self {
            local_schedule,
            storage,
            config,
            config_manager,
            calendar_service: None, // 初期化時はNone、必要に応じて後で初期化
            verbose,
        })
    }

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        match cli.command.as_deref() {
            Some("interactive") => {
                // interactiveコマンドもmain.rsで処理される
                Err(anyhow::anyhow!("この処理はmain.rsで処理されるべきです"))
            }
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
                todo!("Googleカレンダーに対応させる")
            }
            Some("search") => {
                if let Some(search_matches) = cli.matches.subcommand_matches("search") {
                    let query = search_matches.value_of("query").unwrap().to_string();
                    self.search_events_command(query)
                } else {
                    Err(anyhow::anyhow!("Invalid search command"))
                }
            }
            Some("stats") => self.show_statistics(),
            Some("backup") => self.backup_command(),
            Some("restore") => self.restore_command(),
            Some("conversation") => {
                if let Some(conversation_matches) = cli.matches.subcommand_matches("conversation") {
                    match conversation_matches.subcommand() {
                        ("show", _) => self.show_conversation_history(),
                        ("clear", _) => self.clear_conversation_history(),
                        ("summary", _) => self.show_conversation_summary(),
                        _ => {
                            println!("利用可能な会話履歴コマンド:");
                            println!("  show    - 会話履歴を表示");
                            println!("  clear   - 会話履歴をクリア");
                            println!("  summary  - 会話履歴の要約を表示");
                            Ok(())
                        }
                    }
                } else {
                    println!("利用可能な会話履歴コマンド:");
                    println!("  show    - 会話履歴を表示");
                    println!("  clear   - 会話履歴をクリア");
                    println!("  summary  - 会話履歴の要約を表示");
                    Ok(())
                }
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
                        ("edit", _) => self.config_show_command(),
                        _ => self.config_show_command(),
                    }
                } else {
                    self.config_show_command()
                }
            }
            Some("calendar") => {
                if let Some(calendar_matches) = cli.matches.subcommand_matches("calendar") {
                    match calendar_matches.subcommand() {
                        ("auth", _) => self.calendar_auth_command().await,
                        ("today", _) => self.calendar_today_command().await,
                        ("week", _) => self.calendar_week_command().await,
                        ("sync", _) => self.calendar_sync_command().await,
                        ("create", Some(create_matches)) => {
                            let title = create_matches.value_of("title").unwrap().to_string();
                            let start = create_matches.value_of("start").unwrap().to_string();
                            let end = create_matches.value_of("end").unwrap().to_string();
                            let description = create_matches
                                .value_of("description")
                                .map(|s| s.to_string());
                            let location =
                                create_matches.value_of("location").map(|s| s.to_string());
                            self.calendar_create_command(title, start, end, description, location)
                                .await
                        }
                        ("find-free", Some(free_matches)) => {
                            let duration = free_matches
                                .value_of("duration")
                                .unwrap()
                                .parse::<i64>()
                                .map_err(|_| anyhow::anyhow!("無効な時間です"))?;
                            let days = free_matches
                                .value_of("days")
                                .unwrap()
                                .parse::<i64>()
                                .unwrap_or(7);
                            self.calendar_find_free_command(duration, days).await
                        }
                        _ => {
                            println!("利用可能なカレンダーコマンド:");
                            println!("  auth      - Google Calendarで認証");
                            println!("  today     - 今日の予定を表示");
                            println!("  week      - 今週の予定を表示");
                            println!("  sync      - カレンダーと同期");
                            println!("  create    - イベントを作成");
                            println!("  find-free - 空き時間を検索");
                            Ok(())
                        }
                    }
                } else {
                    println!("利用可能なカレンダーコマンド:");
                    println!("  auth      - Google Calendarで認証");
                    println!("  today     - 今日の予定を表示");
                    println!("  week      - 今週の予定を表示");
                    println!("  sync      - カレンダーと同期");
                    println!("  create    - イベントを作成");
                    println!("  find-free - 空き時間を検索");
                    Ok(())
                }
            }
            None => {
                anyhow::bail!("コマンドが指定されていません。`schedule-ai --help`でヘルプを表示してください。");
            }
            _ => Err(anyhow::anyhow!("Unknown command")),
        }
    }

    // カレンダー関連のコマンド実装
    /// Google Calendarで認証
    async fn calendar_auth_command(&mut self) -> Result<()> {
        println!("{}", "Google Calendarで認証中...".blue());

        // 設定から認証情報のパスを取得
        let client_secret_path = self
            .config
            .google_calendar
            .as_ref()
            .and_then(|gc| gc.client_secret_path.as_ref())
            .ok_or_else(|| anyhow::anyhow!("client_secret_pathが設定されていません"))?;
        let token_cache_path = self
            .config
            .google_calendar
            .as_ref()
            .and_then(|gc| gc.token_cache_path.as_ref())
            .ok_or_else(|| anyhow::anyhow!("token_cache_pathが設定されていません"))?;

        match CalendarService::new(client_secret_path, token_cache_path).await {
            Ok(service) => {
                self.calendar_service = Some(service);
                println!("{}", "Google Calendarの認証が完了しました！".green());
            }
            Err(e) => {
                println!("{}: {}", "認証エラー".red(), e);
                println!("設定ファイルのclient_secret_pathとtoken_cache_pathを確認してください。");
            }
        }

        Ok(())
    }
    
    /// 今日の予定を表示
    async fn calendar_today_command(&mut self) -> Result<()> {
        self.ensure_calendar_auth().await?;

        if let Some(service) = &self.calendar_service {
            match service.get_today_events().await {
                Ok(events) => {
                    self.display_calendar_events(&events, "📅 今日のGoogle Calendarの予定");
                }
                Err(e) => {
                    self.print_error("エラー", &e);
                }
            }
        }

        Ok(())
    }

    /// 今週の予定を表示
    async fn calendar_week_command(&mut self) -> Result<()> {
        self.ensure_calendar_auth().await?;

        if let Some(service) = &self.calendar_service {
            match service.get_week_events().await {
                Ok(events) => {
                    if let Some(items) = &events.items {
                        if items.is_empty() {
                            self.print_warning("今週の予定はありません。");
                        } else {
                            println!("{}", "📅 今週のGoogle Calendarの予定".bold().blue());
                            println!("予定数: {} 件\n", items.len());
                            for (i, event) in items.iter().enumerate() {
                                self.display_google_calendar_event(event, i + 1);
                            }
                        }
                    } else {
                        self.print_warning("今週の予定はありません。");
                    }
                }
                Err(e) => {
                    self.print_error("エラー", &e);
                }
            }
        }

        Ok(())
    }

    /// カレンダーと同期
    async fn calendar_sync_command(&mut self) -> Result<()> {
        self.ensure_calendar_auth().await?;

        if let Some(service) = &self.calendar_service {
            println!("{}", "📊 カレンダー情報を同期中...".blue());
            match service.display_calendar_summary().await {
                Ok(_) => {
                    self.print_success("同期が完了しました！");
                }
                Err(e) => {
                    self.print_error("同期エラー", &e);
                }
            }
        }

        Ok(())
    }

    /// イベントを作成
    async fn calendar_create_command(
        &mut self,
        title: String,
        start: String,
        end: String,
        description: Option<String>,
        location: Option<String>,
    ) -> Result<()> {
        self.ensure_calendar_auth().await?;

        if let Some(service) = &self.calendar_service {
            // 日時文字列をパース
            let start_time = chrono::DateTime::parse_from_rfc3339(&start)
                .map_err(|_| anyhow::anyhow!("無効な開始時刻フォーマット: {}", start))?
                .with_timezone(&chrono::Utc);
            let end_time = chrono::DateTime::parse_from_rfc3339(&end)
                .map_err(|_| anyhow::anyhow!("無効な終了時刻フォーマット: {}", end))?
                .with_timezone(&chrono::Utc);

            println!("{}", "📝 Google Calendarにイベントを作成中...".blue());
            match service
                .create_event(
                    &title,
                    description.as_deref(),
                    location.as_deref(),
                    start_time,
                    end_time,
                )
                .await
            {
                Ok(event) => {
                    self.print_success("イベントが作成されました！");
                    if let Some(summary) = &event.summary {
                        println!("タイトル: {}", summary);
                    }
                    if let Some(event_id) = &event.id {
                        println!("ID: {}", event_id);
                    }
                }
                Err(e) => {
                    self.print_error("作成エラー", &e);
                }
            }
        }

        Ok(())
    }

    /// 空き時間を検索
    async fn calendar_find_free_command(
        &mut self,
        duration_minutes: i64,
        days_ahead: i64,
    ) -> Result<()> {
        self.ensure_calendar_auth().await?;

        if let Some(service) = &self.calendar_service {
            let now_jst = chrono::Utc::now().with_timezone(&Tokyo);
            let end_time_jst = now_jst + chrono::Duration::days(days_ahead);

            println!(
                "{}",
                format!("🔍 {}分間の空き時間を検索中...", duration_minutes).blue()
            );
            match service
                .find_free_time(now_jst.with_timezone(&chrono::Utc), end_time_jst.with_timezone(&chrono::Utc), duration_minutes)
                .await
            {
                Ok(free_slots) => {
                    if free_slots.is_empty() {
                        self.print_warning("指定した期間に空き時間が見つかりませんでした。");
                    } else {
                        println!("{}", "=== 空き時間 ===".bold().green());
                        for (i, (start, end)) in free_slots.iter().enumerate() {
                            let start_jst = start.with_timezone(&Tokyo);
                            let end_jst = end.with_timezone(&Tokyo);
                            println!(
                                "{}. {} ～ {} ({}分間)",
                                i + 1,
                                start_jst.format("%Y-%m-%d %H:%M"),
                                end_jst.format("%Y-%m-%d %H:%M"),
                                (*end - *start).num_minutes()
                            );
                        }
                    }
                }
                Err(e) => {
                    self.print_error("検索エラー", &e);
                }
            }
        }

        Ok(())
    }

    /// Google Calendarのイベントを表示
    fn display_google_calendar_event(&self, event: &google_calendar3::api::Event, index: usize) {
        println!("\n--- イベント {} ---", index);

        if let Some(id) = &event.id {
            println!("🆔 ID: {}", id.yellow());
        }

        if let Some(summary) = &event.summary {
            println!("📋 タイトル: {}", summary.green());
        }

        if let Some(start) = &event.start {
            if let Some(date_time) = &start.date_time {
                println!("🕐 開始時刻: {}", date_time.to_string().blue());
            } else if let Some(date) = &start.date {
                println!("📅 開始日: {}", date.to_string().blue());
            }
        }

        if let Some(end) = &event.end {
            if let Some(date_time) = &end.date_time {
                println!("🕐 終了時刻: {}", date_time.to_string().blue());
            } else if let Some(date) = &end.date {
                println!("📅 終了日: {}", date.to_string().blue());
            }
        }

        if let Some(description) = &event.description {
            println!("📝 説明: {}", description);
        }

        if let Some(location) = &event.location {
            println!("📍 場所: {}", location.cyan());
        }
    }


    fn get_context_info(&self) -> String {
        let stats = self.get_local_statistics();
        let upcoming = self.get_local_upcoming_events(3);

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
        priority_str: Option<String>, // 変数名を変更
    ) -> Result<()> {
        let priority = match priority_str.as_deref() {
            Some("low") => Some(Priority::Low),
            Some("medium") => Some(Priority::Medium),
            Some("high") => Some(Priority::High),
            Some("urgent") => Some(Priority::Urgent),
            _ => None, // デフォルト値をNoneにするか、LLMに任せる
        };

        let event_data = crate::models::EventData {
            id: None,
            title: Some(title),
            description,
            start_time: Some(start),
            end_time: Some(end),
            location,
            attendees: Vec::new(),
            priority,
            max_results: None,
        };

        match self.create_local_event(event_data) {
            Ok(event_id) => {
                self.print_success("予定を作成しました。");
                println!("イベントID: {}", event_id.to_string().cyan());
                self.save_schedule()?;
            }
            Err(e) => {
                self.print_error("エラー", &e);
            }
        }

        Ok(())
    }

    fn search_events_command(&self, query: String) -> Result<()> {
        let events = self.search_local_events(&query);

        if events.is_empty() {
            self.print_warning(&format!(
                "「{}」に一致する予定が見つかりませんでした。",
                query
            ));
        } else {
            println!("{}", format!("=== 検索結果: {} ===", query).bold().blue());
            self.display_events_list(events);
        }

        Ok(())
    }

    fn show_statistics(&self) -> Result<()> {
        let stats = self.get_local_statistics();

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
                self.print_success("バックアップを作成しました。");
                println!("ファイル: {}", backup_path.display().to_string().cyan());
            }
            Err(e) => {
                self.print_error("バックアップエラー", &e);
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
                event
                    .start_time
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
                    .green(),
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

    // ヘルパーメソッド
    fn create_local_event(&mut self, event_data: crate::models::EventData) -> Result<uuid::Uuid> {
        use crate::models::Event;

        let title = event_data
            .title
            .clone()
            .ok_or_else(|| anyhow::anyhow!("タイトルが不足しています"))?;
        let start_time_str = event_data
            .start_time
            .clone()
            .ok_or_else(|| anyhow::anyhow!("開始時刻が不足しています"))?;
        let end_time_str = event_data
            .end_time
            .clone()
            .ok_or_else(|| anyhow::anyhow!("終了時刻が不足しています"))?;

        let start_time = self.parse_datetime(&start_time_str)?;
        let end_time = self.parse_datetime(&end_time_str)?;

        if end_time <= start_time {
            return Err(anyhow::anyhow!(
                "終了時刻は開始時刻より後である必要があります"
            ));
        }

        // 重複チェック
        if self.local_schedule.has_conflict(&start_time, &end_time) {
            return Err(anyhow::anyhow!("指定された時間帯に既に予定があります"));
        }

        let mut event = Event::new(title, start_time, end_time);
        event.apply_event_data(event_data, |s| self.parse_datetime(s))?;

        let event_id = event.id;
        self.local_schedule.add_event(event);

        Ok(event_id)
    }

    fn save_schedule(&self) -> Result<()> {
        self.storage.save_schedule(&self.local_schedule)
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
                self.print_success("設定ファイルを作成しました:");
                for file in files {
                    println!("  {}", file.display().to_string().cyan());
                }
                self.print_warning("設定ファイルを編集してAPIキーを設定してください。");
            }
            Err(e) => {
                self.print_error("設定ファイル作成エラー", &e);
            }
        }

        Ok(())
    }

    fn config_show_command(&self) -> Result<()> {
        println!("{}", "=== 現在の設定 ===".bold().blue());

        // LLM設定
        println!("{}", "LLM設定:".bold());
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
        let has_gemini_api_key = self.config.llm.gemini_api_key.is_some();
        println!(
            "  Gemini API Key: {}",
            if has_gemini_api_key {
                "設定済み".green()
            } else {
                "未設定".red()
            }
        );

        Ok(())
    }

    fn config_path_command(&self) -> Result<()> {
        println!("{}", "=== 設定ファイルパス ===".bold().blue());
        println!(
            "設定ディレクトリ: {}",
            self.config_manager
                .get_config_directory_path()
                .display()
                .to_string()
                .cyan()
        );
        println!(
            "設定ファイル: {}",
            self.config_manager
                .get_config_file_path()
                .display()
                .to_string()
                .cyan()
        );
        Ok(())
    }

    fn show_conversation_history(&self) -> Result<()> {
        let conversation = self.storage.load_conversation_history()?;
        if conversation.messages.is_empty() {
            println!("会話履歴はありません。");
            return Ok(());
        }

        println!("=== 会話履歴 ===");
        for (i, message) in conversation.messages.iter().enumerate() {
            let role = match message.role {
                crate::models::MessageRole::User => "ユーザー",
                crate::models::MessageRole::Assistant => "アシスタント", 
                crate::models::MessageRole::System => "システム",
            };
            println!("{}. [{}] {}: {}", 
                i + 1, 
                message.timestamp.format("%Y-%m-%d %H:%M:%S"),
                role, 
                message.content
            );
        }
        Ok(())
    }

    fn clear_conversation_history(&self) -> Result<()> {
        self.storage.clear_conversation_history()?;
        println!("会話履歴をクリアしました。");
        Ok(())
    }

    fn show_conversation_summary(&self) -> Result<()> {
        let conversation = self.storage.load_conversation_history()?;
        if conversation.messages.is_empty() {
            println!("会話履歴はありません。");
            return Ok(());
        }

        let recent_messages = conversation.get_recent_messages(10);
        println!("=== 会話履歴の要約 (最新{}件) ===", recent_messages.len());
        println!("総メッセージ数: {}", conversation.messages.len());
        println!("最初の会話: {}", conversation.created_at.format("%Y-%m-%d %H:%M:%S"));
        println!("最後の更新: {}", conversation.updated_at.format("%Y-%m-%d %H:%M:%S"));
        
        println!("\n最近の会話:");
        for message in recent_messages {
            let role = match message.role {
                crate::models::MessageRole::User => "ユーザー",
                crate::models::MessageRole::Assistant => "アシスタント",
                crate::models::MessageRole::System => "システム",
            };
            println!("- [{}] {}: {}", 
                message.timestamp.format("%m/%d %H:%M"),
                role, 
                if message.content.len() > 50 {
                    format!("{}...", &message.content[..50])
                } else {
                    message.content.clone()
                }
            );
        }
        Ok(())
    }

    fn get_local_statistics(&self) -> crate::scheduler::ScheduleStatistics {
        let schedule = match self.storage.load_schedule() {
            Ok(schedule) => schedule,
            Err(_) => return crate::scheduler::ScheduleStatistics {
                total_events: 0,
                upcoming_events: 0,
                past_events: 0,
                low_priority: 0,
                medium_priority: 0,
                high_priority: 0,
                urgent_priority: 0,
            },
        };

        let now = chrono::Utc::now();
        let total_events = schedule.events.len();
        let upcoming_events = schedule.events.iter().filter(|e| e.start_time > now).count();
        let past_events = schedule.events.iter().filter(|e| e.end_time < now).count();

        let low_priority = schedule.events.iter().filter(|e| matches!(e.priority, crate::models::Priority::Low)).count();
        let medium_priority = schedule.events.iter().filter(|e| matches!(e.priority, crate::models::Priority::Medium)).count();
        let high_priority = schedule.events.iter().filter(|e| matches!(e.priority, crate::models::Priority::High)).count();
        let urgent_priority = schedule.events.iter().filter(|e| matches!(e.priority, crate::models::Priority::Urgent)).count();

        crate::scheduler::ScheduleStatistics {
            total_events,
            upcoming_events,
            past_events,
            low_priority,
            medium_priority,
            high_priority,
            urgent_priority,
        }
    }


    /// 直近のイベントを取得
    fn get_local_upcoming_events(&self, limit: usize) -> Vec<&crate::models::Event> {
        let now = chrono::Utc::now();
        let mut upcoming_events: Vec<&crate::models::Event> = self.local_schedule.events
            .iter()
            .filter(|event| event.start_time > now)
            .collect();
        
        // 開始時刻でソート
        upcoming_events.sort_by(|a, b| a.start_time.cmp(&b.start_time));
        
        // 指定された件数まで取得
        upcoming_events.into_iter().take(limit).collect()
    }

    /// ローカルイベントを検索
    fn search_local_events(&self, query: &str) -> Vec<&crate::models::Event> {
        let query_lower = query.to_lowercase();
        
        self.local_schedule.events
            .iter()
            .filter(|event| {
                // タイトル、説明、場所で検索
                event.title.to_lowercase().contains(&query_lower) ||
                event.description.as_ref().map_or(false, |desc| desc.to_lowercase().contains(&query_lower)) ||
                event.location.as_ref().map_or(false, |loc| loc.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}
