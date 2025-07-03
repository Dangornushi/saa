use std::io::{stdout, Stdout};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use unicode_segmentation::UnicodeSegmentation;
use ratatui::backend::Backend;

use crate::scheduler::Scheduler;

pub struct ChatApp {
    /// 現在の入力
    input: String,
    /// カーソル位置
    cursor_position: usize,
    /// メッセージ履歴
    messages: Vec<ChatMessage>,
    /// アプリケーションが終了すべきかどうか
    should_quit: bool,
    /// スケジューラーへの参照
    scheduler: Scheduler,
    /// 処理中フラグ
    is_processing: bool,
    /// ヘルプが表示されているかどうか
    show_help: bool,
    /// メッセージリストのスクロール状態
    scroll_state: ratatui::widgets::ListState,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

#[derive(Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// UTF-8文字列の安全な操作のためのヘルパー関数
impl ChatApp {
    /// 文字単位でのカーソル位置を取得
    fn char_count_to_byte_index(&self, char_pos: usize) -> usize {
        self.input
            .graphemes(true)
            .take(char_pos)
            .map(|g| g.len())
            .sum()
    }

    /// 文字数を取得
    fn char_count(&self) -> usize {
        self.input.graphemes(true).count()
    }

    /// 安全に文字を挿入
    fn insert_char_at_cursor(&mut self, c: char) {
        let byte_index = self.char_count_to_byte_index(self.cursor_position);
        self.input.insert(byte_index, c);
        self.cursor_position += 1;
    }

    /// 安全に文字を削除（Backspace）
    fn delete_char_before_cursor(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            let byte_index = self.char_count_to_byte_index(self.cursor_position);
            
            // 次の文字の境界を見つける
            if let Some(next_char_boundary) = self.input.get(byte_index..).and_then(|s| {
                s.graphemes(true).next().map(|g| byte_index + g.len())
            }) {
                self.input.drain(byte_index..next_char_boundary);
            }
        }
    }

    /// 文字列の表示幅を計算（絵文字やワイド文字を考慮）
    fn calculate_display_width(&self, text: &str) -> usize {
        text.graphemes(true)
            .map(|g| {
                // ASCII文字は確実に幅1
                if g.chars().all(|c| c.is_ascii()) {
                    return 1;
                }
                
                // 絵文字や記号の幅判定を簡素化
                match g.chars().next() {
                    Some(c) => {
                        match c as u32 {
                            // 一般的な絵文字
                            0x1F600..=0x1F64F | // Emoticons
                            0x1F300..=0x1F5FF | // Misc Symbols and Pictographs
                            0x1F680..=0x1F6FF | // Transport and Map
                            0x1F1E6..=0x1F1FF | // Regional indicators
                            0x2600..=0x26FF   | // Misc symbols
                            0x2700..=0x27BF   | // Dingbats
                            0x1F900..=0x1F9FF   // Supplemental Symbols and Pictographs
                            => 2,
                            // 日本語文字（ひらがな、カタカナ、漢字）
                            0x3040..=0x309F | // ひらがな
                            0x30A0..=0x30FF | // カタカナ
                            0x4E00..=0x9FAF   // CJK統合漢字
                            => 2,
                            // その他は幅1
                            _ => 1,
                        }
                    }
                    None => 0,
                }
            })
            .sum()
    }

