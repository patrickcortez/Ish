use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct BannerBox {}

impl BannerBox {
    pub fn new() -> Self {
        Self {}
    }

    pub fn draw(&self, f: &mut ratatui::Frame, area: Rect) {
        let title = Paragraph::new("Ish - Intelli-Shell")
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            
        f.render_widget(title, area);
    }
}
