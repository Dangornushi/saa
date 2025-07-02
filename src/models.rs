use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
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
    pub conversation_history: Option<ConversationHistory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub action: ActionType,
    pub event_data: Option<EventData>,
    pub response_text: String, // AIの発言
    pub missing_data: Option<MissingEventData>, // 追加
    pub updated_conversation: Option<ConversationHistory>, // 更新された会話履歴
    pub start_time: Option<DateTime<Utc>>, // 開始
    pub end_time: Option<DateTime<Utc>>,     // 終了
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub id: Option<String>, // Google CalendarのイベントID（更新や削除時に使用）
    pub title: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    pub attendees: Vec<String>,
    pub priority: Option<Priority>,
    pub max_results: Option<i32>,
}

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("Validation Error: {0}")]
    ValidationError(String),
    #[error("Parse Error: {0}")]
    ParseError(String),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<chrono::ParseError> for SchedulerError {
    fn from(err: chrono::ParseError) -> Self {
        SchedulerError::ParseError(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MissingEventData {
    Title,
    StartTime,
    EndTime,
    All, // 全ての情報が不足している場合
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
    // EventDataを適用する新しいメソッド
    pub fn apply_event_data(&mut self, event_data: EventData, parse_datetime: impl Fn(&str) -> Result<DateTime<Utc>, SchedulerError>) -> Result<(), SchedulerError> {
        if let Some(title) = event_data.title {
            self.title = title;
        }
        if let Some(description) = event_data.description {
            self.description = Some(description);
        }
        if let Some(location) = event_data.location {
            self.location = Some(location);
        }
        if !event_data.attendees.is_empty() {
            self.attendees = event_data.attendees;
        }
        if let Some(priority) = event_data.priority {
            self.priority = priority;
        }

        let mut updated_start_time = self.start_time;
        if let Some(start_time_str) = event_data.start_time {
            updated_start_time = parse_datetime(&start_time_str)?;
        }

        let mut updated_end_time = self.end_time;
        if let Some(end_time_str) = event_data.end_time {
            updated_end_time = parse_datetime(&end_time_str)?;
        }

        if updated_end_time <= updated_start_time {
            return Err(SchedulerError::ValidationError("終了時刻は開始時刻より後である必要があります".to_string()));
        }

        self.start_time = updated_start_time;
        self.end_time = updated_end_time;
        self.updated_at = Utc::now();

        Ok(())
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


    // 重複チェック
    pub fn has_conflict(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> bool {
        self.events.iter().any(|event| {
            start < &event.end_time && end > &event.start_time
        })
    }

    // 特定のイベントを除外して重複チェック
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationHistory {
    pub messages: Vec<ConversationMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub event_context: Option<Uuid>, // 関連するイベントのID
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl ConversationHistory {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String, event_context: Option<Uuid>) {
        let message = ConversationMessage {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Utc::now(),
            event_context,
        };
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    pub fn add_user_message(&mut self, content: String, event_context: Option<Uuid>) {
        self.add_message(MessageRole::User, content, event_context);
    }

    pub fn add_assistant_message(&mut self, content: String, event_context: Option<Uuid>) {
        self.add_message(MessageRole::Assistant, content, event_context);
    }

    pub fn get_recent_messages(&self, count: usize) -> &[ConversationMessage] {
        let start = if self.messages.len() > count {
            self.messages.len() - count
        } else {
            0
        };
        &self.messages[start..]
    }

    pub fn get_context_string(&self, max_messages: Option<usize>) -> String {
        let messages = if let Some(max) = max_messages {
            self.get_recent_messages(max)
        } else {
            &self.messages
        };

        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "ユーザー",
                    MessageRole::Assistant => "アシスタント",
                    MessageRole::System => "システム",
                };
                format!("{}: {}", role, msg.content)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }
}