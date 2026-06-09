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

fn get_git_status() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "-b"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut branch = String::new();
    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    let mut untracked = 0;
    
    for line in stdout.lines() {
        if line.starts_with("##") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let branch_part = parts[1];
                let b: Vec<&str> = branch_part.split("...").collect();
                branch = b[0].to_string();
            }
        } else if line.len() >= 2 {
            let status = &line[0..2];
            if status == "??" {
                untracked += 1;
            } else {
                if status.contains('A') { added += 1; }
                if status.contains('M') { modified += 1; }
                if status.contains('D') { deleted += 1; }
            }
        }
    }
    
    if branch.is_empty() {
        return None;
    }
    
    let mut details = String::new();
    if added > 0 { details.push_str(&format!("+{} ", added)); }
    if modified > 0 { details.push_str(&format!("~{} ", modified)); }
    if deleted > 0 { details.push_str(&format!("-{} ", deleted)); }
    if untracked > 0 { details.push_str(&format!("?{} ", untracked)); }
    
    let details = details.trim();
    if details.is_empty() {
        Some(format!("\u{e0a0} {}", branch))
    } else {
        Some(format!("\u{e0a0} {} [{}]", branch, details))
    }
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
            let home_dir = if cfg!(target_os = "windows") {
                env::var("USERPROFILE").unwrap_or_default()
            } else {
                env::var("HOME").unwrap_or_default()
            };
            
            let mut pwd = env::current_dir().unwrap_or_default().display().to_string();
            if !home_dir.is_empty() && pwd.starts_with(&home_dir) {
                pwd = pwd.replacen(&home_dir, "~", 1);
            }
            
            let bg_blue = "\x1b[48;2;0;120;215m";
            let fg_white = "\x1b[38;5;255m";
            
            let bg_gray = "\x1b[48;2;60;60;60m";
            let fg_blue = "\x1b[38;2;0;120;215m"; 
            
            let bg_darkblue = "\x1b[48;2;30;30;30m";
            let fg_gray = "\x1b[38;2;60;60;60m"; 
            
            let reset_bg = "\x1b[49m";
            let fg_darkblue = "\x1b[38;2;30;30;30m"; 
            let reset = "\x1b[0m";

            let mut prompt = format!(
                "{}{} {} {}{}\u{e0b0} {}{}{}@{} {}{}\u{e0b0} {}{}\u{f07c} {} ",
                bg_blue, fg_white, os_icon,
                bg_gray, fg_blue,
                bg_gray, fg_white, user, host,
                bg_darkblue, fg_gray,
                bg_darkblue, fg_white, pwd
            );

            if let Some(git_status) = get_git_status() {
                let bg_git = "\x1b[48;2;40;100;40m"; // Dark green
                let fg_git = "\x1b[38;5;255m";
                let fg_darkblue_git = "\x1b[38;2;30;30;30m"; 
                let fg_git_bg = "\x1b[38;2;40;100;40m";
                
                prompt.push_str(&format!(
                    "{}{}\u{e0b0} {}{} {} {}{}\u{e0b0}{} ",
                    bg_git, fg_darkblue_git,
                    bg_git, fg_git, git_status,
                    reset_bg, fg_git_bg,
                    reset
                ));
            } else {
                prompt.push_str(&format!(
                    "{}{}\u{e0b0}{} ",
                    reset_bg, fg_darkblue,
                    reset
                ));
            }

            let mut buffer = String::new();
            let mut cursor_pos = 0;
            let mut history_index = self.history.get_all().len();
            let prompt_width = Self::visible_width(&prompt);
            let mut cursor_state = (0, 0); // (current_row_offset, printed_row_offset)

            let draw_line = |out: &mut std::io::Stdout, buf: &str, pos: usize, sugg: &SuggestionManager, hist: &[String], state: (u16, u16)| -> anyhow::Result<(u16, u16)> {
                let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
                let term_width = if term_width == 0 { 80 } else { term_width };
                
                let last_row_offset = state.0;
                if last_row_offset > 0 {
                    queue!(out, cursor::MoveUp(last_row_offset))?;
                }
                
                queue!(
                    out,
                    cursor::Hide,
                    cursor::MoveToColumn(0),
                    Clear(ClearType::FromCursorDown),
                    Print(&prompt),
                    Print(&sugg.highlight(buf))
                )?;

                if pos == buf.len() {
                    if let Some(hint) = sugg.get_hint(buf, hist) {
                        queue!(out, Print(format!("\x1b[90m{}\x1b[0m", hint)))?;
                    }
                }

                let current_total = prompt_width + pos as u16;
                let current_row_offset = current_total / term_width;
                let current_col = current_total % term_width;

                let text_total = prompt_width + buf.len() as u16;
                
                let hint_len = if pos == buf.len() {
                    if let Some(hint) = sugg.get_hint(buf, hist) {
                        Self::visible_width(&hint)
                    } else {
                        0
                    }
                } else {
                    0
                };
                
                let total_printed_len = text_total + hint_len;
                let printed_row_offset = total_printed_len / term_width;
                
                if printed_row_offset > current_row_offset {
                    queue!(out, cursor::MoveUp(printed_row_offset - current_row_offset))?;
                } else if current_row_offset > printed_row_offset {
                    queue!(out, cursor::MoveDown(current_row_offset - printed_row_offset))?;
                }
                
                queue!(out, cursor::MoveToColumn(current_col), cursor::Show)?;
                out.flush()?;
                Ok((current_row_offset, printed_row_offset))
            };

            cursor_state = draw_line(&mut stdout, &buffer, cursor_pos, &self.sugg_engine, self.history.get_all(), cursor_state)?;
            enable_raw_mode()?;

            let mut break_outer = false;
            let mut cmd = String::new();

            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if cursor_state.1 > cursor_state.0 {
                                    let _ = queue!(stdout, cursor::MoveDown(cursor_state.1 - cursor_state.0));
                                    let _ = stdout.flush();
                                }
                                buffer.clear();
                                println!("\r\n^C");
                                break;
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if buffer.is_empty() {
                                    if cursor_state.1 > cursor_state.0 {
                                        let _ = queue!(stdout, cursor::MoveDown(cursor_state.1 - cursor_state.0));
                                        let _ = stdout.flush();
                                    }
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
                                if cursor_state.1 > cursor_state.0 {
                                    let _ = queue!(stdout, cursor::MoveDown(cursor_state.1 - cursor_state.0));
                                    let _ = stdout.flush();
                                }
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
                        
                        cursor_state = draw_line(&mut stdout, &buffer, cursor_pos, &self.sugg_engine, self.history.get_all(), cursor_state)?;
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
            } else if self.config.current_config.autocd && (cmd.starts_with('/') || cmd.starts_with('\\')) {
                let path_str = cmd[1..].trim();
                if path_str.is_empty() {
                    continue;
                }
                let p = std::path::Path::new(path_str);
                if p.is_dir() {
                    if let Err(e) = std::env::set_current_dir(p) {
                        eprintln!("cd error: {}", e);
                    }
                    continue;
                } else {
                    println!("Ish: directory not found: {}", path_str);
                    continue;
                }
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
            } else if cmd == ":history" {
                let all = self.history.get_all();
                for (i, h) in all.iter().enumerate() {
                    println!("{}: {}", i + 1, h);
                }
                continue;
            } else if cmd == ":history clear" {
                self.history.clear();
                println!("History cleared.");
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
                            let mut linter = Linter::new();
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
