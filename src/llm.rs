use crate::config::Config;
use crate::models::{ActionType, EventData, LLMRequest, LLMResponse, MissingEventData, Priority};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use chrono_tz::Asia::Tokyo;
use colored::Colorize;
use serde_json::{Value, json};
use std::env; // 追加

#[async_trait] // 追加
pub trait LLM: Send + Sync {
    async fn process_request(&self, request: LLMRequest) -> Result<LLMResponse>;
    async fn test_connection(&self) -> Result<()>;
}

pub struct LLMClient {
    api_key: String,
    base_url: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
}

impl LLMClient {
    
    pub fn from_config(config: &Config) -> Result<Self> {
        let llm_config = &config.llm;

        // APIキーを取得
        let api_key = llm_config.gemini_api_key
            .clone()
            .or_else(|| env::var("GEMINI_API_KEY").ok())
            .ok_or_else(|| anyhow!("Gemini API key not found. Please set gemini_api_key in config or GEMINI_API_KEY environment variable"))?;

        // ベースURLを決定
        let base_url = llm_config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());

        // モデルを決定
        let model = llm_config
            .model
            .clone()
            .unwrap_or_else(|| "gemini-2.5-flash".to_string());

        let temperature = llm_config.temperature.unwrap_or(0.7);
        let max_tokens = llm_config.max_tokens.unwrap_or(1000);

        Ok(Self {
            api_key,
            base_url,
            model,
            temperature,
            max_tokens,
        })
    }
}

#[async_trait]
impl LLM for LLMClient {
    async fn process_request(&self, request: LLMRequest) -> Result<LLMResponse> {
        let system_prompt = self.create_system_prompt();
        let user_message = self.create_user_message(&request);
        println!("user message: {}", user_message);

        let client = reqwest::Client::new();
        println!("{}", "LLMクライアントを使用しています...".dimmed());
        let request_url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, self.model, self.api_key
        );

        let payload = json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        {
                            "text": format!("{}\n\n{}", system_prompt, user_message)
                        }
                    ]
                }
            ],
            "generationConfig": {
                "temperature": self.temperature,
                "maxOutputTokens": self.max_tokens
            }
        });

        let request_builder = client.post(&request_url);

        let response = request_builder
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        println!("Response status: {}", response.status());

        let response_json: Value = response.json().await?;
        println!("Response JSON: {:?}", response_json);

        let content = response_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                println!("Invalid response format from Gemini: {:?}", response_json);
                anyhow!("Invalid response format from Gemini")
            })?;

        let llm_response = self.parse_llm_response(content, &request)?;

        // 不足している情報がある場合は、ユーザーに質問を投げかける
        if let Some(missing_data) = &llm_response.missing_data {
            let question = match missing_data {
                MissingEventData::Title => "予定のタイトルを教えていただけますか？",
                MissingEventData::StartTime => "予定の開始時刻を教えていただけますか？",
                MissingEventData::EndTime => "予定の終了時刻を教えていただけますか？",
                MissingEventData::All => {
                    "予定のタイトル、開始時刻、終了時刻を教えていただけますか？"
                }
            };
            
            // 会話履歴を更新
            let mut updated_conversation = request.conversation_history.clone().unwrap_or_else(|| {
                use crate::models::ConversationHistory;
                ConversationHistory::new()
            });
            
            // ユーザーメッセージを追加
            updated_conversation.add_user_message(request.user_input.clone(), None);
            
            // アシスタントメッセージを追加
            updated_conversation.add_assistant_message(question.to_string(), None);
            
            return Ok(LLMResponse {
                action: llm_response.action,
                event_data: llm_response.event_data,
                response_text: question.to_string(),
                missing_data: llm_response.missing_data,
                updated_conversation: Some(updated_conversation),
                start_time: None, // 開始時刻はまだ不明
                end_time: None,   // 終了時刻はまだ不明
            });
        }

        Ok(llm_response)
    }

    async fn test_connection(&self) -> Result<()> {
        println!("LLM接続テスト中 (Gemini)...");
        let test_request = LLMRequest {
            user_input: "こんにちは".to_string(),
            context: None,
            conversation_history: None,
        };

        match self.process_request(test_request).await {
            Ok(response) => {
                println!("LLM接続テスト成功!応答: {}", response.response_text);
                Ok(())
            }
            Err(e) => {
                eprintln!("LLM接続テスト失敗: {}", e);
                Err(e)
            }
        }
    }
}