    /// メッセージ内容を指定された幅で適切に折り返す
    fn wrap_message_content(&self, content: &str, width: usize) -> String {
        // 最小幅を確保
        let safe_width = width.max(10);
        
        let mut wrapped_lines = Vec::new();
        
        for line in content.lines() {
            // 表示幅を計算
            let line_width = self.calculate_display_width(line);
            
            if line_width <= safe_width {
                wrapped_lines.push(line.to_string());
            } else {
                // 長い行は単語単位で分割を試行
                let words: Vec<&str> = line.split_whitespace().collect();
                if words.is_empty() {
                    wrapped_lines.push(String::new());
                    continue;
                }

                let mut current_line = String::new();
                let mut current_width = 0;
                
                for word in words {
                    let word_width = self.calculate_display_width(word);
                    let space_width = if current_line.is_empty() { 0 } else { 1 };
                    
                    if current_width + space_width + word_width <= safe_width {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                            current_width += 1;
                        }
                        current_line.push_str(word);
                        current_width += word_width;
                    } else {
                        // 現在の行を確定
                        if !current_line.is_empty() {
                            wrapped_lines.push(current_line);
                        }
                        
                        // 単語が制限幅より長い場合は文字単位で強制分割
                        if word_width > safe_width {
                            let split_lines = self.force_split_text(word, safe_width);
                            wrapped_lines.extend(split_lines);
                            current_line = String::new();
                            current_width = 0;
                        } else {
                            current_line = word.to_string();
                            current_width = word_width;
                        }
                    }
                }
                
                if !current_line.is_empty() {
                    wrapped_lines.push(current_line);
                }
            }
        }
        wrapped_lines.join("\n")
    }

    /// テキストを強制的に指定幅で分割する
    fn force_split_text(&self, text: &str, max_width: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0;
        
        for grapheme in text.graphemes(true) {
            let grapheme_width = self.calculate_display_width(grapheme);
            
            if current_width + grapheme_width <= max_width {
                current_line.push_str(grapheme);
                current_width += grapheme_width;
            } else {
                if !current_line.is_empty() {
                    result.push(current_line);
                }
                current_line = grapheme.to_string();
                current_width = grapheme_width;
            }
        }
        
        if !current_line.is_empty() {
            result.push(current_line);
        }
        
        result
    }

    /// 行を指定された幅で切り詰める
    fn truncate_line(&self, line: &str, max_width: usize) -> String {
        let mut result = String::new();
        let mut current_width = 0;
        
        for grapheme in line.graphemes(true) {
            let grapheme_width = self.calculate_display_width(grapheme);
            if current_width + grapheme_width <= max_width {
                result.push_str(grapheme);
                current_width += grapheme_width;
            } else {
                break;
            }
        }
        
        result
    }
}

