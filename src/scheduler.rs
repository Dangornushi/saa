use crate::llm::LLM;
use crate::models::{
    ActionType, ConversationHistory, Event, EventData, LLMRequest, LLMResponse, Schedule, SchedulerError
};
use crate::storage::Storage;
use schedule_ai_agent::GoogleCalendarClient;
use anyhow::Result;
use chrono::{DateTime, Utc, Datelike};
use colored::Colorize;
use std::sync::Arc;

pub struct Scheduler {
    schedule: Schedule,
    conversation_history: ConversationHistory,
    llm: Arc<dyn LLM>,
    storage: Storage,
    calendar_client: Option<GoogleCalendarClient>,
}

impl Scheduler {
    pub fn new(llm: Arc<dyn LLM>) -> Result<Self> {
        let storage = Storage::new()?;
        let schedule = storage.load_schedule()?;
        let conversation_history = storage.load_conversation_history()?;

        Ok(Self {
            schedule,
            conversation_history,
            llm,
            storage,
            calendar_client: None,
        })
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

    /// 日時解析のヘルパー関
    pub async fn new_with_calendar(llm: Arc<dyn LLM>, client_secret_path: &str, token_cache_path: &str) -> Result<Self> {
        let storage = Storage::new()?;
        let schedule = storage.load_schedule()?;
        let conversation_history = storage.load_conversation_history()?;
        
        let calendar_client = GoogleCalendarClient::new(client_secret_path, token_cache_path).await?;

        Ok(Self {
            schedule,
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
                if let Some(event_data) = response.event_data {
                    self.update_event_from_data(event_data).await
                } else {
                    Ok("更新するイベントのデータが不足しています。".to_string())
                }
            }
            ActionType::DeleteEvent => {
                self.delete_event_from_input(&user_input).await
            }
            ActionType::ListEvents => {
                self.get_list_events(&response).await
            }
            ActionType::SearchEvents => {
                self.search_events(&user_input)
            }
            ActionType::GetEventDetails => {
                self.get_event_details(&user_input)
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
        
        // 現在のスケジュール概要
        if !self.schedule.events.is_empty() {
            context.push_str(&format!(
                "現在の予定数: {}\n",
                self.schedule.events.len()
            ));
            
            // 今日と明日の予定
            let now = Utc::now();
            let tomorrow = now + chrono::Duration::days(1);
            
            let today_events = self.schedule.get_events_by_date_range(
                now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
                now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc(),
            );
            
            let tomorrow_events = self.schedule.get_events_by_date_range(
                tomorrow.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
                tomorrow.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc(),
            );
            
            if !today_events.is_empty() {
                context.push_str(&format!("今日の予定: {} 件\n", today_events.len()));
            }
            if !tomorrow_events.is_empty() {
                context.push_str(&format!("明日の予定: {} 件\n", tomorrow_events.len()));
            }
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
                    self.print_error("Google Calendar取得エラー", &e);
                }
            }
        } else {
            self.print_warning("Google Calendarが設定されていません。");
        }

        Ok("OK".to_string())
    }
       // カレンダー関連のコマンド実装
    /// Google Calendarで認証
    
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

    /// Google Calendarのイベントを表示
    fn display_google_calendar_event(&self, event: &google_calendar3::api::Event, index: usize) {
        println!("\n--- イベント {} ---", index);

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


    /// クエリの時間範囲を取得
    fn get_query_time_range(&self, response: &LLMResponse) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        let now = chrono::Utc::now();
        
        // LLMのレスポンスから時間範囲を判定
        let response_text = response.response_text.to_lowercase();
        
        if response_text.contains("今日") || response_text.contains("today") {
            let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end_of_day = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
            (start_of_day, end_of_day)
        } else if response_text.contains("明日") || response_text.contains("tomorrow") {
            let tomorrow = now + chrono::Duration::days(1);
            let start_of_day = tomorrow.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end_of_day = tomorrow.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
            (start_of_day, end_of_day)
        } else if response_text.contains("今週") || response_text.contains("this week") {
            let days_from_monday = now.weekday().num_days_from_monday();
            let start_of_week = (now - chrono::Duration::days(days_from_monday as i64))
                .date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end_of_week = start_of_week + chrono::Duration::days(7);
            (start_of_week, end_of_week)
        } else {
            // デフォルトは今日から1週間
            let end_time = now + chrono::Duration::days(7);
            (now, end_time)
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

        // 重複チェック
        if self.schedule.has_conflict(&start_time, &end_time) {
            return Err(SchedulerError::Conflict(
                "指定された時間帯に既に予定があります".to_string(),
            ).into());
        }

        // Google Calendarにもイベントを作成する
        let mut google_event_id = None;
        if let Some(ref calendar_client) = self.calendar_client {
            match calendar_client.create_event_from_event_data(
                title,
                start_time_str,
                end_time_str,
                event_data.description.as_deref(),
                event_data.location.as_deref(),
            ).await {
                Ok(id) => {
                    google_event_id = Some(id);
                    println!("Google Calendarにイベントを作成しました: {}", title);
                }
                Err(e) => {
                    println!("Google Calendarへの作成に失敗しました: {}", e);
                    // Google Calendarでの作成に失敗してもローカルでは続行
                }
            }
        }

        let mut event = Event::new(title.clone(), start_time, end_time);

        if let Some(description) = event_data.description {
            event = event.with_description(description);
        }

        if let Some(location) = event_data.location {
            event = event.with_location(location);
        }

        if let Some(priority) = event_data.priority {
            event = event.with_priority(priority);
        }

        for attendee in event_data.attendees {
            event = event.add_attendee(attendee);
        }

        // Google Calendar IDがある場合は設定
        let has_google_calendar = google_event_id.is_some();
        if let Some(_google_id) = google_event_id {
            // EventにGoogle Calendar IDを保存する仕組みは後で実装
            // 現在は単純にローカルに保存
        }

        let event_id = event.id;
        self.schedule.add_event(event);
        self.storage.save_schedule(&self.schedule)?;

        // 会話履歴にイベント作成の記録を追加
        let success_message = if has_google_calendar {
            format!("予定「{}」をローカルとGoogle Calendarに作成しました", title)
        } else {
            format!("予定「{}」をローカルに作成しました", title)
        };
        
        self.conversation_history.add_assistant_message(
            success_message.clone(),
            Some(event_id),
        );
        self.save_conversation_history()?;

        Ok(format!(
            "{}。\n開始: {}\n終了: {}",
            success_message,
            start_time.format("%Y-%m-%d %H:%M"),
            end_time.format("%Y-%m-%d %H:%M")
        ))
    }

    async fn update_event_from_data(&mut self, event_data: EventData) -> Result<String> {
        // 更新対象のイベントを特定する必要がある
        // この実装では、タイトルで検索して最初に見つかったイベントを更新する
        let title_to_search = event_data.title.as_deref().unwrap_or("");
        
        let event_id = self.schedule.events
            .iter()
            .find(|e| e.title.contains(title_to_search))
            .map(|e| e.id)
            .ok_or_else(|| SchedulerError::NotFound("更新対象の予定が見つかりません".to_string()))?;

        let event_title = {
            let event = self.schedule.get_event_mut(event_id)
                .ok_or_else(|| SchedulerError::NotFound("更新対象の予定が見つかりません".to_string()))?;
            
            // クロージャ内でselfを使わないように、parse_datetimeをローカル関数として定義
            let parse_fn = |s: &str| -> Result<DateTime<Utc>, SchedulerError> {
                // ISO 8601形式での解析を試行
                match DateTime::parse_from_rfc3339(s) {
                    std::result::Result::Ok(dt) => return std::result::Result::Ok(dt.with_timezone(&Utc)),
                    _ => {}
                }

                // その他の形式も試行
                match DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ") {
                    std::result::Result::Ok(dt) => return std::result::Result::Ok(dt.with_timezone(&Utc)),
                    _ => {}
                }

                match DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ") {
                    std::result::Result::Ok(dt) => return std::result::Result::Ok(dt.with_timezone(&Utc)),
                    _ => {}
                }

                std::result::Result::Err(SchedulerError::ParseError(format!(
                    "日時の解析に失敗しました: {}",
                    s
                )))
            };
            
            event.apply_event_data(event_data, parse_fn)?;
            event.title.clone()
        };

        self.storage.save_schedule(&self.schedule)?;

        self.conversation_history.add_assistant_message(
            format!("予定「{}」を更新しました", event_title),
            Some(event_id),
        );
        self.save_conversation_history()?;

        Ok(format!("予定「{}」を更新しました", event_title))
    }