impl LLMClient {
    fn create_system_prompt(&self) -> String {
        r#"
あなたは予定管理AIエージェントです。ユーザーの自然言語入力を解析して、適切なアクションを決定してください。

可能なアクション:
- CREATE_EVENT: 新しい予定を作成
- UPDATE_EVENT: 既存の予定を更新
- DELETE_EVENT: 予定を削除
- GET_EVENT_DETAILS: 予定の詳細を取得(予定を詳しく教えてなどとリクエストされた場合)
- LIST_EVENTS: 予定を簡単に取得
- SEARCH_EVENTS: 予定をタイトル名を基準に検索
- GENERAL_RESPONSE: 一般的な応答

応答は以下のJSON形式で返してください。必要な情報が不足している場合は、`missing_data` フィールドに不足している情報の種類（"Title", "StartTime", "EndTime", "All"）を設定してください。

```json
{
    "action": "アクションタイプ",
    "event_data": {
        "title": "予定のタイトル（不明な場合はnull）",
        "description": "予定の説明（オプション、不明な場合はnull）",
        "start_time": "開始時刻（ISO 8601形式、不明な場合はnull）",
        "end_time": "終了時刻（ISO 8601形式、不明な場合はnull）",
        "location": "場所（オプション、不明な場合はnull）",
        "attendees": ["参加者のリスト"],
        "priority": "Low/Medium/High/Urgent（不明な場合はnull）"
    },
    "response_text": "ユーザーへの応答メッセージ",
    "missing_data": "不足している情報の種類（例: Title, StartTime, EndTime, All, またはnull）"
}
```

例: アクションタイプがGeneralResponseの場合
```json
{
    "action": "GENERAL_RESPONSE",
    "event_data": {
        "title": null,
        "description": null,
        "start_time": null,
        "end_time": null,
        "location": null,
        "attendees": [],
        "priority": null
    },
    "response_text": "ユーザーへの応答メッセージ",
    "missing_data": null
}
```

例: 予定作成に必要なタイトルが不足している場合
```json
{
    "action": "CREATE_EVENT",
    "event_data": {
        "title": null,
        "description": "今日の午後3時の会議",
        "start_time": "2025-06-29T15:00:00Z",
        "end_time": "2025-06-29T16:00:00Z",
        "location": "会議室A",
        "attendees": [],
        "priority": "Medium"
    },
    "response_text": "会議のタイトルを教えていただけますか？",
    "missing_data": "Title"
}
```

日時の解析では、相対的な表現（明日、来週など）も適切に処理してください。
現在の日時を基準として計算してください。
"#.to_string()
    }

    fn create_user_message(&self, request: &LLMRequest) -> String {
        let mut message = format!("ユーザー入力: {}", request.user_input);

        if let Some(context) = &request.context {
            message.push_str(&format!("\n\nコンテキスト: {}", context));
        }

        // 会話履歴を含める
        if let Some(conversation) = &request.conversation_history {
            if !conversation.messages.is_empty() {
                message.push_str("\n\n前回の会話履歴:");
                let recent_context = conversation.get_context_string(Some(5)); // 直近5メッセージ
                message.push_str(&format!("\n{}", recent_context));
            }
        }

        let now_jst = Utc::now().with_timezone(&Tokyo);
        message.push_str(&format!(
            "\n\n現在の日時: {} (JST)",
            now_jst.format("%Y-%m-%d %H:%M:%S")
        ));

        message
    }

