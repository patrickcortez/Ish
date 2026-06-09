use rustyline::error::ReadlineError;
use rustyline::{Config as RlConfig, Editor};
use rustyline::history::DefaultHistory;
use std::env;

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

    pub fn run(&mut self, _config: &ConfigManager) -> anyhow::Result<()> {
        banner::print_banner();

        let rl_config = RlConfig::builder()
            .history_ignore_space(true)
            .completion_type(rustyline::CompletionType::List)
            .auto_add_history(true)
            .build();
            
        let mut rl: Editor<SuggestionManager, DefaultHistory> = Editor::with_config(rl_config)?;
        
        // Use our suggestion manager as the rustyline helper for ghost text
        rl.set_helper(Some(SuggestionManager::new()));

        let history_file = env::current_dir()
            .unwrap_or_default()
            .join(".ish_history");

        if rl.load_history(&history_file).is_err() {
            // No previous history
        }

        loop {
            let os_icon = if std::env::consts::OS == "windows" { "\u{e70f} " } else if std::env::consts::OS == "macos" { "\u{f179} " } else { "\u{f17c} " };
            let user = env::var("USER").or_else(|_| env::var("USERNAME")).unwrap_or_else(|_| "user".to_string());
            let host = env::var("COMPUTERNAME").or_else(|_| env::var("HOSTNAME")).unwrap_or_else(|_| "host".to_string());
            let pwd = env::current_dir().unwrap_or_default().display().to_string();
            
            // Rustyline strips ansi colors on Windows if they are inside highlight_prompt.
            // We bypass this by wrapping the ANSI codes directly with \x01 and \x02 in the prompt string.
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
                "\x01{}\x02\x01{}\x02 {} \x01{}\x02\x01{}\x02\u{e0b0} \x01{}\x02\x01{}\x02{}@{} \x01{}\x02\x01{}\x02\u{e0b0} \x01{}\x02\x01{}\x02\u{f07c} {} \x01{}\x02\x01{}\x02\u{e0b0}\x01{}\x02 ",
                bg_blue, fg_white, os_icon,
                bg_gray, fg_blue,
                bg_gray, fg_white, user, host,
                bg_darkblue, fg_gray,
                bg_darkblue, fg_white, pwd,
                reset_bg, fg_darkblue,
                reset
            );

            let readline = rl.readline(&prompt);
            match readline {
                Ok(line) => {
                    let cmd = line.trim();
                    if cmd.is_empty() {
                        continue;
                    }

                    if cmd == "exit" || cmd == "quit" {
                        break;
                    } else if cmd == "clear" || cmd == "cls" {
                        // Clear screen using ANSI escape codes
                        print!("\x1B[2J\x1B[1;1H");
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

                    // Execute command
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
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C
                    println!("^C");
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl-D
                    println!("exit");
                    break;
                }
                Err(err) => {
                    eprintln!("Error: {:?}", err);
                    break;
                }
            }
        }
        
        let _ = rl.save_history(&history_file);
        Ok(())
    }
}
