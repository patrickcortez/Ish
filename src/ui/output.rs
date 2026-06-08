use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    text::Line,
};

pub struct OutputBox {
    pub history: Vec<String>,
}

impl OutputBox {
    pub fn new() -> Self {
        Self {
            history: vec![
                "Welcome to Ish (Intelli-Shell)".to_string(),
                "Type a command...".to_string(),
            ],
        }
    }

    pub fn append(&mut self, line: String) {
        self.history.push(line);
    }

    pub fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let text: Vec<Line> = self.history.iter().map(|s| Line::from(s.clone())).collect();
        let output_block = Paragraph::new(text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title(" Output "));
        f.render_widget(output_block, area);
    }
}