    fn parse_llm_response(&self, content: &str, request: &LLMRequest) -> Result<LLMResponse> {
        // contentの最初の7文字（```json）と最後尾の3文字（```）が存在すれば削除
        let mut content = content.trim();
        if content.starts_with("```json") {
            content = &content[7..];
            content = content.trim_start();
        }
        if content.ends_with("```") {
            content = &content[..content.len() - 3];
            content = content.trim_end();
        }

        // JSON形式での応答を期待
        let response_json: Value = serde_json::from_str(content)
            .map_err(|e| anyhow!("Failed to parse LLM response: {}", e))?;

        let action_str = response_json["action"]
            .as_str()
            .ok_or_else(|| anyhow!("Action type is missing in the response"))?;

        let action = self.parse_action_type(action_str)?;

        let missing_data_str = response_json["missing_data"].as_str();
        let missing_data = match missing_data_str {
            Some("Title") => Some(MissingEventData::Title),
            Some("StartTime") => Some(MissingEventData::StartTime),
            Some("EndTime") => Some(MissingEventData::EndTime),
            Some("All") => Some(MissingEventData::All),
            _ => None,
        };

        let event_data = if let Some(data) = response_json.get("event_data") {
            Some(self.parse_event_data(data)?)
        } else {
            None
        };

        let response_text = response_json["response_text"]
            .as_str()
            .unwrap_or("No response text provided")
            .to_string();

        // 開始時間と終了時間をパース
        let start_time = if let Some(data) = response_json.get("event_data") {
            if let Some(start_time_str) = data["start_time"].as_str() {
                match DateTime::parse_from_rfc3339(start_time_str) {
                    Ok(dt) => Some(dt.with_timezone(&Utc)),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        let end_time = if let Some(data) = response_json.get("event_data") {
            if let Some(end_time_str) = data["end_time"].as_str() {
                match DateTime::parse_from_rfc3339(end_time_str) {
                    Ok(dt) => Some(dt.with_timezone(&Utc)),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        // 会話履歴を更新
        let mut updated_conversation = request.conversation_history.clone().unwrap_or_else(|| {
            use crate::models::ConversationHistory;
            ConversationHistory::new()
        });
        
        // ユーザーメッセージを追加
        updated_conversation.add_user_message(request.user_input.clone(), None);
        
        // アシスタントメッセージを追加
        updated_conversation.add_assistant_message(response_text.clone(), None);

        Ok(LLMResponse {
            action,
            event_data,
            response_text,
            missing_data,
            updated_conversation: Some(updated_conversation),
            start_time,
            end_time,
        })
    }

    fn parse_action_type(&self, action_str: &str) -> Result<ActionType> {
        match action_str.to_uppercase().as_str() {
            "CREATE_EVENT" => Ok(ActionType::CreateEvent),
            "UPDATE_EVENT" => Ok(ActionType::UpdateEvent),
            "DELETE_EVENT" => Ok(ActionType::DeleteEvent),
            "LIST_EVENTS" => Ok(ActionType::ListEvents),
            "SEARCH_EVENTS" => Ok(ActionType::SearchEvents),
            "GET_EVENT_DETAILS" => Ok(ActionType::GetEventDetails),
            "GENERAL_RESPONSE" => Ok(ActionType::GeneralResponse),
            _ => Ok(ActionType::GeneralResponse), // 未知のアクションタイプはGeneralResponseとして扱う
        }
    }

    fn parse_event_data(&self, data: &Value) -> Result<EventData> {
        let title = data["title"].as_str().map(|s| s.to_string());
        let start_time = data["start_time"].as_str().map(|s| s.to_string());
        let end_time = data["end_time"].as_str().map(|s| s.to_string());

        let description = data["description"].as_str().map(|s| s.to_string());
        let location = data["location"].as_str().map(|s| s.to_string());

        let attendees = if let Some(arr) = data["attendees"].as_array() {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        } else {
            Vec::new()
        };

        let priority = match data["priority"].as_str() {
            Some("Low") => Some(Priority::Low),
            Some("Medium") => Some(Priority::Medium),
            Some("High") => Some(Priority::High),
            Some("Urgent") => Some(Priority::Urgent),
            _ => None,
        };

        Ok(EventData {
            title,
            description,
            start_time,
            end_time,
            location,
            attendees,
            priority,
            max_results: None,
        })
    }
}

// オフライン用のモックLLMクライアント
pub struct MockLLMClient;

impl MockLLMClient {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait] // 追加
impl LLM for MockLLMClient {
    async fn process_request(&self, request: LLMRequest) -> Result<LLMResponse> {
        // 簡単なパターンマッチングで基本的な機能を提供
        let input = request.user_input.to_lowercase();

        if input.contains("予定")
            && (input.contains("作成") || input.contains("追加") || input.contains("入れて"))
        {
            let start_time = Utc::now();
            let end_time = start_time + chrono::Duration::hours(1);
            
            Ok(LLMResponse {
                action: ActionType::CreateEvent,
                event_data: Some(EventData {
                    title: Some("WEB会議".to_string()), // タイトルをWEB会議に固定
                    description: Some("LLMで解析された予定".to_string()),
                    start_time: Some(start_time.format("%Y-%m-%dT%H:%M:%SZ").to_string()), // 仮の時刻
                    end_time: Some(end_time.format("%Y-%m-%dT%H:%M:%SZ").to_string()), // 仮の時刻
                    location: None,
                    attendees: Vec::new(),
                    priority: Some(Priority::Medium),
                    max_results: None,
                }),
                response_text: "新しい予定を作成しました。".to_string(),
                missing_data: None,
                updated_conversation: None,
                start_time: Some(start_time),
                end_time: Some(end_time),
            })
        } else if input.contains("一覧") || input.contains("リスト") {
            Ok(LLMResponse {
                action: ActionType::ListEvents,
                event_data: None,
                response_text: "予定一覧を表示します。".to_string(),
                missing_data: None,
                updated_conversation: None,
                start_time: None,
                end_time: None,
            })
        } else {
            Ok(LLMResponse {
                action: ActionType::GeneralResponse,
                event_data: None,
                response_text: "申し訳ございませんが、その要求を理解できませんでした。".to_string(),
                missing_data: None,
                updated_conversation: None,
                start_time: None,
                end_time: None,
            })
        }
    }

    async fn test_connection(&self) -> Result<()> {
        println!("モックLLM接続テスト中...");
        // モックなので常に成功
        println!("モックLLM接続テスト成功！");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LLMRequest;

    #[tokio::test]
    async fn test_create_event_action() -> Result<()> {
        let mock_llm = MockLLMClient::new();
        let user_input = "明日の2時から始まるWEB会議の予定を入れて".to_string();
        let request = LLMRequest {
            user_input,
            context: None,
            conversation_history: None,
        };

        let response = mock_llm.process_request(request).await?;

        assert_eq!(response.action, ActionType::CreateEvent);
        assert!(
            response
                .response_text
                .contains("新しい予定を作成しました。")
        );

        Ok(())
    }
}
