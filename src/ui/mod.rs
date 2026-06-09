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
                                } else if cmd_trim == "clear" || cmd_trim == "cls" {
                                    self.output.clear();
                                    continue;
                                } else if cmd_trim.starts_with("cd ") {
                                    let path = cmd_trim[3..].trim();
                                    if let Err(e) = std::env::set_current_dir(path) {
                                        self.output.append(format!("cd error: {}", e));
                                    }
                                    continue;
                                } else if cmd_trim.starts_with(":Color ") {
                                    let args: Vec<&str> = cmd_trim[7..].trim().split_whitespace().collect();
                                    if args.len() == 2 {
                                        let target = args[0];
                                        let color_str = args[1];
                                        
                                        // Validate color before saving
                                        use ratatui::style::Color;
                                        use std::str::FromStr;
                                        
                                        if Color::from_str(color_str).is_ok() {
                                            match target.to_lowercase().as_str() {
                                                "--inputbox" => self.config.current_config.input_color = color_str.to_string(),
                                                "--output" => self.config.current_config.output_color = color_str.to_string(),
                                                "--banner" => self.config.current_config.banner_color = color_str.to_string(),
                                                _ => {
                                                    self.output.append(format!("Unknown color target: {}. Use --inputbox, --output, or --banner.", target));
                                                    continue;
                                                }
                                            }
                                            if let Err(e) = self.config.save() {
                                                self.output.append(format!("Failed to save config: {}", e));
                                            } else {
                                                self.output.append(format!("Color for {} set to {}", target, color_str));
                                            }
                                        } else {
                                            self.output.append(format!("Invalid color: {}", color_str));
                                        }
                                    } else {
                                        self.output.append("Usage: :Color <Target> <Color>".to_string());
                                    }
                                    continue;
                                } else if cmd_trim.starts_with(":Toggle ") {
                                    let args: Vec<&str> = cmd_trim[8..].trim().split_whitespace().collect();
                                    if args.len() == 2 {
                                        let flag = args[0];
                                        let value = args[1] == "true";
                                        match flag.to_lowercase().as_str() {
                                            "--autocd" => {
                                                self.config.current_config.autocd = value;
                                                self.output.append(format!("Autocd set to: {}", value));
                                            }
                                            "--suggestions" => {
                                                self.config.current_config.suggestions = value;
                                                self.output.append(format!("Suggestions set to: {}", value));
                                            }
                                            _ => {
                                                self.output.append(format!("Unknown flag: {}. Use --autocd or --suggestions.", flag));
                                            }
                                        }
                                        let _ = self.config.save();
                                    } else {
                                        self.output.append("Usage: :Toggle <Flag> <true/false>".to_string());
                                    }
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
                                                    let mut jobs = std::mem::replace(&mut self.jobs, JobController::new());
                                                    let mut executor = Executor::new(vec![]);
                                                    match executor.execute(&ast, &mut jobs, &mut |child, merge_err| {
                                                        self.pump_io(child, terminal, config, merge_err)
                                                    }) {
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
                                                    self.jobs = jobs;
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

        self.banner.draw(f, chunks[0], _config);
        self.output.draw(f, chunks[1], _config);
        
        // Overlap suggestions box above the input box if active
        if self.suggestions.is_active() && _config.current_config.suggestions {
            let suggest_rect = Rect {
                x: chunks[2].x,
                y: chunks[2].y.saturating_sub(6), // Display above input box
                width: chunks[2].width,
                height: 6,
            };
            self.suggestions.draw(f, suggest_rect);
        }

        self.input.draw(f, chunks[2], _config);
    }

    fn pump_io(
        &mut self,
        child: &mut std::process::Child,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        config: &ConfigManager,
        merge_err: bool,
    ) -> Result<String, crate::error::IshError> {
        use std::sync::mpsc;
        use std::thread;
        use std::io::{Read, Write};

        let (tx, rx) = mpsc::channel();
        
        if let Some(mut stdout) = child.stdout.take() {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut buf = [0; 1024];
                while let Ok(n) = stdout.read(&mut buf) {
                    if n == 0 { break; }
                    let _ = tx.send((false, buf[..n].to_vec()));
                }
            });
        }
        
        if let Some(mut stderr) = child.stderr.take() {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut buf = [0; 1024];
                while let Ok(n) = stderr.read(&mut buf) {
                    if n == 0 { break; }
                    let _ = tx.send((true, buf[..n].to_vec()));
                }
            });
        }

        let mut alt_screen_active = false;
        let mut full_output = String::new();
        let mut full_err = String::new();
        let mut stdin_thread_spawned = false;

        loop {
            while let Ok((is_err, bytes)) = rx.try_recv() {
                let chunk = String::from_utf8_lossy(&bytes);
                
                if !is_err {
                    if chunk.contains("\x1b[?1049h") || chunk.contains("\x1b[?1047h") || chunk.contains("\x1b[?47h") {
                        if !alt_screen_active {
                            alt_screen_active = true;
                            let _ = disable_raw_mode();
                            let _ = execute!(io::stdout(), LeaveAlternateScreen);
                        }
                    }
                    
                    if alt_screen_active {
                        let _ = io::stdout().write_all(&bytes);
                        let _ = io::stdout().flush();
                    } else {
                        full_output.push_str(&chunk);
                        
                        // Parse lines for UI rendering
                        let mut parts: Vec<&str> = chunk.split('\n').collect();
                        if parts.len() > 1 {
                            let first = parts.remove(0);
                            let current_partial = self.output.partial_line.take().unwrap_or_default();
                            self.output.append(format!("{}{}", current_partial, first));
                            
                            let last = parts.pop().unwrap();
                            for p in parts {
                                self.output.append(p.to_string());
                            }
                            self.output.partial_line = Some(last.to_string());
                        } else {
                            let current_partial = self.output.partial_line.take().unwrap_or_default();
                            self.output.partial_line = Some(format!("{}{}", current_partial, parts[0]));
                        }
                    }
                } else {
                    if alt_screen_active {
                        let _ = io::stderr().write_all(&bytes);
                        let _ = io::stderr().flush();
                    } else {
                        full_err.push_str(&chunk);
                    }
                }
            }

            if alt_screen_active {
                if !stdin_thread_spawned {
                    stdin_thread_spawned = true;
                    if let Some(mut child_stdin) = child.stdin.take() {
                        thread::spawn(move || {
                            let mut raw_stdin = std::io::stdin();
                            let mut buf = [0; 128];
                            while let Ok(n) = raw_stdin.read(&mut buf) {
                                if n == 0 { break; }
                                if child_stdin.write_all(&buf[..n]).is_err() { break; }
                                let _ = child_stdin.flush();
                            }
                        });
                    }
                }
            } else {
                let _ = terminal.draw(|f| self.draw(f, config));
                
                if event::poll(std::time::Duration::from_millis(20)).unwrap_or(false) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.code == KeyCode::Enter {
                            let line = format!("{}\n", self.input.input);
                            if let Some(stdin) = child.stdin.as_mut() {
                                let _ = stdin.write_all(line.as_bytes());
                                let _ = stdin.flush();
                            }
                            self.output.append(self.input.input.clone());
                            self.input.input.clear();
                            self.input.cursor_pos = 0;
                        } else if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            let _ = child.kill();
                        } else {
                            self.input.handle_key(key);
                        }
                    }
                }
            }

            if let Ok(Some(_)) = child.try_wait() {
                // Drain
                while let Ok((is_err, bytes)) = rx.try_recv() {
                    let chunk = String::from_utf8_lossy(&bytes);
                    if !is_err {
                        if alt_screen_active {
                            let _ = io::stdout().write_all(&bytes);
                            let _ = io::stdout().flush();
                        } else {
                            full_output.push_str(&chunk);
                        }
                    } else {
                        if alt_screen_active {
                            let _ = io::stderr().write_all(&bytes);
                            let _ = io::stderr().flush();
                        } else {
                            full_err.push_str(&chunk);
                        }
                    }
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        self.output.partial_line = None;

        if alt_screen_active {
            let _ = enable_raw_mode();
            let _ = execute!(io::stdout(), EnterAlternateScreen);
            let _ = terminal.clear();
            return Ok(String::new());
        }

        if merge_err {
            full_output.push_str(&full_err);
        } else if !full_err.is_empty() {
            full_output.push_str(&full_err);
        }

        // Return empty so the caller (executor loop) doesn't append it again
        Ok(String::new())
    }
}
