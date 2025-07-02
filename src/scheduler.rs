use crate::llm::LLM;
use crate::models::{
    ActionType, ConversationHistory, EventData, LLMRequest, LLMResponse, SchedulerError
};
use crate::storage::Storage;
use schedule_ai_agent::GoogleCalendarClient;
use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_tz::Asia::Tokyo;
use colored::Colorize;
use std::sync::Arc;

pub struct Scheduler {
    conversation_history: ConversationHistory,
    llm: Arc<dyn LLM>,
    storage: Storage,
    calendar_client: Option<GoogleCalendarClient>,
}

impl Scheduler {
    pub fn new(llm: Arc<dyn LLM>) -> Result<Self> {
        let storage = Storage::new()?;
        let conversation_history = storage.load_conversation_history()?;

        Ok(Self {
            conversation_history,
            llm,
            storage,
            calendar_client: None,
        })
    }


    /// 日時解析のヘルパー関
    pub async fn new_with_calendar(llm: Arc<dyn LLM>, client_secret_path: &str, token_cache_path: &str) -> Result<Self> {
        let storage = Storage::new()?;
        let conversation_history = storage.load_conversation_history()?;
        
        let calendar_client = GoogleCalendarClient::new(client_secret_path, token_cache_path).await?;

        Ok(Self {
            conversation_history,
            llm,
            storage,
            calendar_client: Some(calendar_client),
        })
    }

    pub async fn process_user_input(&mut self, user_input: String) -> Result<String> {

        // llmへのリクエストを作成
        let request = LLMRequest {
            user_input: user_input.clone(),
            context: Some(self.create_context()),
            conversation_history: Some(self.conversation_history.clone()),
        };

        // llmにリクエストを送信
        // llmからの応答を待機
        let response = self.llm.process_request(request).await?;

        // 会話履歴を更新
        if let Some(updated_conversation) = response.updated_conversation.clone() {
            self.conversation_history = updated_conversation;
            self.save_conversation_history()?;
        }

        // アクションに基づいて処理を実行
        let result = match response.action {
            ActionType::CreateEvent => {
                if let Some(event_data) = response.event_data {
                    self.create_event_from_data(event_data).await
                } else {
                    Ok("イベントデータが不足しています。".to_string())
                }
            }
            ActionType::UpdateEvent => {
                Ok("予定の更新は現在サポートされていません。予定を削除してから新しく作成してください。".to_string())
            }
            ActionType::DeleteEvent => {
                if let Some(event_data) = response.event_data {
                self.delete_event(event_data).await
                    .map(|_| "予定を削除しました。".to_string())
                    .map_err(|e| anyhow::anyhow!(e))
                } else {
                    println!("イベントデータがありません。");
                    Ok("イベントデータが不足しています。".to_string())
                }
            }
            ActionType::ListEvents => {
                self.get_list_events(&response).await
            }
            ActionType::SearchEvents => {
                Ok("ローカルスケジュールは削除されました。Google Calendarから予定を検索してください。".to_string())
            }
            ActionType::GetEventDetails => {
                Ok("ローカルスケジュールは削除されました。Google Calendarから予定の詳細を確認してください。".to_string())
            }
            ActionType::GeneralResponse => {
                Ok(response.response_text.clone())
            }
        };

        // 成功時はresponse_textがあればそれを、なければ処理結果を返す
        match result {
            Ok(msg) => {
                if !response.response_text.is_empty() {
                    Ok(response.response_text)
                } else {
                    Ok(msg)
                }
            }
            Err(e) => {
                let error_msg = format!("エラーが発生しました: {}", e);
                // エラーメッセージを会話履歴に追加（失敗しても処理を続行）
                let _ = self.conversation_history.add_assistant_message(error_msg.clone(), None);
                let _ = self.save_conversation_history();
                Ok(error_msg)
            }
        }
    }

    pub fn clear_conversation_history(&mut self) -> Result<()> {
        self.conversation_history.clear();
        self.storage.clear_conversation_history()?;
        Ok(())
    }

