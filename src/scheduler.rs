use crate::models::{Event, EventData, Priority, Schedule};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct Scheduler {
    schedule: Schedule,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            schedule: Schedule::new(),
        }
    }

    pub fn load_schedule(&mut self, schedule: Schedule) {
        self.schedule = schedule;
    }

    pub fn get_schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn create_event(&mut self, event_data: EventData) -> Result<Uuid> {
        let start_time = self.parse_datetime(&event_data.start_time)?;
        let end_time = self.parse_datetime(&event_data.end_time)?;

        if end_time <= start_time {
            return Err(anyhow!("終了時刻は開始時刻より後である必要があります"));
        }

        // 重複チェック
        if self.has_conflict(&start_time, &end_time) {
            return Err(anyhow!("指定された時間帯に既に予定があります"));
        }

        let mut event = Event::new(event_data.title, start_time, end_time)
            .with_priority(event_data.priority);

        if let Some(description) = event_data.description {
            event = event.with_description(description);
        }

        if let Some(location) = event_data.location {
            event = event.with_location(location);
        }

        for attendee in event_data.attendees {
            event = event.add_attendee(attendee);
        }

        let event_id = event.id;
        self.schedule.add_event(event);

        Ok(event_id)
    }

    pub fn update_event(&mut self, event_id: Uuid, event_data: EventData) -> Result<()> {
        // 先に日時を解析
        let start_time = self.parse_datetime(&event_data.start_time)?;
        let end_time = self.parse_datetime(&event_data.end_time)?;

        if end_time <= start_time {
            return Err(anyhow!("終了時刻は開始時刻より後である必要があります"));
        }

        // 他の予定との重複チェック（自分自身は除く）
        if self.has_conflict_excluding(&start_time, &end_time, event_id) {
            return Err(anyhow!("指定された時間帯に既に他の予定があります"));
        }

        // 最後にイベントを更新
        let event = self.schedule.get_event_mut(event_id)
            .ok_or_else(|| anyhow!("指定されたIDの予定が見つかりません"))?;

        event.title = event_data.title;
        event.start_time = start_time;
        event.end_time = end_time;
        event.description = event_data.description;
        event.location = event_data.location;
        event.attendees = event_data.attendees;
        event.priority = event_data.priority;
        event.updated_at = Utc::now();

        Ok(())
    }

    pub fn delete_event(&mut self, event_id: Uuid) -> Result<()> {
        if self.schedule.remove_event(event_id) {
            Ok(())
        } else {
            Err(anyhow!("指定されたIDの予定が見つかりません"))
        }
    }

    pub fn get_event(&self, event_id: Uuid) -> Option<&Event> {
        self.schedule.get_event(event_id)
    }

    pub fn list_events(&self) -> Vec<&Event> {
        let mut events = self.schedule.events.iter().collect::<Vec<_>>();
        events.sort_by(|a, b| a.start_time.cmp(&b.start_time));
        events
    }

    pub fn get_events_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&Event> {
        self.schedule.get_events_by_date_range(start, end)
    }

    pub fn search_events(&self, query: &str) -> Vec<&Event> {
        let query_lower = query.to_lowercase();
        self.schedule
            .events
            .iter()
            .filter(|event| {
                event.title.to_lowercase().contains(&query_lower)
                    || event
                        .description
                        .as_ref()
                        .map_or(false, |desc| desc.to_lowercase().contains(&query_lower))
                    || event
                        .location
                        .as_ref()
                        .map_or(false, |loc| loc.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    pub fn get_upcoming_events(&self, limit: usize) -> Vec<&Event> {
        let now = Utc::now();
        let mut upcoming: Vec<_> = self.schedule
            .events
            .iter()
            .filter(|event| event.start_time > now)
            .collect();
        
        upcoming.sort_by(|a, b| a.start_time.cmp(&b.start_time));
        upcoming.into_iter().take(limit).collect()
    }

    pub fn get_today_events(&self) -> Vec<&Event> {
        let now = Utc::now();
        let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_of_day = now.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc();
        
        self.get_events_by_date_range(start_of_day, end_of_day)
    }

    fn parse_datetime(&self, datetime_str: &str) -> Result<DateTime<Utc>> {
        // ISO 8601形式の解析を試行
        if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
            return Ok(dt.with_timezone(&Utc));
        }

        // その他の形式も試行
        let formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%d",
            "%m/%d/%Y %H:%M",
            "%m/%d/%Y",
        ];

        for format in &formats {
            if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(datetime_str, format) {
                return Ok(naive_dt.and_utc());
            }
            if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(datetime_str, format) {
                return Ok(naive_date.and_hms_opt(0, 0, 0).unwrap().and_utc());
            }
        }

        Err(anyhow!("日時の形式が認識できません: {}", datetime_str))
    }

    fn has_conflict(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> bool {
        self.schedule.events.iter().any(|event| {
            // 重複の条件: 新しい予定の開始時刻が既存の予定の終了時刻より前で、
            // かつ新しい予定の終了時刻が既存の予定の開始時刻より後
            start < &event.end_time && end > &event.start_time
        })
    }

    fn has_conflict_excluding(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
        exclude_id: Uuid,
    ) -> bool {
        self.schedule.events.iter().any(|event| {
            event.id != exclude_id && start < &event.end_time && end > &event.start_time
        })
    }

    pub fn get_statistics(&self) -> ScheduleStatistics {
        let total_events = self.schedule.events.len();
        let now = Utc::now();
        
        let upcoming_events = self.schedule.events
            .iter()
            .filter(|e| e.start_time > now)
            .count();
        
        let past_events = total_events - upcoming_events;
        
        let priority_counts = self.schedule.events
            .iter()
            .fold([0; 4], |mut acc, event| {
                match event.priority {
                    Priority::Low => acc[0] += 1,
                    Priority::Medium => acc[1] += 1,
                    Priority::High => acc[2] += 1,
                    Priority::Urgent => acc[3] += 1,
                }
                acc
            });

        ScheduleStatistics {
            total_events,
            upcoming_events,
            past_events,
            low_priority: priority_counts[0],
            medium_priority: priority_counts[1],
            high_priority: priority_counts[2],
            urgent_priority: priority_counts[3],
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