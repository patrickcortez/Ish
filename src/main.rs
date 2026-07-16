use clap::{Parser, Subcommand};
use std::process::ExitCode;

pub mod error;
pub mod core;
pub mod managers;

#[cfg(windows)]
use core::io::platform::enable_virtual_terminal_processing;


#[derive(Parser, Debug)]
#[command(name = "ish")]
#[command(about = "Ish Programming Language CLI", version = env!("CARGO_PKG_VERSION"), long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run an Ish script
    Run {
        /// Optional path to the script file (or CLI arguments)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Initialize a new Ish project
    Init,
    /// Run an Ish script in debug mode
    Debug {
        /// Optional path to the script file (or CLI arguments)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Print version information
    Version,
    /// Display information about an Ish project
    Info {
        /// Path to the project directory
        path: String,
    },
}

async fn execute_headless_command(content: &str, executor: &mut core::executor::Executor, enforce_entry_point: bool, jobs: &mut managers::job_controller::JobController, expected_namespace: Option<String>, is_entry_file: bool) -> Result<(), error::IshError> {
    let mut lexer = core::tokenizer::Tokenizer::new(content);
    let tokens = lexer.tokenize()?;
    let mut parser = core::parser::Parser::new(tokens);
    let ast = parser.parse()?;
    executor.execute(&ast, enforce_entry_point, jobs, expected_namespace, is_entry_file).await?;
    Ok(())
}

#[async_recursion::async_recursion]
async fn load_all_ish_files(dir: &std::path::Path, project_root: &std::path::Path, primary_namespace: &str, entry_file_path: &std::path::Path, executor: &mut core::executor::Executor, jobs: &mut managers::job_controller::JobController) -> Result<(), error::IshError> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).map_err(|e| error::IshError::ExecutionError(e.to_string()))? {
            let entry = entry.map_err(|e| error::IshError::ExecutionError(e.to_string()))?;
            let path = entry.path();
            if path.is_dir() {
                load_all_ish_files(&path, project_root, primary_namespace, entry_file_path, executor, jobs).await?;
            } else if path.extension().map_or(false, |ext| ext == "ish") {
                let content = std::fs::read_to_string(&path).map_err(|e| error::IshError::ExecutionError(e.to_string()))?;
                
                let is_entry = path == entry_file_path;
                let expected_ns = if let Ok(rel_path) = path.strip_prefix(project_root) {
                    if let Some(parent) = rel_path.parent() {
                        let mut ns = primary_namespace.to_string();
                        for comp in parent.components() {
                            if let std::path::Component::Normal(s) = comp {
                                ns.push('.');
                                ns.push_str(s.to_string_lossy().as_ref());
                            }
                        }
                        Some(ns)
                    } else {
                        Some(primary_namespace.to_string())
                    }
                } else {
                    None
                };

                let _ = execute_headless_command(&content, executor, false, jobs, expected_ns, is_entry).await; // Ignore failures in included files for now, or just let them register classes
            }
        }
    }
    Ok(())
}

fn load_config_or_default() -> core::config::IshConfig {
    let mut config = core::config::IshConfig::default();
    let current_dir = std::env::current_dir().unwrap_or_default();
    
    // Look for any .ic file in the current directory
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "ic" {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(parsed) = core::config::parse_config_str(&content) {
                            config = parsed;
                        } else {
                            eprintln!("Warning: Failed to parse configuration file '{}'", entry.path().display());
                        }
                    }
                    break;
                }
            }
        }
    }
    config
}