impl ChatApp {
    pub fn new(scheduler: Scheduler) -> Self {
        let mut messages = Vec::new();
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: "スケジュールAIアシスタントへようこそ!\n\n以下のことができます:\n• 予定の追加・変更・削除\n• 空き時間の確認\n• スケジュールの最適化\n• 自然言語での予定管理\n\n入力して Enter を押すか、Ctrl+H でヘルプを表示してください。".to_string(),
            timestamp: chrono::Local::now(),
        });
        
        let mut scroll_state = ListState::default();
        // 初期状態では選択なしにして、背景色の反転を避ける
        scroll_state.select(None);
        
        Self {
            input: String::new(),
            cursor_position: 0,
            messages,
            should_quit: false,
            scheduler,
            is_processing: false,
            show_help: false,
            scroll_state,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // ターミナルセットアップ
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal).await;

        // ターミナルクリーンアップ
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            // 描画前にスクロール状態をチェック
            let should_stay_at_bottom = self.scroll_state.selected().is_none() || 
                self.scroll_state.selected().map_or(true, |selected| {
                    selected >= self.messages.len().saturating_sub(2)
                });
            
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Min(5),     // メッセージエリア（最小5行確保）
                        Constraint::Length(3),  // 入力エリア
                        Constraint::Length(1),  // ステータスバー
                    ])
                    .split(f.size());

                // スクロール状態のクローンを作成
                let mut local_scroll_state = self.scroll_state.clone();
                
                // 最下部に留まるべき場合は選択をクリア
                if should_stay_at_bottom {
                    local_scroll_state.select(None);
                }
                
                self.render_messages_with_state(f, chunks[0], &mut local_scroll_state);
                self.render_input(f, chunks[1]);
                self.render_status_bar(f, chunks[2]);
                
                // スクロール状態を更新
                self.scroll_state = local_scroll_state;

                if self.show_help {
                    self.render_help(f);
                }
            })?;
            
            // 描画後にターミナルをフラッシュして画面更新を確実にする
            terminal.backend_mut().flush()?;

            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    // KeyEventKindが押下の場合のみ処理
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key.code {
                        KeyCode::Esc => {
                            if self.show_help {
                                self.show_help = false;
                            } else {
                                self.should_quit = true;
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.should_quit = true;
                        }
                        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.show_help = !self.show_help;
                        }
                        KeyCode::Enter => {
                            if !self.show_help && !self.is_processing {
                                let input_text = self.input.trim().to_string();
                                if !input_text.is_empty() {
                                    // 先にユーザーメッセージを追加して画面に表示
                                    self.messages.push(ChatMessage {
                                        role: MessageRole::User,
                                        content: input_text.clone(),
                                        timestamp: chrono::Local::now(),
                                    });

                                    // 入力をクリアして最下部にスクロール
                                    self.input.clear();
                                    self.cursor_position = 0;
                                    self.update_scroll_to_bottom();
                                    
                                    // 処理中メッセージを追加
                                    self.messages.push(ChatMessage {
                                        role: MessageRole::Assistant,
                                        content: "🤔 考え中です...".to_string(),
                                        timestamp: chrono::Local::now(),
                                    });
                                    
                                    self.is_processing = true;
                                    self.update_scroll_to_bottom();
                                    
                                    // 画面を一度描画して処理中メッセージを表示
                                    terminal.draw(|f| {
                                        let chunks = Layout::default()
                                            .direction(Direction::Vertical)
                                            .margin(1)
                                            .constraints([
                                                Constraint::Min(5),
                                                Constraint::Length(3),
                                                Constraint::Length(1),
                                            ])
                                            .split(f.size());

                                        let mut scroll_state_clone = self.scroll_state.clone();
                                        self.render_messages_with_state(f, chunks[0], &mut scroll_state_clone);
                                        self.render_input(f, chunks[1]);
                                        self.render_status_bar(f, chunks[2]);
                                        self.scroll_state = scroll_state_clone;

                                        if self.show_help {
                                            self.render_help(f);
                                        }
                                    })?;
                                    terminal.backend_mut().flush()?;
                                    
                                    // AIの処理を実行
                                    let processing_msg_index = self.messages.len() - 1;
                                    match self.scheduler.process_user_input(input_text).await {
                                        Ok(response) => {
                                            let cleaned_response = self.clean_response(&response);
                                            if let Some(msg) = self.messages.get_mut(processing_msg_index) {
                                                msg.content = if cleaned_response.is_empty() {
                                                    "✅ 処理が完了しました。".to_string()
                                                } else {
                                                    cleaned_response
                                                };
                                                msg.timestamp = chrono::Local::now();
                                            }
                                        }
                                        Err(e) => {
                                            if let Some(msg) = self.messages.get_mut(processing_msg_index) {
                                                msg.content = format!("❌ エラーが発生しました:\n{}\n\n💡 別の方法で試してみてください。", e);
                                                msg.timestamp = chrono::Local::now();
                                            }
                                        }
                                    }
                                    
                                    self.is_processing = false;
                                    self.update_scroll_to_bottom();
                                    
                                    // AI処理完了後の画面更新を即座に反映
                                    terminal.draw(|f| {
                                        let chunks = Layout::default()
                                            .direction(Direction::Vertical)
                                            .margin(1)
                                            .constraints([
                                                Constraint::Min(5),
                                                Constraint::Length(3),
                                                Constraint::Length(1),
                                            ])
                                            .split(f.size());

                                        let mut scroll_state_clone = self.scroll_state.clone();
                                        self.render_messages_with_state(f, chunks[0], &mut scroll_state_clone);
                                        self.render_input(f, chunks[1]);
                                        self.render_status_bar(f, chunks[2]);
                                        self.scroll_state = scroll_state_clone;

                                        if self.show_help {
                                            self.render_help(f);
                                        }
                                    })?;
                                    terminal.backend_mut().flush()?;
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            if !self.show_help && !self.is_processing {
                                self.insert_char_at_cursor(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if !self.show_help && !self.is_processing && self.cursor_position > 0 {
                                self.delete_char_before_cursor();
                            }
                        }
                        KeyCode::Left => {
                            if !self.show_help && self.cursor_position > 0 {
                                self.cursor_position -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if !self.show_help && self.cursor_position < self.char_count() {
                                self.cursor_position += 1;
                            }
                        }
                        KeyCode::Up => {
                            if !self.show_help && !self.messages.is_empty() {
                                let current = self.scroll_state.selected().unwrap_or(self.messages.len().saturating_sub(1));
                                if current > 0 {
                                    self.scroll_state.select(Some(current - 1));
                                }
                            }
                        }
                        KeyCode::Down => {
                            if !self.show_help && !self.messages.is_empty() {
                                let current = self.scroll_state.selected().unwrap_or(0);
                                let max_index = self.messages.len().saturating_sub(1);
                                if current < max_index {
                                    self.scroll_state.select(Some(current + 1));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    async fn handle_user_input(&mut self, input: String) -> Result<()> {
        // ユーザーメッセージを追加
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: input.clone(),
            timestamp: chrono::Local::now(),
        });

        // 処理中メッセージを表示
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "🤔 考え中です...".to_string(),
            timestamp: chrono::Local::now(),
        });

        // 新しいメッセージが追加されたので最下部にスクロール
        self.update_scroll_to_bottom();
        self.is_processing = true;

        // 最後のメッセージのインデックス（処理中メッセージ）
        let processing_msg_index = self.messages.len() - 1;

        // AIの応答を取得
        match self.scheduler.process_user_input(input).await {
            Ok(response) => {
                // AIの応答をクリーンアップ
                let cleaned_response = self.clean_response(&response);
                
                // 処理中メッセージを実際の応答に置き換え
                if let Some(msg) = self.messages.get_mut(processing_msg_index) {
                    msg.content = if cleaned_response.is_empty() {
                        "✅ 処理が完了しました。".to_string()
                    } else {
                        cleaned_response
                    };
                    msg.timestamp = chrono::Local::now();
                }
            }
            Err(e) => {
                // 処理中メッセージをエラーメッセージに置き換え
                if let Some(msg) = self.messages.get_mut(processing_msg_index) {
                    msg.content = format!("❌ エラーが発生しました:\n{}\n\n💡 別の方法で試してみてください。", e);
                    msg.timestamp = chrono::Local::now();
                }
            }
        }

        self.is_processing = false;
        // メッセージ更新後に最下部を表示
        self.update_scroll_to_bottom();
        Ok(())
    }

    /// ユーザーメッセージが既に追加されている状態で処理を行う
    async fn handle_user_input_with_existing_message(&mut self, input: String) -> Result<()> {
        // 処理中メッセージを表示
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "🤔 考え中です...".to_string(),
            timestamp: chrono::Local::now(),
        });

        // 新しいメッセージが追加されたので最下部にスクロール
        self.update_scroll_to_bottom();
        self.is_processing = true;

        // 最後のメッセージのインデックス（処理中メッセージ）
        let processing_msg_index = self.messages.len() - 1;

        // AIの応答を取得
        match self.scheduler.process_user_input(input).await {
            Ok(response) => {
                // AIの応答をクリーンアップ
                let cleaned_response = self.clean_response(&response);
                
                // 処理中メッセージを実際の応答に置き換え
                if let Some(msg) = self.messages.get_mut(processing_msg_index) {
                    msg.content = if cleaned_response.is_empty() {
                        "✅ 処理が完了しました。".to_string()
                    } else {
                        cleaned_response
                    };
                    msg.timestamp = chrono::Local::now();
                }
            }
            Err(e) => {
                // 処理中メッセージをエラーメッセージに置き換え
                if let Some(msg) = self.messages.get_mut(processing_msg_index) {
                    msg.content = format!("❌ エラーが発生しました:\n{}\n\n💡 別の方法で試してみてください。", e);
                    msg.timestamp = chrono::Local::now();
                }
            }
        }

        self.is_processing = false;
        // メッセージ更新後に最下部を表示
        self.update_scroll_to_bottom();
        Ok(())
    }

    /// スクロールを最下部に移動（選択状態をクリア）
    fn update_scroll_to_bottom(&mut self) {
        // 自動スクロール時は選択状態をクリアして背景色の変更を避ける
        self.scroll_state.select(None);
    }

    /// メッセージ表示を強制的に更新（条件付きで最下部にスクロール）
    fn force_redraw(&mut self) {
        // ユーザーが手動でスクロールしていない場合のみ最下部に移動
        let should_auto_scroll = self.scroll_state.selected().is_none() || 
            self.scroll_state.selected().map_or(true, |selected| {
                selected >= self.messages.len().saturating_sub(2)
            });
        
        if should_auto_scroll {
            // 自動スクロール時は選択状態をクリア
            self.scroll_state.select(None);
        }
    }

    /// AIの応答をクリーンアップする
    fn clean_response(&self, response: &str) -> String {
        let mut cleaned = response.to_string();
        
        // "LLM Response:" で始まるデバッグ情報を除去
        if let Some(pos) = cleaned.find("LLM Response:") {
            if let Some(newline_pos) = cleaned[pos..].find('\n') {
                let after_debug = &cleaned[pos + newline_pos + 1..];
                if !after_debug.trim().is_empty() {
                    cleaned = after_debug.trim().to_string();
                } else {
                    cleaned = cleaned[..pos].trim().to_string();
                }
            } else {
                cleaned = cleaned[..pos].trim().to_string();
            }
        }
        
        // JSONライクなデバッグ情報を除去
        if cleaned.starts_with('{') && cleaned.contains("\"action\"") {
            if let Some(pos) = cleaned.find("}\n") {
                let after_json = &cleaned[pos + 2..];
                if !after_json.trim().is_empty() {
                    cleaned = after_json.trim().to_string();
                }
            }
        }
        
        // その他のデバッグパターンを除去
        let debug_patterns = [
            "DEBUG:",
            "Info:",
            "Warning:",
            "Trace:",
            "Error:",
        ];
        
        for pattern in &debug_patterns {
            while let Some(pos) = cleaned.find(pattern) {
                if let Some(newline_pos) = cleaned[pos..].find('\n') {
                    let before = &cleaned[..pos];
                    let after = &cleaned[pos + newline_pos + 1..];
                    cleaned = format!("{}{}", before, after);
                } else {
                    cleaned = cleaned[..pos].trim().to_string();
                    break;
                }
            }
        }
        
        // 余分な空白行を除去（ただし必要な改行は保持）
        let lines: Vec<&str> = cleaned.lines().collect();
        let mut filtered_lines = Vec::new();
        let mut consecutive_empty = 0;
        
        for line in lines {
            if line.trim().is_empty() {
                consecutive_empty += 1;
                if consecutive_empty <= 1 {
                    filtered_lines.push(line);
                }
            } else {
                consecutive_empty = 0;
                filtered_lines.push(line);
            }
        }
        
        cleaned = filtered_lines.join("\n").trim().to_string();
        
        // 空の場合はデフォルトメッセージ
        if cleaned.is_empty() {
            "✅ 処理が完了しました。".to_string()
        } else {
            // 応答の品質を向上
            self.enhance_response_formatting(&cleaned)
        }
    }

    /// 応答のフォーマットを改善する
    fn enhance_response_formatting(&self, response: &str) -> String {
        let mut enhanced = response.to_string();
        
        // 重要な情報にアイコンを追加（より控えめに）
        enhanced = enhanced
            .replace("予定を追加", "📅 予定を追加")
            .replace("予定を削除", "🗑️ 予定を削除")
            .replace("予定を変更", "✏️ 予定を変更")
            .replace("空き時間", "🕐 空き時間")
            .replace("同期", "🔄 同期")
            .replace("完了", "✅ 完了")
            .replace("失敗", "❌ 失敗")
            .replace("エラー", "⚠️ エラー");
        
        // リストの改善（より控えめに）
        enhanced = enhanced
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("- ") {
                    format!("• {}", &trimmed[2..])
                } else if trimmed.starts_with("* ") {
                    format!("• {}", &trimmed[2..])
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        enhanced
    }

    fn render_messages_with_state(&self, f: &mut Frame, area: Rect, scroll_state: &mut ListState) {
        // 安全な幅計算（最小幅を確保）
        let available_width = area.width.saturating_sub(4).max(10); // ボーダー2 + マージン2、最低10文字確保
        
        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .enumerate()
            .map(|(_index, m)| {
                let timestamp = m.timestamp.format("%H:%M:%S");
                let (prefix, header_style, content_style) = match m.role {
                    MessageRole::User => (
                        "👤 あなた",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::White)
                    ),
                    MessageRole::Assistant => (
                        "🤖 AIアシスタント",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::LightGreen)
                    ),
                    MessageRole::System => (
                        "ℹ️  システム",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                        Style::default().fg(Color::LightYellow)
                    ),
                };

                let header = format!("[{}] {}", timestamp, prefix);
                
                // メッセージ内容の処理
                let processed_content = match m.role {
                    MessageRole::Assistant => {
                        self.enhance_response_formatting(&m.content)
                    }
                    _ => m.content.clone(),
                };
                
                // 安全な幅でコンテンツを折り返し
                let content_width = available_width.saturating_sub(4).max(6) as usize; // インデント分を引く、最低6文字確保
                let wrapped_content = self.wrap_message_content(&processed_content, content_width);
                
                // テキスト構築
                let mut lines = Vec::new();
                
                // ヘッダー行
                let header_line = if header.len() > available_width as usize {
                    self.truncate_line(&header, available_width.saturating_sub(3) as usize) + "..."
                } else {
                    header
                };
                lines.push(Line::from(vec![Span::styled(header_line, header_style)]));
                lines.push(Line::from(""));
                
                // コンテンツ行
                for line in wrapped_content.lines() {
                    if line.trim().is_empty() {
                        lines.push(Line::from(""));
                    } else {
                        let indented_line = format!("  {}", line);
                        let safe_line = if indented_line.len() > available_width as usize {
                            self.truncate_line(&indented_line, available_width.saturating_sub(3) as usize) + "..."
                        } else {
                            indented_line
                        };
                        lines.push(Line::from(vec![Span::styled(safe_line, content_style)]));
                    }
                }
                
                lines.push(Line::from(""));
                ListItem::new(Text::from(lines))
            })
            .collect();

        let title = if self.is_processing {
            "💬 Schedule AI Chat - 🔄 処理中..."
        } else {
            "💬 Schedule AI Chat - ✅ 準備完了"
        };

        let messages_list = List::new(messages)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_alignment(Alignment::Left)
                    .border_style(if self.is_processing {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Blue)
                    }),
            )
            .highlight_style(Style::default().bg(Color::Reset))
            .highlight_symbol("");

        f.render_stateful_widget(messages_list, area, scroll_state);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let title = if self.is_processing {
            "⏳ AIが処理中です... しばらくお待ちください"
        } else {
            "✏️ メッセージを入力 (Enter: 送信 | Ctrl+H: ヘルプ | Esc: 終了)"
        };

        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(if self.is_processing {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK)
            } else {
                Style::default().fg(Color::Green)
            });

        let input_style = if self.is_processing {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        let input_text = if self.is_processing {
            "処理中です..."
        } else {
            &self.input
        };

        // 入力テキストが長すぎる場合の安全な処理
        let display_text = if !self.is_processing {
            let max_input_width = area.width.saturating_sub(4) as usize; // ボーダー分を引く
            if input_text.len() > max_input_width {
                // 長すぎる場合は末尾から表示
                let start_pos = input_text.len().saturating_sub(max_input_width);
                &input_text[start_pos..]
            } else {
                input_text
            }
        } else {
            input_text
        };

        let input_paragraph = Paragraph::new(display_text)
            .style(input_style)
            .block(input_block)
            .wrap(Wrap { trim: true });

        f.render_widget(input_paragraph, area);

        // カーソル表示（処理中でない場合のみ）
        if !self.is_processing && !self.show_help {
            // カーソル位置を安全に計算
            let display_cursor_pos = if self.input.is_empty() {
                0
            } else {
                let cursor_byte_pos = self.char_count_to_byte_index(self.cursor_position);
                let text_before_cursor = &self.input[..cursor_byte_pos];
                self.calculate_display_width(text_before_cursor).min(area.width.saturating_sub(2) as usize)
            };
            
            f.set_cursor(
                (area.x + display_cursor_pos as u16 + 1).min(area.x + area.width.saturating_sub(1)),
                area.y + 1,
            );
        }
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let (status_text, status_style) = if self.is_processing {
            (
                "🔄 AIが考え中です... お待ちください",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK)
            )
        } else {
            (
                "✅ 準備完了 | ↑↓: スクロール | Ctrl+H: ヘルプ | Ctrl+C/Esc: 終了 | メッセージを入力してEnterで送信",
                Style::default().fg(Color::Gray)
            )
        };

        let status = Paragraph::new(status_text)
            .style(status_style)
            .alignment(Alignment::Center);

        f.render_widget(status, area);
    }

    fn render_help(&self, f: &mut Frame) {
        let area = centered_rect(70, 80, f.size());
        
        f.render_widget(Clear, area);
        
        let help_text = Text::from(vec![
            Line::from(vec![
                Span::styled("📖 Schedule AI Assistant - Help", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("⌨️  Keyboard Shortcuts:", Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))
            ]),
            Line::from("  Enter      - Send message to AI"),
            Line::from("  ↑/↓        - Scroll through messages"),
            Line::from("  Ctrl+H     - Toggle this help dialog"),
            Line::from("  Ctrl+C/Esc - Quit application"),
            Line::from("  ←/→        - Move cursor in input field"),
            Line::from("  Backspace  - Delete character"),
            Line::from(""),
            Line::from(vec![
                Span::styled("💡 Example Commands:", Style::default().fg(Color::Green).add_modifier(Modifier::UNDERLINED))
            ]),
            Line::from("  • '明日の3時に会議を追加して'"),
            Line::from("  • '来週の予定を教えて'"),
            Line::from("  • '空いている時間はいつ？'"),
            Line::from("  • 'ランチミーティングをキャンセル'"),
            Line::from("  • '予定を最適化して'"),
            Line::from("  • 'Google Calendarと同期して'"),
            Line::from(""),
            Line::from(vec![
                Span::styled("🎯 Features:", Style::default().fg(Color::Magenta).add_modifier(Modifier::UNDERLINED))
            ]),
            Line::from("  • Natural language schedule management"),
            Line::from("  • Google Calendar integration"),
            Line::from("  • Intelligent schedule optimization"),
            Line::from("  • Real-time AI assistance"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Press Esc to close this help.", Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC))
            ]),
        ]);

        let help_paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Help & Usage Guide ")
                    .title_alignment(Alignment::Center)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(help_paragraph, area);
    }
}

// ヘルプダイアログを中央に配置するためのヘルパー関数
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
