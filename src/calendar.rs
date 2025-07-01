use anyhow::Result;
use chrono::{DateTime, Utc, Duration, TimeZone, Datelike};
use chrono_tz::Asia::Tokyo;
use schedule_ai_agent::GoogleCalendarClient;
use google_calendar3::api::{Event, Events};

/// カレンダーサービス
pub struct CalendarService {
    client: GoogleCalendarClient,
}

impl CalendarService {
    /// 新しいカレンダーサービスを作成
    pub async fn new(client_secret_path: &str, token_cache_path: &str) -> Result<Self> {
        let client = GoogleCalendarClient::new(client_secret_path, token_cache_path).await?;
        Ok(Self { client })
    }

    /// 今日の予定を取得する
    pub async fn get_today_events(&self) -> Result<Events> {
        let now_jst = Utc::now().with_timezone(&Tokyo);
        let start_of_day = Tokyo.with_ymd_and_hms(now_jst.year(), now_jst.month(), now_jst.day(), 0, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let end_of_day = Tokyo.with_ymd_and_hms(now_jst.year(), now_jst.month(), now_jst.day(), 23, 59, 59)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        
        self.client.get_events_in_range(
            "primary",
            start_of_day,
            end_of_day,
            50
        ).await
    }

    /// 今週の予定を取得する
    pub async fn get_week_events(&self) -> Result<Events> {
        let now_jst = Utc::now().with_timezone(&Tokyo);
        let week_later_jst = now_jst + Duration::weeks(1);
        
        self.client.get_events_in_range(
            "primary",
            now_jst.with_timezone(&Utc),
            week_later_jst.with_timezone(&Utc),
            100
        ).await
    }

    /// 指定した期間の予定を取得する
    pub async fn get_events_in_period(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        max_results: i32
    ) -> Result<Events> {
        self.client.get_events_in_range("primary", start, end, max_results).await
    }

    /// 空き時間を検索する
    pub async fn find_free_time(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        duration_minutes: i64
    ) -> Result<Vec<(DateTime<Utc>, DateTime<Utc>)>> {
        let events = self.get_events_in_period(start, end, 100).await?;
        let mut free_slots = Vec::new();
        
        if let Some(items) = &events.items {
            let mut busy_times = Vec::new();
            
            // 忙しい時間帯を収集
            for event in items {
                if let (Some(start_time), Some(end_time)) = (
                    event.start.as_ref().and_then(|s| s.date_time.as_ref()),
                    event.end.as_ref().and_then(|e| e.date_time.as_ref())
                ) {
                    busy_times.push((start_time.clone(), end_time.clone()));
                }
            }
            
            // 忙しい時間帯をソート
            busy_times.sort_by(|a, b| a.0.cmp(&b.0));
            
            // 空き時間を計算
            let mut current_time = start;
            let duration = Duration::minutes(duration_minutes);
            
            for (busy_start, busy_end) in busy_times {
                // 現在時刻から忙しい時間帯の開始まで空きがあるかチェック
                if busy_start > current_time && busy_start - current_time >= duration {
                    free_slots.push((current_time, busy_start));
                }
                current_time = current_time.max(busy_end);
            }
            
            // 最後の忙しい時間帯から終了時刻まで空きがあるかチェック
            if current_time < end && end - current_time >= duration {
                free_slots.push((current_time, end));
            }
        } else {
            // イベントがない場合は全体が空き時間
            free_slots.push((start, end));
        }
        
        Ok(free_slots)
    }

    /// イベントを作成する
    pub async fn create_event(
        &self,
        title: &str,
        description: Option<&str>,
        location: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>
    ) -> Result<Event> {
        use schedule_ai_agent::EventBuilder;
        
        let mut builder = EventBuilder::new()
            .summary(title)
            .start_time(start_time)
            .end_time(end_time);
            
        if let Some(desc) = description {
            builder = builder.description(desc);
        }
        
        if let Some(loc) = location {
            builder = builder.location(loc);
        }
        
        let event = builder.build();
        self.client.create_primary_event(event).await
    }

    /// カレンダー情報をコンソールに表示する
    pub async fn display_calendar_summary(&self) -> Result<()> {
        println!("=== カレンダー情報 ===");
        
        // 今日の予定
        println!("\n📅 今日の予定:");
        let today_events = self.get_today_events().await?;
        self.client.display_events(&today_events);
        
        // 今週の予定数
        let week_events = self.get_week_events().await?;
        let week_count = week_events.items.as_ref().map_or(0, |v| v.len());
        println!("\n📊 今週の予定数: {} 件", week_count);
        
        Ok(())
    }
}
