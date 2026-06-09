use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::str::FromStr;
use crate::managers::config::ConfigManager;

pub struct InputBox {
    pub input: String,
    pub cursor_pos: usize,
}

impl InputBox {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
    }

    pub fn set_input(&mut self, text: String) {
        self.input = text;
        self.cursor_pos = self.input.chars().count();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match key.code {
            KeyCode::Char(c) => {
                let char_idx = self.input.char_indices().nth(self.cursor_pos).map(|(i, _)| i).unwrap_or(self.input.len());
                self.input.insert(char_idx, c);
                self.cursor_pos += 1;
                None
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let char_idx = self.input.char_indices().nth(self.cursor_pos - 1).map(|(i, _)| i).unwrap_or(self.input.len());
                    self.input.remove(char_idx);
                    self.cursor_pos -= 1;
                }
                None
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                None
            }
            KeyCode::Right => {
                if self.cursor_pos < self.input.chars().count() {
                    self.cursor_pos += 1;
                }
                None
            }
            KeyCode::Enter => {
                let val = self.input.clone();
                self.input.clear();
                self.cursor_pos = 0;
                Some(val)
            }
            _ => None
        }
    }

    pub fn draw(&self, f: &mut ratatui::Frame, area: Rect, config: &ConfigManager) {
        let color = Color::from_str(&config.current_config.input_color).unwrap_or(Color::Cyan);
        let input_block = Paragraph::new(format!("> {}", self.input))
            .style(Style::default().fg(color))
            .block(Block::default().borders(Borders::ALL).title(" Input "));
        f.render_widget(input_block, area);

        let cursor_x = (area.x + 3 + self.cursor_pos as u16).min(area.x + area.width.saturating_sub(2));
        f.set_cursor_position((cursor_x, area.y + 1));
    }
}
