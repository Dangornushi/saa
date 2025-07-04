use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LLMConfig,
    pub calendar: CalendarConfig,
    #[serde(default)]
    pub google_calendar: Option<GoogleCalendarConfig>,
    pub app: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub gemini_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    // 他のカレンダープロバイダーのフィールドを追加可能
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCalendarConfig {
    pub client_secret_path: Option<String>,
    pub token_cache_path: Option<String>,
    pub calendar_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub data_dir: Option<String>,
    pub backup_count: Option<usize>,
    pub auto_backup: Option<bool>,
    pub verbose: Option<bool>,
    pub debug_mode: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LLMConfig {
                base_url: Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
                model: Some("gemini-2.5-flash".to_string()),
                temperature: Some(0.7),
                max_tokens: Some(1000),
                gemini_api_key: None,
            },
            calendar: CalendarConfig {
            },
            google_calendar: Some(GoogleCalendarConfig {
                client_secret_path: Some("client_secret.json".to_string()),
                token_cache_path: Some("token_cache.json".to_string()),
                calendar_id: Some("primary".to_string()),
            }),
            app: AppConfig {
                data_dir: None,
                backup_count: Some(5),
                auto_backup: Some(true),
                verbose: Some(false),
                debug_mode: Some(false),
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
        // LLM設定 (Geminiに特化)
        if let Ok(base_url) = env::var("GEMINI_BASE_URL") {
            config.llm.base_url = Some(base_url);
        }
        if let Ok(model) = env::var("LLM_MODEL") {
            config.llm.model = Some(model);
        }
        if let Ok(gemini_api_key) = env::var("GEMINI_API_KEY") {
            config.llm.gemini_api_key = Some(gemini_api_key);
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

        // LLM APIキー (Geminiに特化)
        if let Some(gemini_key) = secrets.get("gemini_api_key").and_then(|v| v.as_str()) {
            config.llm.gemini_api_key = Some(gemini_key.to_string());
        }

        Ok(())
    }

    fn load_api_keys_file(&self, api_keys_file: &Path, config: &mut Config) -> Result<()> {
        let content = fs::read_to_string(api_keys_file)?;
        let api_keys: toml::Value = toml::from_str(&content)?;

        // LLM APIキー (Geminiに特化)
        if let Some(gemini_key) = api_keys.get("gemini_api_key").and_then(|v| v.as_str()) {
            config.llm.gemini_api_key = Some(gemini_key.to_string());
        }

        Ok(())
    }

    fn create_sample_config_content(&self) -> String {
        r#"# Schedule AI Agent Configuration File
# This is a sample configuration file. Copy this to config.toml and customize as needed.

[llm]
# LLM Provider: Gemini (default)

# API Base URL for Gemini
# base_url = "https://generativelanguage.googleapis.com/v1beta"

# Model to use for Gemini
# model = "gemini-2.5-flash"

# Temperature for response generation (0.0 to 2.0, default: 0.7)
# temperature = 0.7

# Maximum tokens in response (default: 1000)
# max_tokens = 1000

[calendar]
# 他のカレンダープロバイダーの設定
# 将来的に他のカレンダーサービスに対応する場合は、ここに設定を追加

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

# Gemini API Key
GEMINI_API_KEY=AIzaSyAWDoC7udFRxe95Gvp0vBKv55PaIdSzyqE

# Optional: Custom API base URL for Gemini
# GEMINI_BASE_URL=https://generativelanguage.googleapis.com/v1beta

# Optional: LLM Model for Gemini
# LLM_MODEL=gemini-2.5-flash

# カレンダー統合設定（将来的な拡張用）
# CALENDAR_PROVIDER=none
"#;
        fs::write(&env_example, env_content)?;
        created_files.push(env_example);

        // secrets.json ファイルの例
        let secrets_example = self.config_dir.join("secrets.example.json");
        let secrets_content = r#"{
  "gemini_api_key": "AIzaSyAWDoC7udFRxe95Gvp0vBKv55PaIdSzyqE"
}
"#;
        fs::write(&secrets_example, secrets_content)?;
        created_files.push(secrets_example);

        // api_keys.toml ファイルの例
        let api_keys_example = self.config_dir.join("api_keys.example.toml");
        let api_keys_content = r#"# API Keys Configuration
# Copy this file to api_keys.toml and set your actual keys

# Gemini API Key
gemini_api_key = "AIzaSyAWDoC7udFRxe95Gvp0vBKv55PaIdSzyqE"
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
