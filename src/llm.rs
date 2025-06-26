use crate::models::{ActionType, EventData, LLMRequest, LLMResponse, Priority};
use crate::config::Config;
use anyhow::{anyhow, Result};
use chrono::Utc;
use regex::Regex;
use serde_json::{json, Value};
use std::env;

#[derive(Debug, Clone)]
pub enum LLMProvider {
    OpenAI,
    GitHubCopilot,
    AzureOpenAI,
    Gemini,
}

pub struct LLMClient {
    provider: LLMProvider,
    api_key: String,
    base_url: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
}

impl LLMClient {
    pub fn new() -> Result<Self> {
        // デフォルトはOpenAI
        let provider = LLMProvider::OpenAI;
        let api_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("LLM_API_KEY"))
            .map_err(|_| anyhow!("API key not found. Please set OPENAI_API_KEY or LLM_API_KEY environment variable"))?;
        
        let base_url = env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        
        let model = env::var("LLM_MODEL")
            .unwrap_or_else(|_| "gpt-3.5-turbo".to_string());

        Ok(Self {
            provider,
            api_key,
            base_url,
            model,
            temperature: 0.7,
            max_tokens: 1000,
        })
    }

    pub fn from_config(config: &Config) -> Result<Self> {
        let llm_config = &config.llm;
        
        // プロバイダーを決定
        let provider = match llm_config.provider.as_deref().unwrap_or("openai") {
            "github_copilot" => LLMProvider::GitHubCopilot,
            "azure_openai" => LLMProvider::AzureOpenAI,
            "gemini" => LLMProvider::Gemini,
            _ => LLMProvider::OpenAI,
        };
        
        // APIキーを取得（プロバイダーに応じて）
        let api_key = match provider {
            LLMProvider::GitHubCopilot => {
                llm_config.github_token
                    .clone()
                    .or_else(|| env::var("GITHUB_TOKEN").ok())
                    .or_else(|| env::var("GITHUB_COPILOT_TOKEN").ok())
                    .ok_or_else(|| anyhow!("GitHub token not found. Please set github_token in config or GITHUB_TOKEN environment variable"))?
            },
            LLMProvider::Gemini => {
                llm_config.gemini_api_key
                    .clone()
                    .or_else(|| env::var("GEMINI_API_KEY").ok())
                    .ok_or_else(|| anyhow!("Gemini API key not found. Please set gemini_api_key in config or GEMINI_API_KEY environment variable"))?
            },
            _ => {
                llm_config.api_key
                    .clone()
                    .or_else(|| env::var("OPENAI_API_KEY").ok())
                    .or_else(|| env::var("LLM_API_KEY").ok())
                    .ok_or_else(|| anyhow!("API key not found. Please set it in config file or environment variable"))?
            }
        };
        
        // ベースURLを決定
        let base_url = llm_config.base_url
            .clone()
            .or_else(|| match provider {
                LLMProvider::GitHubCopilot => Some("https://api.githubcopilot.com".to_string()),
                LLMProvider::OpenAI => Some("https://api.openai.com/v1".to_string()),
                LLMProvider::AzureOpenAI => env::var("AZURE_OPENAI_ENDPOINT").ok(),
                LLMProvider::Gemini => Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
            })
            .unwrap_or_else(|| match provider {
                LLMProvider::GitHubCopilot => "https://api.githubcopilot.com".to_string(),
                LLMProvider::AzureOpenAI => "https://your-resource.openai.azure.com".to_string(),
                LLMProvider::Gemini => "https://generativelanguage.googleapis.com/v1beta".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });
        
        // モデルを決定
        let model = llm_config.model
            .clone()
            .unwrap_or_else(|| match provider {
                LLMProvider::GitHubCopilot => "gpt-4".to_string(),
                LLMProvider::Gemini => "gemini-1.5-flash".to_string(),
                _ => "gpt-3.5-turbo".to_string(),
            });
        
        let temperature = llm_config.temperature.unwrap_or(0.7);
        let max_tokens = llm_config.max_tokens.unwrap_or(1000);

        Ok(Self {
            provider,
            api_key,
            base_url,
            model,
            temperature,
            max_tokens,
        })
    }

    pub fn process_request(&self, request: LLMRequest) -> Result<LLMResponse> {
        let system_prompt = self.create_system_prompt();
        let user_message = self.create_user_message(&request);

        let _payload = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": user_message
                }
            ],
            "temperature": self.temperature,
            "max_tokens": self.max_tokens
        });

        // HTTP通信は将来実装予定
        // 現在は設定に基づいたモック応答を返す
        let provider_name = match self.provider {
            LLMProvider::GitHubCopilot => "GitHub Copilot",
            LLMProvider::OpenAI => "OpenAI",
            LLMProvider::AzureOpenAI => "Azure OpenAI",
            LLMProvider::Gemini => "Gemini",
        };

        let response_json = json!({
            "choices": [{
                "message": {
                    "content": format!(r#"{{"action": "GENERAL_RESPONSE", "response_text": "{}（{}）の設定が完了しています。実際のLLM通信は今後実装予定です。現在はモックLLMをお使いください。"}}"#, provider_name, self.model)
                }
            }]
        });
        
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid response format"))?;

        self.parse_llm_response(content)
    }

    fn create_system_prompt(&self) -> String {
        r#"
あなたは予定管理AIエージェントです。ユーザーの自然言語入力を解析して、適切なアクションを決定してください。

可能なアクション:
- CREATE_EVENT: 新しい予定を作成
- UPDATE_EVENT: 既存の予定を更新
- DELETE_EVENT: 予定を削除
- LIST_EVENTS: 予定一覧を表示
- SEARCH_EVENTS: 予定を検索
- GET_EVENT_DETAILS: 特定の予定の詳細を取得
- GENERAL_RESPONSE: 一般的な応答

応答は以下のJSON形式で返してください:
{
    "action": "アクションタイプ",
    "event_data": {
        "title": "予定のタイトル",
        "description": "予定の説明（オプション）",
        "start_time": "開始時刻（ISO 8601形式）",
        "end_time": "終了時刻（ISO 8601形式）",
        "location": "場所（オプション）",
        "attendees": ["参加者のリスト"],
        "priority": "Low/Medium/High/Urgent"
    },
    "response_text": "ユーザーへの応答メッセージ"
}

日時の解析では、相対的な表現（明日、来週など）も適切に処理してください。
現在の日時を基準として計算してください。
"#.to_string()
    }

    fn create_user_message(&self, request: &LLMRequest) -> String {
        let mut message = format!("ユーザー入力: {}", request.user_input);
        
        if let Some(context) = &request.context {
            message.push_str(&format!("\n\nコンテキスト: {}", context));
        }

        let now = Utc::now();
        message.push_str(&format!("\n\n現在の日時: {}", now.format("%Y-%m-%d %H:%M:%S UTC")));

        message
    }

    fn parse_llm_response(&self, content: &str) -> Result<LLMResponse> {
        // JSONブロックを抽出
        let json_regex = Regex::new(r"```json\s*(.*?)\s*```")?;
        let json_str = if let Some(captures) = json_regex.captures(content) {
            captures.get(1).unwrap().as_str()
        } else {
            // JSONブロックがない場合は、全体をJSONとして解析を試行
            content
        };

        match serde_json::from_str::<Value>(json_str) {
            Ok(json) => {
                let action = self.parse_action_type(
                    json["action"].as_str().unwrap_or("GENERAL_RESPONSE")
                )?;
                
                let event_data = if json["event_data"].is_object() {
                    Some(self.parse_event_data(&json["event_data"])?)
                } else {
                    None
                };

                let response_text = json["response_text"]
                    .as_str()
                    .unwrap_or("応答を生成できませんでした。")
                    .to_string();

                Ok(LLMResponse {
                    action,
                    event_data,
                    response_text,
                })
            }
            Err(_) => {
                // JSONの解析に失敗した場合は、一般的な応答として処理
                Ok(LLMResponse {
                    action: ActionType::GeneralResponse,
                    event_data: None,
                    response_text: content.to_string(),
                })
            }
        }
    }

    fn parse_action_type(&self, action_str: &str) -> Result<ActionType> {
        match action_str.to_uppercase().as_str() {
            "CREATE_EVENT" => Ok(ActionType::CreateEvent),
            "UPDATE_EVENT" => Ok(ActionType::UpdateEvent),
            "DELETE_EVENT" => Ok(ActionType::DeleteEvent),
            "LIST_EVENTS" => Ok(ActionType::ListEvents),
            "SEARCH_EVENTS" => Ok(ActionType::SearchEvents),
            "GET_EVENT_DETAILS" => Ok(ActionType::GetEventDetails),
            _ => Ok(ActionType::GeneralResponse),
        }
    }

    fn parse_event_data(&self, data: &Value) -> Result<EventData> {
        let title = data["title"]
            .as_str()
            .ok_or_else(|| anyhow!("Title is required"))?
            .to_string();

        let start_time = data["start_time"]
            .as_str()
            .ok_or_else(|| anyhow!("Start time is required"))?
            .to_string();

        let end_time = data["end_time"]
            .as_str()
            .ok_or_else(|| anyhow!("End time is required"))?
            .to_string();

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

        let priority = match data["priority"].as_str().unwrap_or("Medium") {
            "Low" => Priority::Low,
            "High" => Priority::High,
            "Urgent" => Priority::Urgent,
            _ => Priority::Medium,
        };

        Ok(EventData {
            title,
            description,
            start_time,
            end_time,
            location,
            attendees,
            priority,
        })
    }
}

