// デバッグ用のモジュール
pub mod debug;

use google_calendar3::{CalendarHub, oauth2, api::Event, api::Events};
use hyper_rustls::HttpsConnectorBuilder;
use oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use chrono::Utc;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Google Calendar APIクライアント
pub struct GoogleCalendarClient {
    hub: CalendarHub<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>,
}

impl GoogleCalendarClient {
    /// client_secret.jsonファイルを検索する
    fn find_client_secret_file(client_secret_path: &str) -> Result<PathBuf> {
        let path = Path::new(client_secret_path);
        
        // 絶対パスまたは相対パスとして指定されたパスが存在するかチェック
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        
        // カレントディレクトリからの相対パスで検索
        let current_dir_path = std::env::current_dir()?.join(client_secret_path);
        if current_dir_path.exists() {
            return Ok(current_dir_path);
        }
        
        // .schedule_ai_agentディレクトリで検索
        if let Ok(home_dir) = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("ホームディレクトリが見つかりません")) {
            let config_dir_path = home_dir.join(".schedule_ai_agent").join(client_secret_path);
            if config_dir_path.exists() {
                return Ok(config_dir_path);
            }
        }
        
        // プロジェクトルートディレクトリの.schedule_ai_agentディレクトリで検索
        let mut current = std::env::current_dir()?;
        loop {
            let config_dir = current.join(".schedule_ai_agent");
            if config_dir.exists() {
                let client_secret_in_config = config_dir.join(client_secret_path);
                if client_secret_in_config.exists() {
                    return Ok(client_secret_in_config);
                }
            }
            
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }
        
