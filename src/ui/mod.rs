use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    queue,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::env;
use std::io::{self, Write};

pub mod banner;

use crate::managers::config::ConfigManager;
use crate::managers::history::HistoryManager;
use crate::managers::job_controller::JobController;
use crate::managers::suggestions::SuggestionManager;
use crate::core::tokenizer::Tokenizer;
use crate::core::parser::Parser;
use crate::core::linter::Linter;
use crate::core::executor::Executor;

pub struct App {
    pub config: ConfigManager,
    pub history: HistoryManager,
    pub jobs: JobController,
    pub sugg_engine: SuggestionManager,
}

impl App {
    pub fn new() -> Self {
        Self {
            config: ConfigManager::new().unwrap(),
            history: HistoryManager::new(),
            jobs: JobController::new(),
            sugg_engine: SuggestionManager::new(),
        }
    }

    fn visible_width(s: &str) -> u16 {
        let mut width = 0;
        let mut in_ansi = false;
        for c in s.chars() {
            if c == '\x1b' {
                in_ansi = true;
            } else if in_ansi {
                if c.is_ascii_alphabetic() {
                    in_ansi = false;
                }
            } else {
                width += 1;
            }
        }
        width
    }

    pub fn run(&mut self, _config: &ConfigManager) -> anyhow::Result<()> {
        banner::print_banner();
        let mut stdout = io::stdout();

        loop {
            let os_icon = if std::env::consts::OS == "windows" { "\u{e70f} " } else if std::env::consts::OS == "macos" { "\u{f179} " } else { "\u{f17c} " };
            let user = env::var("USER").or_else(|_| env::var("USERNAME")).unwrap_or_else(|_| "user".to_string());
            let host = env::var("COMPUTERNAME").or_else(|_| env::var("HOSTNAME")).unwrap_or_else(|_| "host".to_string());
            let pwd = env::current_dir().unwrap_or_default().display().to_string();
            
            let bg_blue = "\x1b[48;2;0;120;215m";
            let fg_white = "\x1b[38;5;255m";
            
            let bg_gray = "\x1b[48;2;60;60;60m";
            let fg_blue = "\x1b[38;2;0;120;215m"; 
            
            let bg_darkblue = "\x1b[48;2;30;30;30m";
            let fg_gray = "\x1b[38;2;60;60;60m"; 
            
            let reset_bg = "\x1b[49m";
            let fg_darkblue = "\x1b[38;2;30;30;30m"; 
            let reset = "\x1b[0m";

            let prompt = format!(
                "{}{} {} {}{}\u{e0b0} {}{}{}@{} {}{}\u{e0b0} {}{}\u{f07c} {} {}{}\u{e0b0}{} ",
                bg_blue, fg_white, os_icon,
                bg_gray, fg_blue,
                bg_gray, fg_white, user, host,
                bg_darkblue, fg_gray,
                bg_darkblue, fg_white, pwd,
                reset_bg, fg_darkblue,
                reset
            );

            let mut buffer = String::new();
            let mut cursor_pos = 0;
            let mut history_index = self.history.get_all().len();
            let prompt_width = Self::visible_width(&prompt);

            let draw_line = |out: &mut std::io::Stdout, buf: &str, pos: usize, sugg: &SuggestionManager, hist: &[String]| -> anyhow::Result<()> {
                let total_cursor_col = prompt_width + pos as u16;
                queue!(
                    out,
                    cursor::Hide,
                    cursor::MoveToColumn(0),
                    Clear(ClearType::UntilNewLine),
                    Print(&prompt),
                    Print(&sugg.highlight(buf))
                )?;

                if pos == buf.len() {
                    if let Some(hint) = sugg.get_hint(buf, hist) {
                        queue!(out, Print(format!("\x1b[90m{}\x1b[0m", hint)))?;
                    }
                }

                queue!(out, cursor::MoveToColumn(total_cursor_col), cursor::Show)?;
                out.flush()?;
                Ok(())
            };

            draw_line(&mut stdout, &buffer, cursor_pos, &self.sugg_engine, self.history.get_all())?;
            enable_raw_mode()?;

            let mut break_outer = false;
            let mut cmd = String::new();

            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                buffer.clear();
                                println!("\r\n^C");
                                break;
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if buffer.is_empty() {
                                    println!("\r\nexit");
                                    break_outer = true;
                                    break;
                                }
                            }
                            KeyCode::Char(c) => {
                                buffer.insert(cursor_pos, c);
                                cursor_pos += 1;
                            }
                            KeyCode::Backspace => {
                                if cursor_pos > 0 {
                                    buffer.remove(cursor_pos - 1);
                                    cursor_pos -= 1;
                                }
                            }
                            KeyCode::Delete => {
                                if cursor_pos < buffer.len() {
                                    buffer.remove(cursor_pos);
                                }
                            }
                            KeyCode::Left => {
                                if cursor_pos > 0 {
                                    cursor_pos -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if cursor_pos == buffer.len() {
                                    if let Some(hint) = self.sugg_engine.get_hint(&buffer, self.history.get_all()) {
                                        buffer.push_str(&hint);
                                        cursor_pos = buffer.len();
                                    }
                                } else if cursor_pos < buffer.len() {
                                    cursor_pos += 1;
                                }
                            }
                            KeyCode::Up => {
                                if history_index > 0 {
                                    history_index -= 1;
                                    if let Some(h) = self.history.get_all().get(history_index) {
                                        buffer = h.clone();
                                        cursor_pos = buffer.len();
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if history_index < self.history.get_all().len() {
                                    history_index += 1;
                                    if history_index == self.history.get_all().len() {
                                        buffer.clear();
                                        cursor_pos = 0;
                                    } else if let Some(h) = self.history.get_all().get(history_index) {
                                        buffer = h.clone();
                                        cursor_pos = buffer.len();
                                    }
                                }
                            }
                            KeyCode::End => {
                                cursor_pos = buffer.len();
                            }
                            KeyCode::Home => {
                                cursor_pos = 0;
                            }
                            KeyCode::Enter => {
                                println!("\r");
                                cmd = buffer.clone();
                                break;
                            }
                            KeyCode::Tab => {
                                if let Some(hint) = self.sugg_engine.get_hint(&buffer, self.history.get_all()) {
                                    buffer.push_str(&hint);
                                    cursor_pos = buffer.len();
                                }
                            }
                            _ => {}
                        }
                        
                        draw_line(&mut stdout, &buffer, cursor_pos, &self.sugg_engine, self.history.get_all())?;
                    }
                }
            }

