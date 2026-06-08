use clap::Parser;
use std::process::ExitCode;

pub mod error;
pub mod managers;

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
        // Headless mode: run a single command
        println!("Headless mode (command): {}", cmd);
        // TODO: Pass to the executor
        return ExitCode::SUCCESS;
    } else if let Some(script_path) = args.script {
        // Headless mode: run a script
        println!("Headless mode (script): {}", script_path);
        // TODO: Read script and pass to the parser/executor
        return ExitCode::SUCCESS;
    }

    // Launch TUI (Phase 2)
    println!("Launching Ish TUI...");
    // TODO: Init Ratatui and event loop

    ExitCode::SUCCESS
}