        // どこにも見つからない場合は元のパスを返す（エラーメッセージのため）
        Err(anyhow::anyhow!(
            "client_secret.jsonが見つかりません。以下の場所を確認してください:\n\
            1. {}\n\
            2. カレントディレクトリ\n\
            3. ~/.schedule_ai_agent/\n\
            4. プロジェクトの.schedule_ai_agentディレクトリ",
            client_secret_path
        ))
    }

    /// 新しいGoogle Calendar クライアントを作成
    pub async fn new(client_secret_path: &str, token_cache_path: &str) -> Result<Self> {
        // client_secret.jsonファイルを検索
        let actual_client_secret_path = Self::find_client_secret_file(client_secret_path)?;
        
        // HTTPSクライアントを作成
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .build();
        let client = hyper::Client::builder().build::<_, hyper::Body>(https);

        // OAuth2の秘密情報を読み込み
        let secret = oauth2::read_application_secret(&actual_client_secret_path)
            .await
            .map_err(|e| anyhow::anyhow!("client_secret.json の読み込みに失敗しました: {} (パス: {})", e, actual_client_secret_path.display()))?;

        // 認証器を作成
        let auth = InstalledFlowAuthenticator::builder(
            secret,
            InstalledFlowReturnMethod::HTTPRedirect,
        )
        .persist_tokens_to_disk(token_cache_path)
        .build()
        .await?;

        // Calendar APIのハブを作成
        let hub = CalendarHub::new(client, auth);

        Ok(Self { hub })
    }

    /// イベントを取得する
    pub async fn get_events(&self, calendar_id: &str, max_results: i32) -> Result<Events> {
        let result = self.hub
            .events()
            .list(calendar_id)
            .time_min(Utc::now())
            .max_results(max_results)
            .single_events(true)
            .order_by("startTime")
            .doit()
            .await?;

        Ok(result.1)
    }

    /// プライマリカレンダーのイベントを取得する
    pub async fn get_primary_events(&self, max_results: i32) -> Result<Events> {
        self.get_events("primary", max_results).await
    }

    /// イベントの詳細情報を表示する
    pub fn display_events(&self, events: &Events) {
        println!("取得されたイベント数: {}", events.items.as_ref().map_or(0, |v| v.len()));
        
        if let Some(items) = &events.items {
            for (i, event) in items.iter().enumerate() {
                self.display_event(event, i + 1);
            }
        } else {
            println!("今後の予定はありません。");
        }
    }

    /// 単一のイベントの詳細を表示する
    pub fn display_event(&self, event: &Event, index: usize) {
        println!("\n--- イベント {} ---", index);
        
        if let Some(id) = &event.id {
            println!("ID: {}", id);
        }
        
        if let Some(summary) = &event.summary {
            println!("タイトル: {}", summary);
        }
        
        if let Some(start) = &event.start {
            if let Some(date_time) = &start.date_time {
                println!("開始時刻: {}", date_time);
            } else if let Some(date) = &start.date {
                println!("開始日: {}", date);
            }
        }
        
        if let Some(end) = &event.end {
            if let Some(date_time) = &end.date_time {
                println!("終了時刻: {}", date_time);
            } else if let Some(date) = &end.date {
                println!("終了日: {}", date);
            }
        }
        
        if let Some(description) = &event.description {
            println!("説明: {}", description);
        }
        
        if let Some(location) = &event.location {
            println!("場所: {}", location);
        }
    }

    /// イベントを作成する
    pub async fn create_event(&self, calendar_id: &str, event: Event) -> Result<Event> {
        let result = self.hub
            .events()
            .insert(event, calendar_id)
            .doit()
            .await?;

        Ok(result.1)
    }

    /// プライマリカレンダーにイベントを作成する
    pub async fn create_primary_event(&self, event: Event) -> Result<Event> {
        self.create_event("primary", event).await
    }

    /// イベントを削除する
    pub async fn delete_event(&self, calendar_id: &str, event_id: &str) -> Result<()> {
        self.hub
            .events()
            .delete(calendar_id, event_id)
            .doit()
            .await?;

        Ok(())
    }

    /// プライマリカレンダーのイベントを削除する
    pub async fn delete_primary_event(&self, event_id: &str) -> Result<()> {
        self.delete_event("primary", event_id).await
    }

    /// イベントを更新する
    pub async fn update_event(&self, calendar_id: &str, event_id: &str, event: Event) -> Result<Event> {
        let result = self.hub
            .events()
            .update(event, calendar_id, event_id)
            .doit()
            .await?;

        Ok(result.1)
    }

    /// プライマリカレンダーのイベントを更新する
    pub async fn update_primary_event(&self, event_id: &str, event: Event) -> Result<Event> {
        self.update_event("primary", event_id, event).await
    }

    /// 指定した日時範囲のイベントを取得する
    pub async fn get_events_in_range(
        &self,
        calendar_id: &str,
        time_min: chrono::DateTime<chrono::Utc>,
        time_max: chrono::DateTime<chrono::Utc>,
        max_results: i32,
    ) -> Result<Events> {
        let result = self.hub
            .events()
            .list(calendar_id)
            .time_min(time_min)
            .time_max(time_max)
            .max_results(max_results)
            .single_events(true)
            .order_by("startTime")
            .doit()
            .await?;

        Ok(result.1)
    }

    /// EventDataからGoogle CalendarのEventを作成する
    pub async fn create_event_from_event_data(&self, 
        title: &str,
        start_time: &str,
        end_time: &str,
        description: Option<&str>,
        location: Option<&str>
    ) -> Result<String> {
        use google_calendar3::api::{Event, EventDateTime};
        use chrono::{DateTime, Utc};
        
        // 日時解析のヘルパー関数
        fn parse_datetime(datetime_str: &str) -> Result<DateTime<Utc>> {
            use chrono::TimeZone;
            use chrono_tz::Asia::Tokyo;
            
            // ISO 8601形式の解析を試行
            if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
                return Ok(dt.with_timezone(&Utc));
            }

            // タイムゾーン付きフォーマット
            let formats_with_tz = [
                "%Y-%m-%dT%H:%M:%S%.fZ",
                "%Y-%m-%dT%H:%M:%SZ",
                "%Y-%m-%dT%H:%M:%S%z",
                "%Y-%m-%dT%H:%M:%S%.f%z",
            ];

            for format in &formats_with_tz {
                if let Ok(dt) = DateTime::parse_from_str(datetime_str, format) {
                    return Ok(dt.with_timezone(&Utc));
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
                        .ok_or_else(|| anyhow::anyhow!("日本時間への変換に失敗: {}", datetime_str))?;
                    return Ok(jst_dt.with_timezone(&Utc));
                }
                if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(datetime_str, format) {
                    let naive_dt = naive_date.and_hms_opt(0, 0, 0).unwrap();
                    let jst_dt = Tokyo.from_local_datetime(&naive_dt).single()
                        .ok_or_else(|| anyhow::anyhow!("日本時間への変換に失敗: {}", datetime_str))?;
                    return Ok(jst_dt.with_timezone(&Utc));
                }
            }

            Err(anyhow::anyhow!("日時の形式が認識できません。対応フォーマット例: '2025-07-01 15:30'、'2025年07月01日 15:30'、'2025-07-01T15:30:00' など: {}", datetime_str))
        }
        
        let start_time = parse_datetime(start_time)?;
        let end_time = parse_datetime(end_time)?;

        if end_time <= start_time {
            return Err(anyhow::anyhow!("終了時刻は開始時刻より後である必要があります"));
        }

        let mut event = Event::default();
        event.summary = Some(title.to_string());
        event.description = description.map(|s| s.to_string());
        event.location = location.map(|s| s.to_string());
        
        event.start = Some(EventDateTime {
            date_time: Some(start_time),
            time_zone: Some("Asia/Tokyo".to_string()),
            ..Default::default()
        });
        
        event.end = Some(EventDateTime {
            date_time: Some(end_time),
            time_zone: Some("Asia/Tokyo".to_string()),
            ..Default::default()
        });

        let created_event = self.create_primary_event(event).await?;
        Ok(created_event.id.unwrap_or_default())
    }

    /// 指定されたIDのイベントを取得する
    pub async fn get_event_by_id(&self, calendar_id: &str, event_id: &str) -> Result<Event> {
        let result = self.hub
            .events()
            .get(calendar_id, event_id)
            .doit()
            .await?;

        Ok(result.1)
    }

    /// プライマリカレンダーからIDでイベントを取得する
    pub async fn get_primary_event_by_id(&self, event_id: &str) -> Result<Event> {
        self.get_event_by_id("primary", event_id).await
    }
}

