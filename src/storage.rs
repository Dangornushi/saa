use crate::models::{Schedule, ConversationHistory};
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Storage {
    data_dir: PathBuf,
    schedule_file: PathBuf,
    conversation_file: PathBuf,
}

impl Storage {
    pub fn new() -> Result<Self> {
        let data_dir = Self::get_data_directory()?;
        let schedule_file = data_dir.join("schedule.json");
        let conversation_file = data_dir.join("conversation_history.json");

        // データディレクトリが存在しない場合は作成
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
            println!("データディレクトリを作成しました: {}", data_dir.display());
        }

        Ok(Self {
            data_dir,
            schedule_file,
            conversation_file,
        })
    }

    pub fn save_schedule(&self, schedule: &Schedule) -> Result<()> {
        let json_data = serde_json::to_string_pretty(schedule)?;
        println!("スケジュールを保存: {}", self.schedule_file.display());
        fs::write(&self.schedule_file, json_data)?;
        Ok(())
    }

    pub fn load_schedule(&self) -> Result<Schedule> {
        if !self.schedule_file.exists() {
            return Ok(Schedule::new());
        }

        let json_data = fs::read_to_string(&self.schedule_file)?;
        let schedule: Schedule = serde_json::from_str(&json_data)?;
        Ok(schedule)
    }

    pub fn backup_schedule(&self) -> Result<PathBuf> {
        if !self.schedule_file.exists() {
            return Err(anyhow!("バックアップするスケジュールファイルが存在しません"));
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_file = self.data_dir.join(format!("schedule_backup_{}.json", timestamp));

        fs::copy(&self.schedule_file, &backup_file)?;
        Ok(backup_file)
    }

    pub fn restore_schedule(&self, backup_file: &Path) -> Result<()> {
        if !backup_file.exists() {
            return Err(anyhow!("指定されたバックアップファイルが存在しません"));
        }

        // 現在のファイルをバックアップ
        if self.schedule_file.exists() {
            let _ = self.backup_schedule();
        }

        fs::copy(backup_file, &self.schedule_file)?;
        Ok(())
    }

    pub fn export_schedule(&self, export_path: &Path) -> Result<()> {
        if !self.schedule_file.exists() {
            return Err(anyhow!("エクスポートするスケジュールファイルが存在しません"));
        }

        fs::copy(&self.schedule_file, export_path)?;
        Ok(())
    }

    pub fn import_schedule(&self, import_path: &Path) -> Result<Schedule> {
        if !import_path.exists() {
            return Err(anyhow!("インポートするファイルが存在しません"));
        }

        let json_data = fs::read_to_string(import_path)?;
        let schedule: Schedule = serde_json::from_str(&json_data)?;
        Ok(schedule)
    }

    pub fn save_conversation_history(&self, conversation: &ConversationHistory) -> Result<()> {
        let json_data = serde_json::to_string_pretty(conversation)?;
        println!("会話履歴を保存: {}", self.conversation_file.display());
        fs::write(&self.conversation_file, json_data)?;
        Ok(())
    }

    pub fn load_conversation_history(&self) -> Result<ConversationHistory> {
        if !self.conversation_file.exists() {
            return Ok(ConversationHistory::new());
        }

        let json_data = fs::read_to_string(&self.conversation_file)?;
        let conversation: ConversationHistory = serde_json::from_str(&json_data)?;
        Ok(conversation)
    }

    pub fn clear_conversation_history(&self) -> Result<()> {
        if self.conversation_file.exists() {
            fs::remove_file(&self.conversation_file)?;
            println!("会話履歴をクリアしました");
        }
        Ok(())
    }

    pub fn list_backups(&self) -> Result<Vec<PathBuf>> {
        let mut backups = Vec::new();

        if !self.data_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if let Some(filename_str) = filename.to_str() {
                        if filename_str.starts_with("schedule_backup_") && filename_str.ends_with(".json") {
                            backups.push(path);
                        }
                    }
                }
            }
        }

        // 日付順でソート（新しいものが先）
        backups.sort_by(|a, b| {
            let a_metadata = fs::metadata(a).ok();
            let b_metadata = fs::metadata(b).ok();

            match (a_metadata, b_metadata) {
                (Some(a_meta), Some(b_meta)) => {
                    b_meta.modified().unwrap_or(std::time::UNIX_EPOCH)
                        .cmp(&a_meta.modified().unwrap_or(std::time::UNIX_EPOCH))
                }
                _ => std::cmp::Ordering::Equal,
            }
        });

        Ok(backups)
    }

    fn get_data_directory() -> Result<PathBuf> {
        // ホームディレクトリ内にアプリケーション専用のディレクトリを作成
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("ホームディレクトリが見つかりません"))?;

        Ok(home_dir.join(".schedule_ai_agent"))
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