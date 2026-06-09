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
};

use crate::managers::config::ConfigManager;
use crate::managers::history::HistoryManager;
use crate::managers::job_controller::JobController;
use crate::managers::suggestions::SuggestionManager;
use crate::core::tokenizer::Tokenizer;
use crate::core::parser::Parser;
use crate::core::linter::Linter;
use crate::core::executor::Executor;

pub struct App {
    pub should_quit: bool,
    pub input: input::InputBox,
    pub output: output::OutputBox,
    pub banner: banner::BannerBox,
    pub suggestions: suggestions::SuggestionsBox,
    pub config: ConfigManager,
    pub history: HistoryManager,
    pub jobs: JobController,
    pub sugg_engine: SuggestionManager,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            input: input::InputBox::new(),
            output: output::OutputBox::new(),
            banner: banner::BannerBox::new(),
            suggestions: suggestions::SuggestionsBox::new(),
            config: ConfigManager::new().unwrap(),
            history: HistoryManager::new(),
            jobs: JobController::new(),
            sugg_engine: SuggestionManager::new(),
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
                    let mut is_suggestion_nav = false;
                    if self.suggestions.is_active() {
                        match key.code {
                            KeyCode::Down | KeyCode::Up | KeyCode::Right | KeyCode::Tab | KeyCode::Esc => {
                                is_suggestion_nav = true;
                            }
                            _ => {}
                        }
                    }

                    if is_suggestion_nav {
                        if let Some(accepted) = self.suggestions.handle_key(key) {
                            let mut tokens: Vec<&str> = self.input.input.split_whitespace().collect();
                            if !self.input.input.ends_with(char::is_whitespace) && !tokens.is_empty() {
                                tokens.pop();
                            }
                            let mut new_input = tokens.join(" ");
                            if !new_input.is_empty() {
                                new_input.push(' ');
                            }
                            new_input.push_str(&accepted);
                            self.input.set_input(new_input);
                        }
                    } else if key.code == KeyCode::PageUp || (key.code == KeyCode::Up && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)) {
                        self.output.scroll_up();
                    } else if key.code == KeyCode::PageDown || (key.code == KeyCode::Down && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)) {
                        self.output.scroll_down();
                    } else if key.code == KeyCode::Up {
                        if let Some(cmd) = self.history.get_previous() {
                            self.input.set_input(cmd);
                        }
                    } else if key.code == KeyCode::Down {
                        if let Some(cmd) = self.history.get_next() {
                            self.input.set_input(cmd);
                        }
                    } else {
                        if let Some(cmd) = self.input.handle_key(key) {
                            if !cmd.trim().is_empty() {
                                self.history.add(cmd.clone());
                                self.output.append(format!("> {}", cmd));

                                let cmd_trim = cmd.trim();
                                
                                if cmd_trim == "exit" || cmd_trim == "quit" {
                                    self.should_quit = true;
                                    break;
                                } else if cmd_trim.starts_with("cd ") {
                                    let path = cmd_trim[3..].trim();
                                    if let Err(e) = std::env::set_current_dir(path) {
                                        self.output.append(format!("cd error: {}", e));
                                    }
                                    continue;
                                } else if cmd_trim.starts_with(":Color ") {
                                    let color = cmd_trim[7..].trim();
                                    self.output.append(format!("Color configuration is a WIP: {}", color));
                                    continue;
                                } else if cmd_trim == ":Toggle" {
                                    self.config.current_config.autocd = !self.config.current_config.autocd;
                                    self.output.append(format!("Autocd set to: {}", self.config.current_config.autocd));
                                    continue;
                                } else if cmd_trim.starts_with(":Editor ") {
                                    let editor = cmd_trim[8..].trim();
                                    self.config.current_config.editor = editor.to_string();
                                    self.output.append(format!("Editor set to: {}", editor));
                                    continue;
                                } else if cmd_trim == "jobs" {
                                    let out = self.jobs.list_jobs();
                                    self.output.append(out);
                                    continue;
                                } else if cmd.starts_with("kill ") {
                                    if let Ok(id) = cmd[5..].trim().parse::<u32>() {
                                        match self.jobs.kill_job(id) {
                                            Ok(msg) => self.output.append(msg),
                                            Err(e) => self.output.append(e),
                                        }
                                    } else {
                                        self.output.append("Usage: kill <job_id>".to_string());
                                    }
                                    continue;
                                } else if cmd.starts_with("fg ") {
                                    if let Ok(id) = cmd[3..].trim().parse::<u32>() {
                                        match self.jobs.wait_job(id) {
                                            Ok(msg) => self.output.append(msg),
                                            Err(e) => self.output.append(e),
                                        }
                                    } else {
                                        self.output.append("Usage: fg <job_id>".to_string());
                                    }
                                    continue;
                                }
                                
                                let mut tokenizer = Tokenizer::new(&cmd);
                                match tokenizer.tokenize() {
                                    Ok(tokens) => {
                                        let mut parser = Parser::new(tokens);
                                        match parser.parse() {
                                            Ok(ast) => {
                                                let linter = Linter::new();
                                                if let Err(e) = linter.lint(&ast) {
                                                    self.output.append(format!("Lint Error: {}", e));
                                                } else {
                                                    let mut executor = Executor::new(vec![]);
                                                    match executor.execute(&ast, &mut self.jobs) {
                                                        Ok((_, out)) => {
                                                            if !out.trim().is_empty() {
                                                                for line in out.lines() {
                                                                    self.output.append(line.to_string());
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            self.output.append(format!("Execution Error: {}", e));
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => self.output.append(format!("Parse Error: {}", e)),
                                        }
                                    }
                                    Err(e) => self.output.append(format!("Tokenize Error: {}", e)),
                                }
                            }
                        } else {
                            // Update suggestions dynamically as typing occurs
                            if matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace) {
                                let suggs = self.sugg_engine.get_suggestions(&self.input.input, self.history.get_all());
                                self.suggestions.set_suggestions(suggs);
                            }
                        }
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