    async fn delete_event_from_input(&mut self, input: &str) -> Result<String> {
        // 入力からイベントを特定して削除
        // 簡単な実装：タイトルが含まれているイベントを検索
        let events_to_delete: Vec<_> = self.schedule.events
            .iter()
            .filter(|e| input.contains(&e.title))
            .map(|e| (e.id, e.title.clone()))
            .collect();

        if events_to_delete.is_empty() {
            return Ok("削除対象の予定が見つかりません".to_string());
        }

        let mut deleted_titles = Vec::new();
        for (event_id, title) in events_to_delete {
            if self.schedule.remove_event(event_id) {
                deleted_titles.push(title.clone());
                self.conversation_history.add_assistant_message(
                    format!("予定「{}」を削除しました", title),
                    Some(event_id),
                );
            }
        }

        if !deleted_titles.is_empty() {
            self.storage.save_schedule(&self.schedule)?;
            self.save_conversation_history()?;
            Ok(format!("以下の予定を削除しました: {}", deleted_titles.join(", ")))
        } else {
            Ok("予定の削除に失敗しました".to_string())
        }
    }

    fn list_events(&self) -> String {
        if self.schedule.events.is_empty() {
            return "予定はありません。".to_string();
        }

        let mut events = self.schedule.events.clone();
        events.sort_by(|a, b| a.start_time.cmp(&b.start_time));

        let events_str = events
            .iter()
            .map(|e| {
                format!(
                    "• {} ({}〜{})",
                    e.title,
                    e.start_time.format("%m/%d %H:%M"),
                    e.end_time.format("%H:%M")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("予定一覧 ({} 件):\n{}", events.len(), events_str)
    }

    fn search_events(&self, query: &str) -> Result<String> {
        let matching_events: Vec<_> = self.schedule.events
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&query.to_lowercase()) ||
                e.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&query.to_lowercase()))
            })
            .collect();

        if matching_events.is_empty() {
            Ok(format!("「{}」に関連する予定は見つかりませんでした", query))
        } else {
            let events_str = matching_events
                .iter()
                .map(|e| {
                    format!(
                        "• {} ({}〜{})",
                        e.title,
                        e.start_time.format("%m/%d %H:%M"),
                        e.end_time.format("%H:%M")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            Ok(format!("「{}」の検索結果 ({} 件):\n{}", query, matching_events.len(), events_str))
        }
    }

    fn get_event_details(&self, query: &str) -> Result<String> {
        let event = self.schedule.events
            .iter()
            .find(|e| e.title.to_lowercase().contains(&query.to_lowercase()))
            .ok_or_else(|| SchedulerError::NotFound("指定された予定が見つかりません".to_string()))?;

        let mut details = format!(
            "予定の詳細:\n📅 タイトル: {}\n⏰ 開始: {}\n⏰ 終了: {}\n🎯 優先度: {:?}\n📊 状態: {:?}",
            event.title,
            event.start_time.format("%Y年%m月%d日 %H:%M"),
            event.end_time.format("%Y年%m月%d日 %H:%M"),
            event.priority,
            event.status
        );

        if let Some(description) = &event.description {
            details.push_str(&format!("\n📝 説明: {}", description));
        }

        if let Some(location) = &event.location {
            details.push_str(&format!("\n📍 場所: {}", location));
        }

        if !event.attendees.is_empty() {
            details.push_str(&format!("\n👥 参加者: {}", event.attendees.join(", ")));
        }

        Ok(details)
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
                let mut sync_count = 0;
                let mut sync_messages = Vec::new();

                for google_event in google_events {
                    if let (Some(summary), Some(start), Some(end)) = (
                        google_event.summary,
                        google_event.start.and_then(|s| s.date_time),
                        google_event.end.and_then(|e| e.date_time),
                    ) {
                        // Google Calendar の日時形式をパース
                        let start_utc = start.with_timezone(&Utc);
                        let end_utc = end.with_timezone(&Utc);

                        // 既存のイベントとの重複チェック（タイトルと時刻で判定）
                        let existing = self.schedule.events.iter().any(|e| {
                            e.title == summary && 
                            e.start_time == start_utc && 
                            e.end_time == end_utc
                        });

                        if !existing {
                            let mut event = Event::new(summary.clone(), start_utc, end_utc);
                            
                            if let Some(description) = google_event.description {
                                event = event.with_description(description);
                            }
                            
                            if let Some(location) = google_event.location {
                                event = event.with_location(location);
                            }

                            self.schedule.add_event(event);
                            sync_count += 1;
                            sync_messages.push(format!("• {}", summary));
                        }
                    }
                }

                if sync_count > 0 {
                    self.storage.save_schedule(&self.schedule)?;
                    Ok(format!(
                        "Google Calendarから {} 件の新しい予定を同期しました:\n{}",
                        sync_count,
                        sync_messages.join("\n")
                    ))
                } else {
                    Ok("Google Calendarとの同期が完了しました。新しい予定はありませんでした。".to_string())
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