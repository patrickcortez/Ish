use clap::Parser;
use std::process::ExitCode;

pub mod error;
pub mod core;
pub mod managers;

#[cfg(windows)]
use core::io::platform::enable_virtual_terminal_processing;

#[derive(Parser, Debug)]
#[command(name = "Ish")]
#[command(about = "Ish Programming Language", long_about = None)]
struct Args {
    /// Execute a command and exit
    #[arg(short, long)]
    command: Option<String>,

    /// Path to a script file to execute
    script: Option<String>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    script_args: Vec<String>,
}

fn execute_headless_command(cmd: &str, executor: &mut core::executor::Executor, jobs: &mut managers::job_controller::JobController) -> Result<(), error::IshError> {
    if cmd.is_empty() { return Ok(()); }
    let mut tokenizer = core::tokenizer::Tokenizer::new(cmd);
    let tokens = tokenizer.tokenize()?;
    if tokens.is_empty() { return Ok(()); }
    let mut parser = core::parser::Parser::new(tokens);
    let ast = parser.parse()?;
    let mut linter = core::linter::Linter::new();
    linter.lint(&ast)?;
    
    executor.execute(&ast, jobs)?;
    Ok(())
}

fn main() -> ExitCode {
    #[cfg(windows)]
    let _ = enable_virtual_terminal_processing();
    
    let args = Args::parse();

    if let Some(cmd) = args.command {
        let mut executor = core::executor::Executor::new(vec![]);
        let mut jobs = managers::job_controller::JobController::new();
        if let Err(e) = execute_headless_command(&cmd, &mut executor, &mut jobs) {
            eprintln!("Command execution failed: {}", e);
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    } else if let Some(script_path) = args.script {
        match std::fs::read_to_string(&script_path) {
            Ok(content) => {
                let mut executor = core::executor::Executor::new(args.script_args);
                let mut jobs = managers::job_controller::JobController::new();
                if let Err(e) = execute_headless_command(&content, &mut executor, &mut jobs) {
                    eprintln!("Script execution failed: {}", e);
                    return ExitCode::FAILURE;
                }
            }
            Err(e) => {
                eprintln!("Failed to read script file: {}", e);
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }

    eprintln!("Usage: ish <script.ish> [args...]");
    ExitCode::FAILURE
}
