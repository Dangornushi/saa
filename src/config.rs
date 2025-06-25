use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LLMConfig,
    pub calendar: CalendarConfig,
    pub app: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: Option<String>, // "openai", "github_copilot", "azure_openai"
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub github_token: Option<String>, // GitHub Personal Access Token for Copilot
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub google: Option<GoogleCalendarConfig>,
    pub notion: Option<NotionCalendarConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCalendarConfig {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub calendar_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionCalendarConfig {
    pub api_key: Option<String>,
    pub database_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub data_dir: Option<String>,
    pub backup_count: Option<usize>,
    pub auto_backup: Option<bool>,
    pub verbose: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LLMConfig {
                provider: None,
                api_key: None,
                base_url: Some("https://api.openai.com/v1".to_string()),
                model: Some("gpt-3.5-turbo".to_string()),
                temperature: Some(0.7),
                max_tokens: Some(1000),
                github_token: None,
            },
            calendar: CalendarConfig {
                google: None,
                notion: None,
            },
            app: AppConfig {
                data_dir: None,
                backup_count: Some(5),
                auto_backup: Some(true),
                verbose: Some(false),
            },
        }
    }
}

pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_dir = Self::get_config_directory()?;
        let config_file = config_dir.join("config.toml");

        // 設定ディレクトリが存在しない場合は作成
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(Self {
            config_dir,
            config_file,
        })
    }

    pub fn load_config(&self) -> Result<Config> {
        // 1. 設定ファイルから読み込み
        let mut config = if self.config_file.exists() {
            self.load_from_file(&self.config_file)?
        } else {
            // デフォルト設定を作成して保存
            let default_config = Config::default();
            self.save_config(&default_config)?;
            default_config
        };

        // 2. 環境変数で上書き
        self.override_with_env_vars(&mut config);

        // 3. 追加の設定ファイルをチェック
        self.load_additional_configs(&mut config)?;

        Ok(config)
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        let toml_content = toml::to_string_pretty(config)?;
        fs::write(&self.config_file, toml_content)?;
        Ok(())
    }

    pub fn create_sample_config(&self) -> Result<PathBuf> {
        let sample_file = self.config_dir.join("config.sample.toml");
        let sample_config = self.create_sample_config_content();
        fs::write(&sample_file, sample_config)?;
        Ok(sample_file)
    }

    fn load_from_file(&self, file_path: &Path) -> Result<Config> {
        let content = fs::read_to_string(file_path)?;

        // ファイル拡張子に基づいて形式を判定
        match file_path.extension().and_then(|s| s.to_str()) {
            Some("toml") => {
                toml::from_str(&content).map_err(|e| anyhow!("TOML parse error: {}", e))
            }
            Some("json") => {
                serde_json::from_str(&content).map_err(|e| anyhow!("JSON parse error: {}", e))
            }
            _ => {
                // デフォルトでTOMLとして解析を試行
                toml::from_str(&content).map_err(|e| anyhow!("Config parse error: {}", e))
            }
        }
    }

    fn override_with_env_vars(&self, config: &mut Config) {
        // LLM設定
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            config.llm.api_key = Some(api_key);
        }
        if let Ok(api_key) = env::var("LLM_API_KEY") {
            config.llm.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("OPENAI_BASE_URL") {
            config.llm.base_url = Some(base_url);
        }
        if let Ok(model) = env::var("LLM_MODEL") {
            config.llm.model = Some(model);
        }
        if let Ok(github_token) = env::var("GITHUB_TOKEN") {
            config.llm.github_token = Some(github_token);
        }
        if let Ok(github_token) = env::var("GITHUB_COPILOT_TOKEN") {
            config.llm.github_token = Some(github_token);
        }

        // Google Calendar設定
        if let Ok(token) = env::var("GOOGLE_CALENDAR_ACCESS_TOKEN") {
            if config.calendar.google.is_none() {
                config.calendar.google = Some(GoogleCalendarConfig {
                    access_token: None,
                    refresh_token: None,
                    client_id: None,
                    client_secret: None,
                    calendar_id: None,
                });
            }
            if let Some(ref mut google_config) = config.calendar.google {
                google_config.access_token = Some(token);
            }
        }
        if let Ok(calendar_id) = env::var("GOOGLE_CALENDAR_ID") {
            if config.calendar.google.is_none() {
                config.calendar.google = Some(GoogleCalendarConfig {
                    access_token: None,
                    refresh_token: None,
                    client_id: None,
                    client_secret: None,
                    calendar_id: None,
                });
            }
            if let Some(ref mut google_config) = config.calendar.google {
                google_config.calendar_id = Some(calendar_id);
            }
        }

        // Notion設定
        if let Ok(api_key) = env::var("NOTION_API_KEY") {
            if config.calendar.notion.is_none() {
                config.calendar.notion = Some(NotionCalendarConfig {
                    api_key: None,
                    database_id: None,
                });
            }
            if let Some(ref mut notion_config) = config.calendar.notion {
                notion_config.api_key = Some(api_key);
            }
        }
        if let Ok(database_id) = env::var("NOTION_DATABASE_ID") {
            if config.calendar.notion.is_none() {
                config.calendar.notion = Some(NotionCalendarConfig {
                    api_key: None,
                    database_id: None,
                });
            }
            if let Some(ref mut notion_config) = config.calendar.notion {
                notion_config.database_id = Some(database_id);
            }
        }
    }

    fn load_additional_configs(&self, config: &mut Config) -> Result<()> {
        // .env ファイルの読み込み
        let env_file = self.config_dir.join(".env");
        if env_file.exists() {
            self.load_env_file(&env_file)?;
            // 環境変数を再度適用
            self.override_with_env_vars(config);
        }

        // secrets.json ファイルの読み込み
        let secrets_file = self.config_dir.join("secrets.json");
        if secrets_file.exists() {
            self.load_secrets_file(&secrets_file, config)?;
        }

        // api_keys.toml ファイルの読み込み
        let api_keys_file = self.config_dir.join("api_keys.toml");
        if api_keys_file.exists() {
            self.load_api_keys_file(&api_keys_file, config)?;
        }

        Ok(())
    }

    fn load_env_file(&self, env_file: &Path) -> Result<()> {
        let content = fs::read_to_string(env_file)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                env::set_var(key, value);
            }
        }
        Ok(())
    }

    fn load_secrets_file(&self, secrets_file: &Path, config: &mut Config) -> Result<()> {
        let content = fs::read_to_string(secrets_file)?;
        let secrets: serde_json::Value = serde_json::from_str(&content)?;

        // LLM APIキー
        if let Some(llm_key) = secrets.get("llm_api_key").and_then(|v| v.as_str()) {
            config.llm.api_key = Some(llm_key.to_string());
        }
        if let Some(openai_key) = secrets.get("openai_api_key").and_then(|v| v.as_str()) {
            config.llm.api_key = Some(openai_key.to_string());
        }

        // Google Calendar
        if let Some(google_secrets) = secrets.get("google_calendar") {
            if config.calendar.google.is_none() {
                config.calendar.google = Some(GoogleCalendarConfig {
                    access_token: None,
                    refresh_token: None,
                    client_id: None,
                    client_secret: None,
                    calendar_id: None,
                });
            }
            if let Some(ref mut google_config) = config.calendar.google {
                if let Some(token) = google_secrets.get("access_token").and_then(|v| v.as_str()) {
                    google_config.access_token = Some(token.to_string());
                }
                if let Some(refresh_token) =
                    google_secrets.get("refresh_token").and_then(|v| v.as_str())
                {
                    google_config.refresh_token = Some(refresh_token.to_string());
                }
                if let Some(client_id) = google_secrets.get("client_id").and_then(|v| v.as_str()) {
                    google_config.client_id = Some(client_id.to_string());
                }
                if let Some(client_secret) =
                    google_secrets.get("client_secret").and_then(|v| v.as_str())
                {
                    google_config.client_secret = Some(client_secret.to_string());
                }
            }
        }

        // Notion
        if let Some(notion_secrets) = secrets.get("notion") {
            if config.calendar.notion.is_none() {
                config.calendar.notion = Some(NotionCalendarConfig {
                    api_key: None,
                    database_id: None,
                });
            }
            if let Some(ref mut notion_config) = config.calendar.notion {
                if let Some(api_key) = notion_secrets.get("api_key").and_then(|v| v.as_str()) {
                    notion_config.api_key = Some(api_key.to_string());
                }
                if let Some(database_id) =
                    notion_secrets.get("database_id").and_then(|v| v.as_str())
                {
                    notion_config.database_id = Some(database_id.to_string());
                }
            }
        }

        Ok(())
    }

    fn load_api_keys_file(&self, api_keys_file: &Path, config: &mut Config) -> Result<()> {
        let content = fs::read_to_string(api_keys_file)?;
        let api_keys: toml::Value = toml::from_str(&content)?;

        // LLM APIキー
        if let Some(llm_key) = api_keys.get("llm_api_key").and_then(|v| v.as_str()) {
            config.llm.api_key = Some(llm_key.to_string());
        }
        if let Some(openai_key) = api_keys.get("openai_api_key").and_then(|v| v.as_str()) {
            config.llm.api_key = Some(openai_key.to_string());
        }

        // Google Calendar APIキー
        if let Some(google_token) = api_keys
            .get("google_calendar_access_token")
            .and_then(|v| v.as_str())
        {
            if config.calendar.google.is_none() {
                config.calendar.google = Some(GoogleCalendarConfig {
                    access_token: None,
                    refresh_token: None,
                    client_id: None,
                    client_secret: None,
                    calendar_id: None,
                });
            }
            if let Some(ref mut google_config) = config.calendar.google {
                google_config.access_token = Some(google_token.to_string());
            }
        }

        // Notion APIキー
        if let Some(notion_key) = api_keys.get("notion_api_key").and_then(|v| v.as_str()) {
            if config.calendar.notion.is_none() {
                config.calendar.notion = Some(NotionCalendarConfig {
                    api_key: None,
                    database_id: None,
                });
            }
            if let Some(ref mut notion_config) = config.calendar.notion {
                notion_config.api_key = Some(notion_key.to_string());
            }
        }

        Ok(())
    }

    fn create_sample_config_content(&self) -> String {
        r#"# Schedule AI Agent Configuration File
# This is a sample configuration file. Copy this to config.toml and customize as needed.

[llm]
# LLM Provider: "openai", "github_copilot", "azure_openai" (default: openai)
# provider = "github_copilot"

# API Key (for OpenAI or Azure OpenAI)
# api_key = "sk-your-openai-api-key-here"

# GitHub Personal Access Token (for GitHub Copilot)
# github_token = "ghp_your-github-token-here"

# API Base URL
# For OpenAI: https://api.openai.com/v1
# For GitHub Copilot: https://api.githubcopilot.com
# For Azure OpenAI: https://your-resource.openai.azure.com
# base_url = "https://api.githubcopilot.com"

# Model to use
# OpenAI: gpt-3.5-turbo, gpt-4, gpt-4-turbo
# GitHub Copilot: gpt-3.5-turbo, gpt-4, gpt-4-turbo, claude-3-5-sonnet, o1-preview, o1-mini
# model = "gpt-4"

# Temperature for response generation (0.0 to 2.0, default: 0.7)
# temperature = 0.7

# Maximum tokens in response (default: 1000)
# max_tokens = 1000

[calendar.google]
# Google Calendar API credentials
# access_token = "your-google-access-token"
# refresh_token = "your-google-refresh-token"
# client_id = "your-google-client-id"
# client_secret = "your-google-client-secret"
# calendar_id = "primary"

[calendar.notion]
# Notion API credentials
# api_key = "your-notion-api-key"
# database_id = "your-notion-database-id"

[app]
# Application settings
# data_dir = "~/.schedule_ai_agent"
# backup_count = 5
# auto_backup = true
# verbose = false
"#
        .to_string()
    }

    fn get_config_directory() -> Result<PathBuf> {
        // ホームディレクトリ内にアプリケーション専用の設定ディレクトリを作成
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow!("ホームディレクトリが見つかりません"))?;

        Ok(home_dir.join(".schedule_ai_agent"))
    }

    pub fn get_config_directory_path(&self) -> &Path {
        &self.config_dir
    }

    pub fn get_config_file_path(&self) -> &Path {
        &self.config_file
    }

    pub fn config_exists(&self) -> bool {
        self.config_file.exists()
    }

    pub fn create_example_files(&self) -> Result<Vec<PathBuf>> {
        let mut created_files = Vec::new();

        // サンプル設定ファイル
        let sample_config = self.create_sample_config()?;
        created_files.push(sample_config);

        // .env ファイルの例
        let env_example = self.config_dir.join("env.example");
        let env_content = r#"# Environment Variables for Schedule AI Agent
# Copy this file to .env and set your actual values

# OpenAI API Key (for OpenAI provider)
OPENAI_API_KEY=sk-your-openai-api-key-here

# GitHub Personal Access Token (for GitHub Copilot provider)
# GITHUB_TOKEN=ghp_your-github-token-here

# Optional: Custom API base URL
# OPENAI_BASE_URL=https://api.openai.com/v1
# GITHUB_COPILOT_BASE_URL=https://api.githubcopilot.com

# Optional: LLM Model
# LLM_MODEL=gpt-4

# Google Calendar
# GOOGLE_CALENDAR_ACCESS_TOKEN=your-google-access-token
# GOOGLE_CALENDAR_ID=primary

# Notion
# NOTION_API_KEY=your-notion-api-key
# NOTION_DATABASE_ID=your-notion-database-id
"#;
        fs::write(&env_example, env_content)?;
        created_files.push(env_example);

        // secrets.json ファイルの例
        let secrets_example = self.config_dir.join("secrets.example.json");
        let secrets_content = r#"{
  "llm_api_key": "sk-your-openai-api-key-here",
  "google_calendar": {
    "access_token": "your-google-access-token",
    "refresh_token": "your-google-refresh-token",
    "client_id": "your-google-client-id",
    "client_secret": "your-google-client-secret"
  },
  "notion": {
    "api_key": "your-notion-api-key",
    "database_id": "your-notion-database-id"
  }
}
"#;
        fs::write(&secrets_example, secrets_content)?;
        created_files.push(secrets_example);

        // api_keys.toml ファイルの例
        let api_keys_example = self.config_dir.join("api_keys.example.toml");
        let api_keys_content = r#"# API Keys Configuration
# Copy this file to api_keys.toml and set your actual keys

# LLM API Key
llm_api_key = "sk-your-openai-api-key-here"

# Google Calendar Access Token
google_calendar_access_token = "your-google-access-token"

# Notion API Key
notion_api_key = "your-notion-api-key"
"#;
        fs::write(&api_keys_example, api_keys_content)?;
        created_files.push(api_keys_example);

        Ok(created_files)
    }
}

// dirsクレートの代替実装（依存関係を減らすため）
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}
