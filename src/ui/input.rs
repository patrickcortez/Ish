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

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                // To be processed by the executor
                self.input.clear();
            }
            _ => {}
        }
    }

    pub fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let input_block = Paragraph::new(format!("> {}", self.input))
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL).title(" Input "));
        f.render_widget(input_block, area);
    }
}
