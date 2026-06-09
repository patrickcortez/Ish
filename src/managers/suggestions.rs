use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::sync::{Arc, Mutex};

pub struct SuggestionManager {
    path_executables: Arc<Mutex<Vec<String>>>,
}

impl SuggestionManager {
    pub fn new() -> Self {
        let path_executables = Arc::new(Mutex::new(Vec::new()));
        
        // Load PATH in background
        let executables_clone = Arc::clone(&path_executables);
        thread::spawn(move || {
            let mut execs = Vec::new();
            if let Ok(paths) = env::var("PATH") {
                let split_char = if cfg!(target_os = "windows") { ';' } else { ':' };
                for path in paths.split(split_char) {
                    if let Ok(entries) = fs::read_dir(path) {
                        for entry in entries.flatten() {
                            if let Ok(file_type) = entry.file_type() {
                                if file_type.is_file() {
                                    if let Ok(name) = entry.file_name().into_string() {
                                        // On Windows, strip .exe / .bat for cleaner suggestions
                                        let clean_name = if cfg!(target_os = "windows") {
                                            if name.to_lowercase().ends_with(".exe") || name.to_lowercase().ends_with(".bat") || name.to_lowercase().ends_with(".cmd") {
                                                name[..name.len() - 4].to_string()
                                            } else {
                                                name
                                            }
                                        } else {
                                            name
                                        };
                                        execs.push(clean_name);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if cfg!(target_os = "windows") {
                if let Ok(output) = std::process::Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg("Get-Command -CommandType Cmdlet,Alias,Function | Select-Object -ExpandProperty Name")
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            execs.push(trimmed.to_string());
                        }
                    }
                }
            }

            execs.sort();
            execs.dedup();
            if let Ok(mut locked) = executables_clone.lock() {
                *locked = execs;
            }
        });

        Self {
            path_executables,
        }
    }

    pub fn get_suggestions(&self, input: &str, history: &[String]) -> Vec<String> {
        let mut results = Vec::new();
        let trimmed = input.trim_start();
        if trimmed.is_empty() {
            return results;
        }

        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        let ends_with_space = trimmed.ends_with(char::is_whitespace);

        // Built-in command contextual suggestions
        if trimmed.starts_with(':') {
            if tokens.len() == 1 && !ends_with_space {
                let builtins = [":Color", ":Toggle", ":Editor"];
                for b in builtins.iter() {
                    if b.to_lowercase().starts_with(&tokens[0].to_lowercase()) {
                        results.push(b.to_string());
                    }
                }
                if !results.is_empty() { return results; }
            } else if tokens[0].eq_ignore_ascii_case(":Color") {
                if (tokens.len() == 1 && ends_with_space) || (tokens.len() == 2 && !ends_with_space) {
                    let partial = if tokens.len() == 2 { tokens[1] } else { "" };
                    let targets = ["--inputbox", "--output", "--banner"];
                    for t in targets.iter() {
                        if t.to_lowercase().starts_with(&partial.to_lowercase()) {
                            results.push(t.to_string());
                        }
                    }
                    return results;
                } else if (tokens.len() == 2 && ends_with_space) || (tokens.len() == 3 && !ends_with_space) {
                    let partial = if tokens.len() == 3 { tokens[2] } else { "" };
                    let colors = ["Red", "Green", "Blue", "Cyan", "Magenta", "Yellow", "White", "Black", "Gray", "DarkGray", "LightRed", "LightGreen", "LightBlue", "LightCyan", "LightMagenta", "LightYellow"];
                    for c in colors.iter() {
                        if c.to_lowercase().starts_with(&partial.to_lowercase()) {
                            results.push(c.to_string());
                        }
                    }
                    return results;
                }
            } else if tokens[0].eq_ignore_ascii_case(":Toggle") {
                if (tokens.len() == 1 && ends_with_space) || (tokens.len() == 2 && !ends_with_space) {
                    let partial = if tokens.len() == 2 { tokens[1] } else { "" };
                    let flags = ["--autocd", "--suggestions"];
                    for f in flags.iter() {
                        if f.to_lowercase().starts_with(&partial.to_lowercase()) {
                            results.push(f.to_string());
                        }
                    }
                    return results;
                } else if (tokens.len() == 2 && ends_with_space) || (tokens.len() == 3 && !ends_with_space) {
                    let partial = if tokens.len() == 3 { tokens[2] } else { "" };
                    let bools = ["true", "false"];
                    for b in bools.iter() {
                        if b.to_lowercase().starts_with(&partial.to_lowercase()) {
                            results.push(b.to_string());
                        }
                    }
                    return results;
                }
            } else if tokens[0].eq_ignore_ascii_case(":Editor") {
                if (tokens.len() == 1 && ends_with_space) || (tokens.len() == 2 && !ends_with_space) {
                    let partial = if tokens.len() == 2 { tokens[1] } else { "" };
                    let editors = ["vim", "nano", "code", "nvim", "emacs"];
                    for e in editors.iter() {
                        if e.to_lowercase().starts_with(&partial.to_lowercase()) {
                            results.push(e.to_string());
                        }
                    }
                    if !results.is_empty() { return results; }
                }
            }
        }

        // 1. If it has a space, suggest files/directories from the last token
        if ends_with_space || tokens.len() > 1 {
            let last_token = if ends_with_space {
                ""
            } else {
                tokens.last().unwrap()
            };
            results.extend(self.get_file_suggestions(last_token));
            return results; // Only suggest files/args after command is typed
        }

        // 2. Suggest from History (Commands)
        for h in history.iter().rev() {
            if h.starts_with(trimmed) && !results.contains(h) {
                results.push(h.clone());
            }
        }

        // 3. Suggest from PATH executables
        if let Ok(execs) = self.path_executables.lock() {
            for exec in execs.iter() {
                if exec.starts_with(trimmed) && !results.contains(exec) {
                    results.push(exec.clone());
                }
            }
        }

        results
    }

    fn get_file_suggestions(&self, partial: &str) -> Vec<String> {
        let mut results = Vec::new();
        let path = Path::new(partial);
        
        let (dir_to_search, prefix) = if partial.is_empty() {
            (Path::new("."), "")
        } else if path.is_dir() && (partial.ends_with('/') || partial.ends_with('\\')) {
            (path, "")
        } else {
            let parent = path.parent().unwrap_or_else(|| Path::new(""));
            let search_dir = if parent.as_os_str().is_empty() { Path::new(".") } else { parent };
            let file_prefix = path.file_name().unwrap_or_default().to_str().unwrap_or("");
            (search_dir, file_prefix)
        };

        if let Ok(entries) = fs::read_dir(dir_to_search) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with(prefix) {
                        // Reconstruct full path
                        let mut suggestion = PathBuf::from(dir_to_search);
                        if dir_to_search == Path::new(".") {
                            suggestion = PathBuf::new();
                        }
                        suggestion.push(&name);
                        
                        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                        let mut sugg_str = suggestion.to_string_lossy().to_string();
                        if is_dir {
                            sugg_str.push(if cfg!(target_os = "windows") { '\\' } else { '/' });
                        }
                        results.push(sugg_str);
                    }
                }
            }
        }
        results
    }
}
