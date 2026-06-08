use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct InputBox {
    pub input: String,
}

impl InputBox {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
    }

    pub fn set_input(&mut self, text: String) {
        self.input = text;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match key.code {
            KeyCode::Char(c) => {
                self.input.push(c);
                None
            }
            KeyCode::Backspace => {
                self.input.pop();
                None
            }
            KeyCode::Enter => {
                let val = self.input.clone();
                self.input.clear();
                Some(val)
            }
            _ => None
        }
    }

    pub fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let input_block = Paragraph::new(format!("> {}", self.input))
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL).title(" Input "));
        f.render_widget(input_block, area);
    }
}
