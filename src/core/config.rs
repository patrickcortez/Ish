use crate::error::IshError;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub author: String,
    pub readme: Option<String>,
    pub entry_file: String,
    pub entry_class: String,
    pub entry_method: String,
    pub with_args: bool,
    pub include_dirs: Vec<String>,
    pub verbose: bool,
    pub dotenv: bool,
}

#[derive(Debug, Clone)]
pub struct InterpreterConfig {
    pub array_size_limit: usize,
    pub list_size_limit: usize,
    pub map_size_limit: usize,
    pub string_length_limit: usize,
    pub max_variables: usize,
    pub max_memory_threshold_mb: usize,
}

#[derive(Debug, Clone)]
pub struct IshConfig {
    pub project: ProjectConfig,
    pub interpreter: InterpreterConfig,
}

impl Default for IshConfig {
    fn default() -> Self {
        Self {
            project: ProjectConfig {
                name: "Ish-Project".to_string(),
                version: "1.0.0".to_string(),
                author: "Unknown".to_string(),
                readme: None,
                entry_file: "Main.ish".to_string(),
                entry_class: "Program".to_string(),
                entry_method: "Main".to_string(),
                with_args: true,
                include_dirs: vec![],
                verbose: false,
                dotenv: false,
            },
            interpreter: InterpreterConfig {
                array_size_limit: 1024,
                list_size_limit: 1024,
                map_size_limit: 1024,
                string_length_limit: 1024,
                max_variables: 1024,
                max_memory_threshold_mb: 128,
            },
        }
    }
}

pub fn parse_config_file<P: AsRef<Path>>(path: P) -> Result<IshConfig, IshError> {
    let content = fs::read_to_string(path).map_err(|e| IshError::ParseError(format!("Failed to read config file: {}", e)))?;
    parse_config_str(&content)
}

pub fn parse_config_str(content: &str) -> Result<IshConfig, IshError> {
    let mut config = IshConfig::default();
    let mut current_section = "Project".to_string();
    let mut current_array_key: Option<String> = None;

    for (_line_num, mut line) in content.lines().enumerate() {
        if let Some(idx) = line.find("//") {
            line = &line[..idx];
        }
        line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            current_array_key = None;
            continue;
        }

        if line.starts_with('-') {
            if let Some(ref key) = current_array_key {
                let mut val = line[1..].trim();
                if val.ends_with(';') {
                    val = &val[..val.len() - 1].trim();
                }
                if val.starts_with('"') && val.ends_with('"') {
                    val = &val[1..val.len() - 1];
                }
                
                if current_section == "Project" && key == "Include" {
                    config.project.include_dirs.push(val.to_string());
                }
            }
            continue;
        }

        if let Some(colon_idx) = line.find(':') {
            let key = line[..colon_idx].trim().to_string();
            let mut val = line[colon_idx + 1..].trim();
            
            if val.ends_with(';') {
                val = &val[..val.len() - 1].trim();
            }

            if val.is_empty() {
                current_array_key = Some(key);
                continue;
            } else {
                current_array_key = None;
            }

            let mut parsed_str = val;
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                parsed_str = &val[1..val.len() - 1];
            }

            match current_section.as_str() {
                "Project" => {
                    match key.as_str() {
                        "Name" => config.project.name = parsed_str.to_string(),
                        "Version" => config.project.version = parsed_str.to_string(),
                        "Author" => config.project.author = parsed_str.to_string(),
                        "Readme" => config.project.readme = Some(parsed_str.to_string()),
                        "Entry-File" => config.project.entry_file = parsed_str.to_string(),
                        "Entry-Class" => config.project.entry_class = parsed_str.to_string(),
                        "Entry-Method" => config.project.entry_method = parsed_str.to_string(),
                        "With-Args" => config.project.with_args = parsed_str.parse().unwrap_or(true),
                        "Verbose" => config.project.verbose = parsed_str.parse().unwrap_or(false),
                        "DotEnv" => config.project.dotenv = parsed_str.parse().unwrap_or(false),
                        _ => {}
                    }
                }
                "Configuration" => {
                    if let Ok(num) = parsed_str.parse::<usize>() {
                        match key.as_str() {
                            "Array-Size-Limit" => config.interpreter.array_size_limit = num,
                            "List-Size-Limit" => config.interpreter.list_size_limit = num,
                            "Map-Size-Limit" => config.interpreter.map_size_limit = num,
                            "String-Length-Limit" => config.interpreter.string_length_limit = num,
                            "Max-variables" => config.interpreter.max_variables = num,
                            "Max-Memory-Threshold" => config.interpreter.max_memory_threshold_mb = num,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(config)
}
