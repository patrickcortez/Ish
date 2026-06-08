use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    text::Line,
};

pub struct OutputBox {
    pub history: Vec<String>,
    pub scroll: u16,
    pub auto_scroll: bool,
}

impl OutputBox {
    pub fn new() -> Self {
        Self {
            history: vec![
                "Welcome to Ish (Intelli-Shell)".to_string(),
                "Type a command...".to_string(),
            ],
            scroll: 0,
            auto_scroll: true,
        }
    }

    pub fn append(&mut self, line: String) {
        for l in line.lines() {
            self.history.push(l.to_string());
        }
        self.auto_scroll = true;
    }

    pub fn scroll_up(&mut self) {
        self.auto_scroll = false;
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
        self.auto_scroll = false;
    }

    pub fn draw(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let text: Vec<Line> = self.history.iter().map(|s| Line::from(s.clone())).collect();
        let total_lines = text.len() as u16;
        let view_height = area.height.saturating_sub(2);

        let max_scroll = total_lines.saturating_sub(view_height);

        if self.auto_scroll || self.scroll >= max_scroll {
            self.auto_scroll = true;
            self.scroll = max_scroll;
        }

        let title = if self.auto_scroll {
            " Output ".to_string()
        } else {
            " Output (SCROLLING) ".to_string()
        };

        let output_block = Paragraph::new(text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title(title))
            .scroll((self.scroll, 0));
        f.render_widget(output_block, area);
    }
}