            disable_raw_mode()?;

            if break_outer {
                break;
            }

            let cmd = cmd.trim();
            if cmd.is_empty() {
                continue;
            }

            self.history.add(cmd.to_string());

            if cmd == "exit" || cmd == "quit" {
                break;
            } else if self.config.current_config.autocd && std::path::Path::new(cmd).is_dir() {
                if let Err(e) = std::env::set_current_dir(cmd) {
                    eprintln!("cd error: {}", e);
                }
                continue;
            } else if cmd.starts_with("cd ") {
                let path = cmd[3..].trim();
                if let Err(e) = std::env::set_current_dir(path) {
                    eprintln!("cd error: {}", e);
                }
                continue;
            } else if cmd.starts_with(":Toggle ") {
                let args: Vec<&str> = cmd[8..].trim().split_whitespace().collect();
                if args.len() == 2 {
                    let flag = args[0];
                    let value = args[1] == "true";
                    match flag.to_lowercase().as_str() {
                        "--autocd" => {
                            self.config.current_config.autocd = value;
                            println!("Autocd set to: {}", value);
                        }
                        "--suggestions" => {
                            self.config.current_config.suggestions = value;
                            println!("Suggestions set to: {}", value);
                        }
                        _ => {
                            println!("Unknown flag: {}. Use --autocd or --suggestions.", flag);
                        }
                    }
                    let _ = self.config.save();
                } else {
                    println!("Usage: :Toggle <Flag> <true/false>");
                }
                continue;
            } else if cmd.starts_with(":Editor ") {
                let editor = cmd[8..].trim();
                self.config.current_config.editor = editor.to_string();
                println!("Editor set to: {}", editor);
                continue;
            } else if cmd == "jobs" {
                let out = self.jobs.list_jobs();
                print!("{}", out);
                continue;
            } else if cmd.starts_with("kill ") {
                if let Ok(id) = cmd[5..].trim().parse::<u32>() {
                    match self.jobs.kill_job(id) {
                        Ok(msg) => print!("{}", msg),
                        Err(e) => println!("{}", e),
                    }
                } else {
                    println!("Usage: kill <job_id>");
                }
                continue;
            } else if cmd.starts_with("fg ") {
                if let Ok(id) = cmd[3..].trim().parse::<u32>() {
                    match self.jobs.wait_job(id) {
                        Ok(msg) => print!("{}", msg),
                        Err(e) => println!("{}", e),
                    }
                } else {
                    println!("Usage: fg <job_id>");
                }
                continue;
            }

            let mut tokenizer = Tokenizer::new(cmd);
            match tokenizer.tokenize() {
                Ok(tokens) => {
                    let mut parser = Parser::new(tokens);
                    match parser.parse() {
                        Ok(ast) => {
                            let linter = Linter::new();
                            if let Err(e) = linter.lint(&ast) {
                                eprintln!("Lint Error: {}", e);
                            } else {
                                let mut jobs = std::mem::replace(&mut self.jobs, JobController::new());
                                let mut executor = Executor::new(vec![]);
                                if let Err(e) = executor.execute(&ast, &mut jobs) {
                                    eprintln!("Execution Error: {}", e);
                                }
                                self.jobs = jobs;
                            }
                        }
                        Err(e) => eprintln!("Parse Error: {}", e),
                    }
                }
                Err(e) => eprintln!("Tokenize Error: {}", e),
            }
        }
        
        Ok(())
    }
}