// オフライン用のモックLLMクライアント
pub struct MockLLMClient;

impl MockLLMClient {
    pub fn new() -> Self {
        Self
    }

    pub fn process_request(&self, request: LLMRequest) -> Result<LLMResponse> {
        // 簡単なパターンマッチングで基本的な機能を提供
        let input = request.user_input.to_lowercase();
        
        if input.contains("予定") && (input.contains("作成") || input.contains("追加")) {
            Ok(LLMResponse {
                action: ActionType::CreateEvent,
                event_data: Some(EventData {
                    title: "新しい予定".to_string(),
                    description: Some("LLMで解析された予定".to_string()),
                    start_time: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                    end_time: (Utc::now() + chrono::Duration::hours(1)).format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                    location: None,
                    attendees: Vec::new(),
                    priority: Priority::Medium,
                }),
                response_text: "新しい予定を作成しました。".to_string(),
            })
        } else if input.contains("一覧") || input.contains("リスト") {
            Ok(LLMResponse {
                action: ActionType::ListEvents,
                event_data: None,
                response_text: "予定一覧を表示します。".to_string(),
            })
        } else {
            Ok(LLMResponse {
                action: ActionType::GeneralResponse,
                event_data: None,
                response_text: "申し訳ございませんが、その要求を理解できませんでした。".to_string(),
            })
        }
    }
}