#[tokio::main]
async fn main() -> ExitCode {
    #[cfg(windows)]
    let _ = enable_virtual_terminal_processing();
    
    let cli = Cli::parse();
    
    let config = load_config_or_default();

    match cli.command {
        Commands::Init => {
            let current_dir = std::env::current_dir().unwrap_or_default();
            let config_path = current_dir.join("project.ic");
            if !config_path.exists() {
                let default_config = "[Project]
Name: \"MyProject\";
Version: \"1.0.0\";
Author: \"Unknown\";
Readme: \"\";
Entry-File: \"Main.ish\";
Entry-Class: \"Program\";
Entry-Method: \"Main\";
With-Args: true;
Verbose: false;
DotEnv: false;

[Configuration]
Max-Memory-Threshold: 100;
Max-variables: 1000;
Array-Size-Limit: 5000;
Map-Size-Limit: 5000;
";
                if let Err(e) = std::fs::write(&config_path, default_config) {
                    eprintln!("Failed to create project.ic: {}", e);
                    return ExitCode::FAILURE;
                }
                println!("Initialized new Ish project.");
            } else {
                println!("Project already initialized.");
            }
            ExitCode::SUCCESS
        }
        Commands::Run { mut args } => {
            let mut script_file = None;
            if !args.is_empty() && args[0].ends_with(".ish") {
                script_file = Some(args.remove(0));
            }
            
            let mut executor = core::executor::Executor::new(args.clone(), config.clone());
            let mut jobs = managers::job_controller::JobController::new();
            
            let current_dir = std::env::current_dir().unwrap_or_default();
            let entry_path = current_dir.join(&config.project.entry_file);
            let primary_namespace = &config.project.name;
            let mut included_any = false;
            
            for inc in &config.project.include_dirs {
                let clean_inc = inc.replace("**", "").replace("*", "");
                let inc_path = current_dir.join(clean_inc.trim_end_matches('/'));
                if inc_path.exists() {
                    if let Err(e) = load_all_ish_files(&inc_path, &current_dir, primary_namespace, &entry_path, &mut executor, &mut jobs).await {
                        eprintln!("Failed to load include path {}: {}", inc, e);
                        return ExitCode::FAILURE;
                    }
                    included_any = true;
                }
            }
            
            if !included_any {
                if let Err(e) = load_all_ish_files(&current_dir, &current_dir, primary_namespace, &entry_path, &mut executor, &mut jobs).await {
                     eprintln!("Failed to load project files: {}", e);
                     return ExitCode::FAILURE;
                }
            }
            
            if let Some(script) = script_file {
                match std::fs::read_to_string(&script) {
                    Ok(content) => {
                        let is_entry = std::path::Path::new(&script).canonicalize().unwrap_or_default() == entry_path.canonicalize().unwrap_or_default();
                        let expected_ns = Some(primary_namespace.to_string());
                        if let Err(e) = execute_headless_command(&content, &mut executor, true, &mut jobs, expected_ns, is_entry).await {
                            eprintln!("Script execution failed: {}", e);
                            return ExitCode::FAILURE;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read script file: {}", e);
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                let entry_path = current_dir.join(&config.project.entry_file);
                if entry_path.exists() {
                    let content = std::fs::read_to_string(&entry_path).unwrap_or_default();
                    let expected_ns = Some(primary_namespace.to_string());
                    if let Err(e) = execute_headless_command(&content, &mut executor, true, &mut jobs, expected_ns, true).await {
                        eprintln!("Entry-File execution failed: {}", e);
                        return ExitCode::FAILURE;
                    }
                } else {
                    eprintln!("Entry-File '{}' not found in project root", config.project.entry_file);
                    return ExitCode::FAILURE;
                }
            }
            ExitCode::SUCCESS
        }
        Commands::Debug { mut args } => {
            println!("--- DEBUG MODE ---");
            println!("Memory Threshold: {} MB", config.interpreter.max_memory_threshold_mb);
            println!("Max Variables: {}", config.interpreter.max_variables);
            println!("Array Size Limit: {}", config.interpreter.array_size_limit);
            println!("Map Size Limit: {}", config.interpreter.map_size_limit);
            println!("------------------");
            
            let mut script_file = None;
            if !args.is_empty() && args[0].ends_with(".ish") {
                script_file = Some(args.remove(0));
            }
            let mut executor = core::executor::Executor::new(args.clone(), config.clone());
            let mut jobs = managers::job_controller::JobController::new();
            
            if let Some(script) = script_file {
                match std::fs::read_to_string(&script) {
                    Ok(content) => {
                        let primary_namespace = config.project.name.clone();
                        let current_dir = std::env::current_dir().unwrap_or_default();
                        let entry_path = current_dir.join(&config.project.entry_file);
                        let is_entry = std::path::Path::new(&script).canonicalize().unwrap_or_default() == entry_path.canonicalize().unwrap_or_default();
                        let expected_ns = Some(primary_namespace);
                        if let Err(e) = execute_headless_command(&content, &mut executor, true, &mut jobs, expected_ns, is_entry).await {
                            eprintln!("Script execution failed: {}", e);
                            return ExitCode::FAILURE;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read script file: {}", e);
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                eprintln!("Debug mode requires a script argument currently.");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Commands::Version => {
            println!("Ish version {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Commands::Info { path } => {
            let target_dir = std::path::Path::new(&path);
            if !target_dir.exists() || !target_dir.is_dir() {
                eprintln!("Error: Path '{}' does not exist or is not a directory.", path);
                return ExitCode::FAILURE;
            }
            
            let mut found_config = false;
            let mut info_config = core::config::IshConfig::default();
            
            if let Ok(entries) = std::fs::read_dir(target_dir) {
                for entry in entries.flatten() {
                    if let Some(ext) = entry.path().extension() {
                        if ext == "ic" {
                            if let Ok(config) = core::config::parse_config_file(entry.path()) {
                                info_config = config;
                                found_config = true;
                            }
                            break;
                        }
                    }
                }
            }
            
            if !found_config {
                eprintln!("Error: No .ic configuration file found in '{}'.", path);
                return ExitCode::FAILURE;
            }
            
            println!("Name: \"{}\";", info_config.project.name);
            println!("Version: \"{}\";", info_config.project.version);
            println!("Author: \"{}\";", info_config.project.author);
            
            if let Some(readme_path) = &info_config.project.readme {
                if !readme_path.is_empty() {
                    let full_readme_path = target_dir.join(readme_path);
                    if full_readme_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&full_readme_path) {
                            println!("{}", content);
                        }
                    }
                }
            }
            
            ExitCode::SUCCESS
        }
    }
}
