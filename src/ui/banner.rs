use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::str::FromStr;
use crate::managers::config::ConfigManager;

pub struct BannerBox {}

impl BannerBox {
    pub fn new() -> Self {
        Self {}
    }

    pub fn draw(&self, f: &mut ratatui::Frame, area: Rect, config: &ConfigManager) {
        let color = Color::from_str(&config.current_config.banner_color).unwrap_or(Color::Yellow);
        let title = Paragraph::new("Ish - Intelli-Shell")
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            
        f.render_widget(title, area);
    }
}
