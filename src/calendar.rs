use crate::models::{Event, Schedule};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
// use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;

pub trait CalendarProvider {
    fn sync_events(&self, schedule: &mut Schedule) -> Result<()>;
    fn create_event(&self, event: &Event) -> Result<String>;
    fn update_event(&self, external_id: &str, event: &Event) -> Result<()>;
    fn delete_event(&self, external_id: &str) -> Result<()>;
}

pub struct GoogleCalendarProvider {
    access_token: String,
    calendar_id: String,
}

impl GoogleCalendarProvider {
    pub fn new(access_token: String, calendar_id: Option<String>) -> Self {
        Self {
            access_token,
            calendar_id: calendar_id.unwrap_or_else(|| "primary".to_string()),
        }
    }

    fn get_events(&self, _time_min: DateTime<Utc>, _time_max: DateTime<Utc>) -> Result<Vec<GoogleCalendarEvent>> {
        // Google Calendar API連携は将来実装
        Ok(Vec::new())
    }

    fn parse_google_event(&self, item: &Value) -> Result<GoogleCalendarEvent> {
        let id = item["id"].as_str().ok_or_else(|| anyhow!("Missing event ID"))?.to_string();
        let summary = item["summary"].as_str().unwrap_or("無題").to_string();
        let description = item["description"].as_str().map(|s| s.to_string());
        let location = item["location"].as_str().map(|s| s.to_string());

        let start_time = self.parse_datetime(&item["start"])?;
        let end_time = self.parse_datetime(&item["end"])?;

        Ok(GoogleCalendarEvent {
            id,
            summary,
            description,
            location,
            start_time,
            end_time,
        })
    }

    fn parse_datetime(&self, datetime_obj: &Value) -> Result<DateTime<Utc>> {
        if let Some(datetime_str) = datetime_obj["dateTime"].as_str() {
            DateTime::parse_from_rfc3339(datetime_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| anyhow!("Failed to parse datetime: {}", e))
        } else if let Some(date_str) = datetime_obj["date"].as_str() {
            let naive_date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
            Ok(naive_date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        } else {
            Err(anyhow!("No valid datetime found"))
        }
    }

    fn event_to_google_format(&self, event: &Event) -> Value {
        let mut google_event = json!({
            "summary": event.title,
            "start": {
                "dateTime": event.start_time.to_rfc3339(),
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": event.end_time.to_rfc3339(),
                "timeZone": "UTC"
            }
        });

        if let Some(ref description) = event.description {
            google_event["description"] = json!(description);
        }

        if let Some(ref location) = event.location {
            google_event["location"] = json!(location);
        }

        if !event.attendees.is_empty() {
            let attendees: Vec<Value> = event.attendees
                .iter()
                .map(|email| json!({"email": email}))
                .collect();
            google_event["attendees"] = json!(attendees);
        }

        google_event
    }
}

impl CalendarProvider for GoogleCalendarProvider {
    fn sync_events(&self, _schedule: &mut Schedule) -> Result<()> {
        // Google Calendar連携は将来実装
        Err(anyhow!("Google Calendar連携は未実装です"))
    }

    fn create_event(&self, _event: &Event) -> Result<String> {
        Err(anyhow!("Google Calendar連携は未実装です"))
    }

    fn update_event(&self, _external_id: &str, _event: &Event) -> Result<()> {
        Err(anyhow!("Google Calendar連携は未実装です"))
    }

    fn delete_event(&self, _external_id: &str) -> Result<()> {
        Err(anyhow!("Google Calendar連携は未実装です"))
    }
}

#[derive(Debug, Clone)]
struct GoogleCalendarEvent {
    id: String,
    summary: String,
    description: Option<String>,
    location: Option<String>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
}

// Notion Calendar連携（将来実装用の基盤）
pub struct NotionCalendarProvider {
    api_key: String,
    database_id: String,
}

impl NotionCalendarProvider {
    pub fn new(api_key: String, database_id: String) -> Self {
        Self {
            api_key,
            database_id,
        }
    }
}

impl CalendarProvider for NotionCalendarProvider {
    fn sync_events(&self, _schedule: &mut Schedule) -> Result<()> {
        // Notion API実装（将来の拡張用）
        Err(anyhow!("Notion Calendar連携は未実装です"))
    }

    fn create_event(&self, _event: &Event) -> Result<String> {
        Err(anyhow!("Notion Calendar連携は未実装です"))
    }

    fn update_event(&self, _external_id: &str, _event: &Event) -> Result<()> {
        Err(anyhow!("Notion Calendar連携は未実装です"))
    }

    fn delete_event(&self, _external_id: &str) -> Result<()> {
        Err(anyhow!("Notion Calendar連携は未実装です"))
    }
}

// カレンダー連携マネージャー
pub struct CalendarManager {
    providers: HashMap<String, Box<dyn CalendarProvider + Send + Sync>>,
}

impl CalendarManager {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn add_provider(&mut self, name: String, provider: Box<dyn CalendarProvider + Send + Sync>) {
        self.providers.insert(name, provider);
    }

    pub fn sync_all(&self, schedule: &mut Schedule) -> Result<()> {
        for (name, provider) in &self.providers {
            match provider.sync_events(schedule) {
                Ok(()) => println!("{}との同期が完了しました", name),
                Err(e) => println!("{}との同期でエラーが発生しました: {}", name, e),
            }
        }
        Ok(())
    }

    pub fn create_event_in_all(&self, event: &Event) -> Result<HashMap<String, String>> {
        let mut external_ids = HashMap::new();

        for (name, provider) in &self.providers {
            match provider.create_event(event) {
                Ok(external_id) => {
                    external_ids.insert(name.clone(), external_id);
                }
                Err(e) => {
                    println!("{}でのイベント作成でエラーが発生しました: {}", name, e);
                }
            }
        }

        Ok(external_ids)
    }
}