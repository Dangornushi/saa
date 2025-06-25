use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub location: Option<String>,
    pub attendees: Vec<String>,
    pub priority: Priority,
    pub status: EventStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventStatus {
    Scheduled,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRequest {
    pub user_input: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub action: ActionType,
    pub event_data: Option<EventData>,
    pub response_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    CreateEvent,
    UpdateEvent,
    DeleteEvent,
    ListEvents,
    SearchEvents,
    GetEventDetails,
    GeneralResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub attendees: Vec<String>,
    pub priority: Priority,
}

impl Event {
    pub fn new(
        title: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            description: None,
            start_time,
            end_time,
            location: None,
            attendees: Vec::new(),
            priority: Priority::Medium,
            status: EventStatus::Scheduled,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self.updated_at = Utc::now();
        self
    }

    pub fn with_location(mut self, location: String) -> Self {
        self.location = Some(location);
        self.updated_at = Utc::now();
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self.updated_at = Utc::now();
        self
    }

    pub fn add_attendee(mut self, attendee: String) -> Self {
        self.attendees.push(attendee);
        self.updated_at = Utc::now();
        self
    }
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn remove_event(&mut self, event_id: Uuid) -> bool {
        if let Some(pos) = self.events.iter().position(|e| e.id == event_id) {
            self.events.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_event(&self, event_id: Uuid) -> Option<&Event> {
        self.events.iter().find(|e| e.id == event_id)
    }

    pub fn get_event_mut(&mut self, event_id: Uuid) -> Option<&mut Event> {
        self.events.iter_mut().find(|e| e.id == event_id)
    }

    pub fn get_events_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.start_time >= start && e.start_time <= end)
            .collect()
    }
}