    pub fn get_conversation_summary(&self) -> String {
        let messages = &self.conversation_history.messages;
        
        if messages.is_empty() {
            return "会話履歴はありません。".to_string();
        }

        let total_messages = messages.len();
        let user_messages = messages.iter()
            .filter(|msg| matches!(msg.role, crate::models::MessageRole::User))
            .count();
        let assistant_messages = total_messages - user_messages; // システムメッセージは稀なので簡略化

        let mut summary = format!(
            "📊 会話統計:\n  • 総メッセージ数: {}\n  • ユーザーメッセージ: {}\n  • アシスタントメッセージ: {}\n\n",
            total_messages, user_messages, assistant_messages
        );
        
        // 最新の会話を表示
        let recent_messages = self.conversation_history.get_recent_messages(10);
        if !recent_messages.is_empty() {
            summary.push_str(&format!("💬 最近の会話 (最新{}件):\n", recent_messages.len()));
            
            for (i, msg) in recent_messages.iter().enumerate() {
                let (icon, name) = match msg.role {
                    crate::models::MessageRole::User => ("👤", "ユーザー"),
                    crate::models::MessageRole::Assistant => ("🤖", "アシスタント"),
                    crate::models::MessageRole::System => ("⚙️", "システム"),
                };
                
                // 長いメッセージは省略
                let content = if msg.content.len() > 100 {
                    format!("{}...", &msg.content[..97])
                } else {
                    msg.content.clone()
                };
                
                summary.push_str(&format!(
                    "  {}. {} {}: {}\n", 
                    recent_messages.len() - i, icon, name, content
                ));
            }
        }
        
        summary
    }

    fn save_conversation_history(&self) -> Result<()> {
        self.storage.save_conversation_history(&self.conversation_history)
    }

    fn create_context(&self) -> String {
        if self.calendar_client.is_some() {
            "Google Calendar連携が有効です。\n".to_string()
        } else {
            "Google Calendar連携は無効です。\n".to_string()
        }
    }

    async fn get_list_events(&mut self, response: &LLMResponse) -> Result<String> {
        let (query_start, query_end) = self.get_query_time_range(&response);
        let query_range_str = format!(
            "📅 {}から{}までの予定",
            query_start.format("%Y年%m月%d日 %H:%M"),
            query_end.format("%Y年%m月%d日 %H:%M")
        );

        // Google Calendarから予定を取得
        match &self.calendar_client {
            Some(google_calendar) => {
                match google_calendar.get_events_in_range("primary", query_start, query_end, 50).await {
                    Ok(events) => self.display_calendar_events(&events, &query_range_str),
                    Err(e) => println!("{}: {}", "Google Calendar取得エラー".red(), e),
                }
            }
            None => println!("{}", "Google Calendarが設定されていません。".yellow()),
        }

        Ok("OK".to_string())
    }
    
    // カレンダー関連のコマンド実装 

