use clap::Parser;
use std::process::ExitCode;

pub mod error;
pub mod core;
pub mod managers;
pub mod ui;

#[derive(Parser, Debug)]
#[command(name = "ish")]
#[command(about = "Intelli-Shell: A cross-platform system shell with built-in intellisense.", long_about = None)]
struct Args {
    /// Execute a command and exit
    #[arg(short, long)]
    command: Option<String>,

    /// Path to a script file to execute
    script: Option<String>,
}

fn execute_headless_command(cmd: &str) -> Result<(), error::IshError> {
    let mut tokenizer = core::tokenizer::Tokenizer::new(cmd);
    let tokens = tokenizer.tokenize()?;
    let mut parser = core::parser::Parser::new(tokens);
    let ast = parser.parse()?;
    let linter = core::linter::Linter::new();
    linter.lint(&ast)?;
    let mut jobs = managers::job_controller::JobController::new();
    let mut executor = core::executor::Executor::new();
    let (_, out) = executor.execute(&ast, &mut jobs)?;
    if !out.is_empty() {
        print!("{}", out);
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Initialize configuration
    let config_manager = match managers::config::ConfigManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            return ExitCode::FAILURE;
        }
    };

    if let Some(cmd) = args.command {
        if let Err(e) = execute_headless_command(&cmd) {
            eprintln!("Command execution failed: {}", e);
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    } else if let Some(script_path) = args.script {
        match std::fs::read_to_string(&script_path) {
            Ok(content) => {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        if let Err(e) = execute_headless_command(trimmed) {
                            eprintln!("Script execution failed on line '{}': {}", trimmed, e);
                            return ExitCode::FAILURE;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read script file: {}", e);
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }

    // Launch TUI (Phase 2)
    println!("Launching Ish TUI...");
    let mut app = ui::App::new();
    if let Err(e) = app.run(&config_manager) {
        eprintln!("Error running TUI: {}", e);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