/// イベント作成用のビルダーパターン
pub struct EventBuilder {
    event: Event,
}

impl EventBuilder {
    /// 新しいイベントビルダーを作成
    pub fn new() -> Self {
        Self {
            event: Event::default(),
        }
    }

    /// イベントのタイトルを設定
    pub fn summary(mut self, summary: &str) -> Self {
        self.event.summary = Some(summary.to_string());
        self
    }

    /// イベントの説明を設定
    pub fn description(mut self, description: &str) -> Self {
        self.event.description = Some(description.to_string());
        self
    }

    /// イベントの場所を設定
    pub fn location(mut self, location: &str) -> Self {
        self.event.location = Some(location.to_string());
        self
    }

    /// イベントの開始時刻を設定
    pub fn start_time(mut self, start_time: chrono::DateTime<chrono::Utc>) -> Self {
        use google_calendar3::api::EventDateTime;
        let mut start = EventDateTime::default();
        start.date_time = Some(start_time);
        start.time_zone = Some("Asia/Tokyo".to_string());
        self.event.start = Some(start);
        self
    }

    /// イベントの終了時刻を設定
    pub fn end_time(mut self, end_time: chrono::DateTime<chrono::Utc>) -> Self {
        use google_calendar3::api::EventDateTime;
        let mut end = EventDateTime::default();
        end.date_time = Some(end_time);
        end.time_zone = Some("Asia/Tokyo".to_string());
        self.event.end = Some(end);
        self
    }

    /// イベントを構築
    pub fn build(self) -> Event {
        self.event
    }
}

impl Default for EventBuilder {
    fn default() -> Self {
        Self::new()
    }
}
