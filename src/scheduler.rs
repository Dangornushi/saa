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
        println!("{} {}", "ユーザー入力:".cyan(), user_input);

        let request = LLMRequest {
            user_input: user_input.clone(),
            context: Some(self.create_context()),
            conversation_history: Some(self.conversation_history.clone()),
        };

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
                Ok("予定の削除は現在サポートされていません。Google Calendarから直接削除してください。".to_string())
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

        match result {
            Ok(msg) => {
                // 成功時のメッセージも会話履歴に追加
                if !response.response_text.is_empty() {
                    return Ok(response.response_text);
                }
                Ok(msg)
            }
            Err(e) => {
                let error_msg = format!("エラーが発生しました: {}", e);
                self.conversation_history.add_assistant_message(error_msg.clone(), None);
                self.save_conversation_history()?;
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
        if self.conversation_history.messages.is_empty() {
            "会話履歴はありません。".to_string()
        } else {
            let total_messages = self.conversation_history.messages.len();
            let user_messages = self.conversation_history.messages.iter()
                .filter(|msg| matches!(msg.role, crate::models::MessageRole::User))
                .count();
            let assistant_messages = self.conversation_history.messages.iter()
                .filter(|msg| matches!(msg.role, crate::models::MessageRole::Assistant))
                .count();

            let recent_messages = self.conversation_history.get_recent_messages(10);
            
            let mut summary = format!(
                "📊 会話統計:\n  • 総メッセージ数: {}\n  • ユーザーメッセージ: {}\n  • アシスタントメッセージ: {}\n\n",
                total_messages, user_messages, assistant_messages
            );
            
            if !recent_messages.is_empty() {
                summary.push_str(&format!("💬 最近の会話 (最新{}件):\n", recent_messages.len()));
                for (i, msg) in recent_messages.iter().enumerate() {
                    let role_icon = match msg.role {
                        crate::models::MessageRole::User => "👤",
                        crate::models::MessageRole::Assistant => "🤖",
                        crate::models::MessageRole::System => "⚙️",
                    };
                    let role_name = match msg.role {
                        crate::models::MessageRole::User => "ユーザー",
                        crate::models::MessageRole::Assistant => "アシスタント",
                        crate::models::MessageRole::System => "システム",
                    };
                    
                    // メッセージが長い場合は省略
                    let content = if msg.content.len() > 100 {
                        format!("{}...", &msg.content[..97])
                    } else {
                        msg.content.clone()
                    };
                    
                    summary.push_str(&format!("  {}. {} {}: {}\n", 
                        recent_messages.len() - i, role_icon, role_name, content));
                }
            }
            
            summary
        }
    }

    fn save_conversation_history(&self) -> Result<()> {
        self.storage.save_conversation_history(&self.conversation_history)
    }

    fn create_context(&self) -> String {
        let mut context = String::new();
        
        // Google Calendarが設定されている場合の情報を追加
        if self.calendar_client.is_some() {
            context.push_str("Google Calendar連携が有効です。\n");
        } else {
            context.push_str("Google Calendar連携は無効です。\n");
        }
        
        context
    }

    async fn get_list_events(&mut self, response: &LLMResponse) -> Result<String> {
        let (query_start, query_end) = self.get_query_time_range(&response);
        let query_range_str = format!(
            "📅 {}から{}までの予定",
            query_start.format("%Y年%m月%d日 %H:%M"),
            query_end.format("%Y年%m月%d日 %H:%M")
        );

        // Google Calendarから予定を取得
        if let Some(ref google_calendar) = self.calendar_client {
            match google_calendar
                .get_events_in_range("primary", query_start, query_end, 50)
                .await
            {
                Ok(events) => {
                    self.display_calendar_events(&events, &query_range_str);
                }
                Err(e) => {
                    println!("{}: {}", "Google Calendar取得エラー".red(), e);
                }
            }
        } else {
            println!("{}", "Google Calendarが設定されていません。".yellow());
        }

        Ok("OK".to_string())
    }
    // カレンダー関連のコマンド実装
    
    /// Google Calendarイベントを表示する共通メソッド
    fn display_calendar_events(&self, events: &google_calendar3::api::Events, title: &str) {
        println!("{}", title.bold().blue());
        if let Some(items) = &events.items {
            if items.is_empty() {
                println!("{}", "予定はありません。".yellow());
            } else {
                for (i, event) in items.iter().enumerate() {
                    self.display_google_calendar_event(event, i + 1);
                }
            }
        } else {
            println!("{}", "予定はありません。".yellow());
        }
    }

    /// Google Calendarのイベントを表示
    fn display_google_calendar_event(&self, event: &google_calendar3::api::Event, index: usize) {
        println!("\n--- イベント {} ---", index);

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
    fn get_query_time_range(&self, response: &LLMResponse) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        
        // LLMのレスポンスから時間範囲を判定
        if let (Some(start), Some(end)) = (response.start_time, response.end_time) {
            return (start, end);
        }
        else {
            println!("時間範囲が指定されていません。");
            return (Utc::now(), Utc::now());
        }
    }

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

    fn parse_datetime(&self, datetime_str: &str) -> Result<DateTime<Utc>, SchedulerError> {
        // ISO 8601形式での解析を試行
        match DateTime::parse_from_rfc3339(datetime_str) {
            std::result::Result::Ok(dt) => return std::result::Result::Ok(dt.with_timezone(&Utc)),
            _ => {}
        }

        // その他の形式も試行
        match DateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M:%S%.fZ") {
            std::result::Result::Ok(dt) => return std::result::Result::Ok(dt.with_timezone(&Utc)),
            _ => {}
        }

        match DateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M:%SZ") {
            std::result::Result::Ok(dt) => return std::result::Result::Ok(dt.with_timezone(&Utc)),
            _ => {}
        }

        std::result::Result::Err(SchedulerError::ParseError(format!(
            "日時の解析に失敗しました: {}",
            datetime_str
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
        if let Some(ref calendar_client) = self.calendar_client {
            let events = calendar_client.get_primary_events(50).await?;
            
            if let Some(google_events) = events.items {
                let sync_messages: Vec<String> = google_events
                    .iter()
                    .filter_map(|google_event| {
                        google_event.summary.as_ref().map(|summary| format!("• {}", summary))
                    })
                    .collect();

                if !sync_messages.is_empty() {
                    Ok(format!(
                        "Google Calendarから {} 件の予定を確認しました:\n{}",
                        sync_messages.len(),
                        sync_messages.join("\n")
                    ))
                } else {
                    Ok("Google Calendarに予定が見つかりませんでした。".to_string())
                }
            } else {
                Ok("Google Calendarに予定が見つかりませんでした。".to_string())
            }
        } else {
            Err(anyhow::anyhow!("Google Calendarクライアントが設定されていません"))
        }
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