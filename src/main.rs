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

fn execute_headless_command(cmd: &str, executor: &mut core::executor::Executor, jobs: &mut managers::job_controller::JobController) -> Result<(), error::IshError> {
    if cmd.is_empty() { return Ok(()); }
    let mut tokenizer = core::tokenizer::Tokenizer::new(cmd);
    let tokens = tokenizer.tokenize()?;
    if tokens.is_empty() { return Ok(()); }
    
    let mut parser = core::parser::Parser::new(tokens);
    let ast = parser.parse()?;
    let linter = core::linter::Linter::new();
    linter.lint(&ast)?;
    
    let (_, out) = executor.execute(&ast, jobs, &mut |child, merge_err| {
        let mut child_out = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            let _ = std::io::Read::read_to_string(&mut stdout, &mut child_out);
        }
        if merge_err {
            if let Some(mut stderr) = child.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut stderr, &mut child_out);
            }
        } else {
            if let Some(mut stderr) = child.stderr.take() {
                let mut err_out = String::new();
                let _ = std::io::Read::read_to_string(&mut stderr, &mut err_out);
                if !err_out.is_empty() {
                    child_out.push_str(&err_out);
                }
            }
        }
        Ok(child_out)
    })?;
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
                let mut executor = core::executor::Executor::new(vec![]); // Will pass real args later
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

    // Launch TUI (Phase 2)
    println!("Launching Ish TUI...");
    let mut app = ui::App::new();
    if let Err(e) = app.run(&config_manager) {
        eprintln!("Error running TUI: {}", e);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
