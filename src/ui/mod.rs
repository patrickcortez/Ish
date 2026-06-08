pub mod input;
pub mod output;
pub mod suggestions;
pub mod banner;

use std::io;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    Terminal,
    widgets::{Block, Borders, Paragraph},
    text::{Line, Span},
    style::{Color, Style, Modifier},
};

use crate::managers::config::ConfigManager;

pub struct App {
    pub should_quit: bool,
    pub input: input::InputBox,
    pub output: output::OutputBox,
    pub banner: banner::BannerBox,
    pub suggestions: suggestions::SuggestionsBox,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            input: input::InputBox::new(),
            output: output::OutputBox::new(),
            banner: banner::BannerBox::new(),
            suggestions: suggestions::SuggestionsBox::new(),
        }
    }

    pub fn run(&mut self, config: &ConfigManager) -> anyhow::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the main loop
        let res = self.run_app(&mut terminal, config);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
        )?;
        terminal.show_cursor()?;

        res
    }

    fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, config: &ConfigManager) -> anyhow::Result<()> {
        while !self.should_quit {
            terminal.draw(|f| self.draw(f, config))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Quick quit for development
                    if key.code == KeyCode::Esc {
                        self.should_quit = true;
                        continue;
                    }

                    // Delegate events
                    if self.suggestions.is_active() {
                        self.suggestions.handle_key(key);
                    } else {
                        self.input.handle_key(key);
                    }
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, f: &mut ratatui::Frame, _config: &ConfigManager) {
        let size = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3), // Banner
                    Constraint::Min(1),    // Output
                    Constraint::Length(3), // Input
                ]
                .as_ref(),
            )
            .split(size);

        self.banner.draw(f, chunks[0]);
        self.output.draw(f, chunks[1]);
        
        // Overlap suggestions box above the input box if active
        if self.suggestions.is_active() {
            let suggest_rect = Rect {
                x: chunks[2].x,
                y: chunks[2].y.saturating_sub(6), // Display above input box
                width: chunks[2].width,
                height: 6,
            };
            self.suggestions.draw(f, suggest_rect);
        }

        self.input.draw(f, chunks[2]);
    }
}