    /// Google Calendarイベントを表示する共通メソッド
    fn display_calendar_events(&self, events: &google_calendar3::api::Events, title: &str) {
        println!("{}", title.bold().blue());
        
        match &events.items {
            Some(items) if !items.is_empty() => {
                for (i, event) in items.iter().enumerate() {
                    self.display_google_calendar_event(event, i + 1);
                }
            }
            _ => println!("{}", "予定はありません。".yellow()),
        }
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
                let start_jst = date_time.with_timezone(&Tokyo);
                println!("🕐 開始時刻: {}", start_jst.format("%Y-%m-%d %H:%M").to_string().blue());
            } else if let Some(date) = &start.date {
                println!("📅 開始日: {}", date.to_string().blue());
            }
        }

        if let Some(end) = &event.end {
            if let Some(date_time) = &end.date_time {
                let end_jst = date_time.with_timezone(&Tokyo);
                println!("🕐 終了時刻: {}", end_jst.format("%Y-%m-%d %H:%M").to_string().blue());
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

    /// クエリの時間範囲を取得
    fn get_query_time_range(&self, response: &LLMResponse) -> (DateTime<Utc>, DateTime<Utc>) {
        // LLMのレスポンスから時間範囲を取得、なければ現在時刻を返す
        match (response.start_time, response.end_time) {
            (Some(start), Some(end)) => (start, end),
            _ => {
                println!("時間範囲が指定されていません。");
                (Utc::now(), Utc::now())
            }
        }
    }

    // Googleカレンダーにイベントを新規作成
    async fn create_event_from_data(&mut self, event_data: EventData) -> Result<String> {
        // 必要な情報が揃っているかチェック
        let title = event_data.title.as_ref()
            .ok_or_else(|| SchedulerError::ValidationError("タイトルが必要です".to_string()))?;

        let start_time_str = event_data.start_time.as_ref()
            .ok_or_else(|| SchedulerError::ValidationError("開始時刻が必要です".to_string()))?;

        let end_time_str = event_data.end_time.as_ref()
            .ok_or_else(|| SchedulerError::ValidationError("終了時刻が必要です".to_string()))?;

        let start_time = self.parse_datetime(start_time_str)?;
        let end_time = self.parse_datetime(end_time_str)?;

        // Google Calendarにイベントを作成する
        if let Some(ref calendar_client) = self.calendar_client {
            match calendar_client.create_event_from_event_data(
                title,
                start_time_str,
                end_time_str,
                event_data.description.as_deref(),
                event_data.location.as_deref(),
            ).await {
                Ok(_id) => {
                    println!("Google Calendarにイベントを作成しました: {}", title);
                }
                Err(e) => {
                    println!("Google Calendarへの作成に失敗しました: {}", e);
                    return Err(e.into());
                }
            }
        } else {
            return Err(anyhow::anyhow!("Google Calendarクライアントが設定されていません"));
        }

        // 会話履歴にイベント作成の記録を追加
        let success_message = format!("予定「{}」をGoogle Calendarに作成しました", title);
        
        self.conversation_history.add_assistant_message(
            success_message.clone(),
            Some(uuid::Uuid::new_v4()),
        );
        self.save_conversation_history()?;

        Ok(format!(
            "{}。\n開始: {}\n終了: {}",
            success_message,
            start_time.with_timezone(&Tokyo).format("%Y-%m-%d %H:%M"),
            end_time.with_timezone(&Tokyo).format("%Y-%m-%d %H:%M")
        ))
    }

    // Googleカレンダーのイベントを削除
    async fn delete_event(&mut self, event_data: EventData) -> Result<(), String> {
        // Google Calendarイベントの削除
        if let Some(ref calendar_client) = self.calendar_client {
            // イベントIDが指定されている場合
            if let Some(event_id) = &event_data.id {
                println!("Google CalendarからイベントIDで削除中: {}", event_id);
                calendar_client.delete_event("primary", event_id).await
                    .map_err(|e| format!("Google Calendarからの削除に失敗しました: {}", e))?;
                println!("Google Calendarからイベントを削除しました: {}", event_id);
            } else if let Some(title) = &event_data.title {
                // タイトルで検索して削除（従来の方法）
                println!("タイトルでイベントを検索して削除中: {}", title);
                // 今日の予定から該当するタイトルのイベントを検索
                match calendar_client.get_primary_events(50).await {
                    Ok(events) => {
                        if let Some(items) = events.items {
                            if let Some(event) = items.iter().find(|e| {
                                e.summary.as_ref().map_or(false, |s| s.contains(title))
                            }) {
                                if let Some(event_id) = &event.id {
                                    calendar_client.delete_event("primary", event_id).await
                                        .map_err(|e| format!("Google Calendarからの削除に失敗しました: {}", e))?;
                                    println!("Google Calendarからイベントを削除しました: {}", title);
                                } else {
                                    return Err("イベントIDが見つかりません".to_string());
                                }
                            } else {
                                return Err(format!("該当するイベントが見つかりません: {}", title));
                            }
                        } else {
                            return Err("イベントが見つかりません".to_string());
                        }
                    }
                    Err(e) => return Err(format!("イベント検索に失敗しました: {}", e)),
                }
            } else {
                return Err("削除対象のイベントIDまたはタイトルが必要です".to_string());
            }
        } else {
            return Err("Google Calendarクライアントが設定されていません。".to_string());
        }
        
        self.save_conversation_history().unwrap();
        Ok(())
    }
    fn parse_datetime(&self, datetime_str: &str) -> Result<DateTime<Utc>, SchedulerError> {
        use chrono::{NaiveDateTime, TimeZone};
        
        // RFC3339形式を最初に試行（タイムゾーン付き）
        if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
            return Ok(dt.with_timezone(&Utc));
        }
        
        // ISO 8601形式（タイムゾーン付き）
        let formats_with_tz = [
            "%Y-%m-%dT%H:%M:%S%.fZ",    // ISO 8601 with fractional seconds
            "%Y-%m-%dT%H:%M:%SZ",       // ISO 8601 basic
            "%Y-%m-%dT%H:%M:%S%z",      // with timezone offset
            "%Y-%m-%dT%H:%M:%S%.f%z",   // with fractional seconds and timezone
        ];
        
        for format in &formats_with_tz {
            if let Ok(dt) = DateTime::parse_from_str(datetime_str, format) {
                return Ok(dt.with_timezone(&Utc));
            }
        }
        
        // タイムゾーンなしの形式（日本時間として解釈）
        let formats_naive = [
            "%Y-%m-%d %H:%M:%S",        // 2025-07-01 15:30:00
            "%Y-%m-%d %H:%M",           // 2025-07-01 15:30
            "%Y-%m-%dT%H:%M:%S",        // 2025-07-01T15:30:00
            "%Y-%m-%dT%H:%M",           // 2025-07-01T15:30
            "%m/%d/%Y %H:%M:%S",        // 07/01/2025 15:30:00
            "%m/%d/%Y %H:%M",           // 07/01/2025 15:30
            "%Y年%m月%d日 %H:%M:%S",     // 2025年07月01日 15:30:00 (日本語)
            "%Y年%m月%d日 %H:%M",        // 2025年07月01日 15:30 (日本語)
            "%Y年%m月%d日",              // 2025年07月01日 (日本語、時刻は00:00と仮定)
            "%Y-%m-%d",                 // 2025-07-01 (時刻は00:00と仮定)
            "%m/%d/%Y",                 // 07/01/2025 (時刻は00:00と仮定)
        ];
        
        for format in &formats_naive {
            if let Ok(naive_dt) = NaiveDateTime::parse_from_str(datetime_str, format) {
                // 日本時間として解釈してUTCに変換
                let jst_dt = Tokyo.from_local_datetime(&naive_dt).single()
                    .ok_or_else(|| SchedulerError::ParseError(format!("日本時間への変換に失敗: {}", datetime_str)))?;
                return Ok(jst_dt.with_timezone(&Utc));
            }
            
            // 日付のみの場合も試行
            if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(datetime_str, format) {
                let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap();
                let jst_dt = Tokyo.from_local_datetime(&naive_dt).single()
                    .ok_or_else(|| SchedulerError::ParseError(format!("日本時間への変換に失敗: {}", datetime_str)))?;
                return Ok(jst_dt.with_timezone(&Utc));
            }
        }

        Err(SchedulerError::ParseError(format!(
            "日時の解析に失敗しました。対応フォーマット例: '2025-07-01 15:30'、'2025年07月01日 15:30'、'2025-07-01T15:30:00' など: {}", datetime_str
        )))
    }

    /// 会話ログをファイルに保存する
    pub fn save_conversation_log_to_file(&self, file_path: Option<&str>) -> Result<String, SchedulerError> {
        use std::fs::File;
        use std::io::Write;
        
        let log_content = self.get_detailed_conversation_log();
        
        let file_path = match file_path {
            Some(path) => path.to_string(),
            None => {
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                format!("conversation_log_{}.txt", timestamp)
            }
        };
        
        let mut file = File::create(&file_path)?;
        file.write_all(log_content.as_bytes())?;
        
        std::result::Result::Ok(file_path)
    }
    
    /// 詳細な会話ログを取得する（ファイル保存用）
    pub fn get_detailed_conversation_log(&self) -> String {
        if self.conversation_history.messages.is_empty() {
            return "会話履歴はありません。".to_string();
        }
        
        let mut log = String::new();
        log.push_str("=== AI予定管理アシスタント 会話ログ ===\n");
        log.push_str(&format!("作成日時: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        log.push_str(&format!("総メッセージ数: {}\n\n", self.conversation_history.messages.len()));
        
        for (i, msg) in self.conversation_history.messages.iter().enumerate() {
            let role_name = match msg.role {
                crate::models::MessageRole::User => "ユーザー",
                crate::models::MessageRole::Assistant => "アシスタント", 
                crate::models::MessageRole::System => "システム",
            };
            
            log.push_str(&format!("[{}] {}: {}\n\n", i + 1, role_name, msg.content));
        }
        
        log.push_str("=== ログ終了 ===");
        log
    }

    /// Google Calendarと同期する
    pub async fn sync_with_google_calendar(&mut self) -> Result<String> {
        let calendar_client = self.calendar_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Google Calendarクライアントが設定されていません"))?;
            
        let events = calendar_client.get_primary_events(50).await?;
        
        let google_events = events.items.unwrap_or_default();
        if google_events.is_empty() {
            return Ok("Google Calendarに予定が見つかりませんでした。".to_string());
        }
        
        let sync_messages: Vec<String> = google_events
            .iter()
            .filter_map(|event| event.summary.as_ref().map(|summary| format!("• {}", summary)))
            .collect();

        Ok(format!(
            "Google Calendarから {} 件の予定を確認しました:\n{}",
            sync_messages.len(),
            sync_messages.join("\n")
        ))
    }
}

#[derive(Debug)]
pub struct ScheduleStatistics {
    pub total_events: usize,
    pub upcoming_events: usize,
    pub past_events: usize,
    pub low_priority: usize,
    pub medium_priority: usize,
    pub high_priority: usize,
    pub urgent_priority: usize,
}