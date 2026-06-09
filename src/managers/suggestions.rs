use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::sync::{Arc, Mutex};
use rustyline::hint::{Hint, Hinter};
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::validate::{Validator, ValidationResult, ValidationContext};
use rustyline::{Helper, Context};

#[derive(Clone, Debug)]
pub struct IshHint(String);

impl Hint for IshHint {
    fn display(&self) -> &str {
        &self.0
    }
    fn completion(&self) -> Option<&str> {
        Some(&self.0)
    }
}

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

        // 1. Separate prefix and last_token perfectly preserving whitespace
        let last_space_idx = input.rfind(char::is_whitespace);
        let (prefix, last_token) = if let Some(idx) = last_space_idx {
            let split_pos = idx + 1; // Includes trailing space in prefix
            (&input[..split_pos], &input[split_pos..])
        } else {
            ("", input)
        };

        // 2. Determine Context
        #[derive(PartialEq)]
        enum Context {
            Command,
            EnvVar,
            Argument, // Files/Directories
        }

        let mut context;
        
        if prefix.is_empty() {
            context = Context::Command;
        } else {
            let trimmed_prefix = prefix.trim_end();
            let words: Vec<&str> = trimmed_prefix.split_whitespace().collect();
            
            if let Some(&last_word) = words.last() {
                let lw = last_word.to_lowercase();
                if lw == ":" || lw == "then" || lw == "while" || lw == "job" {
                    context = Context::Command;
                } else if words.len() >= 2 {
                    let last_two = format!("{} {}", words[words.len() - 2].to_lowercase(), lw);
                    if last_two == "and then" || last_two == "or else" {
                        context = Context::Command;
                    } else {
                        context = Context::Argument;
                    }
                } else {
                    context = Context::Argument;
                }
            } else {
                context = Context::Command; // Should not happen since prefix wasn't empty
            }
        }

        // Override if starts with '$'
        if last_token.starts_with('$') {
            context = Context::EnvVar;
        }

        // 3. Built-ins check (if Command context AND first word in line starts with ':')
        if context == Context::Command && last_token.starts_with(':') && prefix.trim().is_empty() {
            // Suggest built-ins
            let builtins = [":Toggle", ":Editor"];
            for b in builtins.iter() {
                if b.to_lowercase().starts_with(&last_token.to_lowercase()) {
                    results.push(format!("{}{}", prefix, b));
                }
            }
            if !results.is_empty() { return results; }
        }

        // Also handle built-in flags if we are in argument context for a built-in
        if context == Context::Argument {
            let words: Vec<&str> = prefix.split_whitespace().collect();
            if let Some(&first_word) = words.first() {
                if first_word.eq_ignore_ascii_case(":Toggle") {
                    if words.len() == 1 { // e.g. ":Toggle "
                        let flags = ["--autocd", "--suggestions"];
                        for f in flags.iter() {
                            if f.to_lowercase().starts_with(&last_token.to_lowercase()) {
                                results.push(format!("{}{}", prefix, f));
                            }
                        }
                        return results;
                    } else if words.len() == 2 { // e.g. ":Toggle --autocd "
                        let bools = ["true", "false"];
                        for b in bools.iter() {
                            if b.to_lowercase().starts_with(&last_token.to_lowercase()) {
                                results.push(format!("{}{}", prefix, b));
                            }
                        }
                        return results;
                    }
                } else if first_word.eq_ignore_ascii_case(":Editor") {
                    if words.len() == 1 {
                        let editors = ["vim", "nano", "code", "nvim", "emacs"];
                        for e in editors.iter() {
                            if e.to_lowercase().starts_with(&last_token.to_lowercase()) {
                                results.push(format!("{}{}", prefix, e));
                            }
                        }
                        return results;
                    }
                }
            }
        }

        // 4. Generate context-specific suggestions
        match context {
            Context::Command => {
                // Suggest from history first
                for h in history.iter().rev() {
                    if h.starts_with(input) && !results.contains(h) {
                        results.push(h.clone());
                    }
                }
                
                // Then suggest commands/executables
                if let Ok(execs) = self.path_executables.lock() {
                    for exec in execs.iter() {
                        if exec.starts_with(last_token) {
                            let suggestion = format!("{}{}", prefix, exec);
                            if !results.contains(&suggestion) {
                                results.push(suggestion);
                            }
                        }
                    }
                }
            }
            Context::EnvVar => {
                let var_prefix = &last_token[1..];
                for (key, _) in std::env::vars() {
                    if key.to_lowercase().starts_with(&var_prefix.to_lowercase()) {
                        let suggestion = format!("{}${}", prefix, key);
                        if !results.contains(&suggestion) {
                            results.push(suggestion);
                        }
                    }
                }
            }
            Context::Argument => {
                let files = self.get_file_suggestions(last_token);
                for file in files {
                    let suggestion = format!("{}{}", prefix, file);
                    if !results.contains(&suggestion) {
                        results.push(suggestion);
                    }
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

impl Hinter for SuggestionManager {
    type Hint = IshHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if line.is_empty() || pos < line.len() {
            return None;
        }
        
        // Extract recent history strings
        let mut history_items = Vec::new();
        for i in (0.._ctx.history().len()).rev() {
            if let Some(h) = _ctx.history().get(i, rustyline::history::SearchDirection::Forward).ok().flatten() {
                history_items.push(h.entry.to_string());
            }
        }
        
        let suggestions = self.get_suggestions(line, &history_items);
        
        if let Some(first) = suggestions.first() {
            if first.starts_with(line) {
                return Some(IshHint(first[line.len()..].to_string()));
            }
        }
        None
    }
}

impl Completer for SuggestionManager {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let mut history_items = Vec::new();
        for i in (0.._ctx.history().len()).rev() {
            if let Some(h) = _ctx.history().get(i, rustyline::history::SearchDirection::Forward).ok().flatten() {
                history_items.push(h.entry.to_string());
            }
        }

        let suggestions = self.get_suggestions(&line[..pos], &history_items);
        
        // Find the boundary of the last word
        let start = line[..pos].rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
        
        let mut candidates = Vec::new();
        for sugg in suggestions {
            let word = &sugg[start..];
            candidates.push(Pair {
                display: word.to_string(),
                replacement: word.to_string(),
            });
        }
        Ok((start, candidates))
    }
}

impl Highlighter for SuggestionManager {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        let mut colored = String::new();
        
        let c_cmd = "\x1b[96m"; // Light cyan
        let c_arg = "\x1b[92m"; // Light green
        let c_log = "\x1b[93m"; // Light yellow
        let c_quo = "\x1b[38;5;136m"; // Light brown
        let c_bra = "\x1b[93m"; // Yellow
        let reset = "\x1b[0m";

        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut word_buf = String::new();
        let mut expect_cmd = true; // First word of a command
        
        let flush_word = |word: &mut String, out: &mut String, first: &mut bool| {
            if word.is_empty() { return; }
            let w = word.as_str();
            let lw = w.to_lowercase();
            // check keywords
            let is_kw = lw == ":" || lw == "to" || lw == "from" || lw == "append" || lw == "read" || lw == "merge" || lw == "err" || lw == "doc" || lw == "then" || lw == "while" || lw == "job" || lw == "if" || lw == "else" || lw == "fn" || lw == "and" || lw == "or";
            
            if is_kw {
                out.push_str(c_log);
                out.push_str(w);
                out.push_str(reset);
                if lw == ":" || lw == "then" || lw == "else" || lw == "while" || lw == "job" || lw == "fn" {
                    *first = true; // Next word is a command
                }
            } else if *first {
                out.push_str(c_cmd);
                out.push_str(w);
                out.push_str(reset);
                *first = false;
            } else {
                out.push_str(c_arg);
                out.push_str(w);
                out.push_str(reset);
            }
            word.clear();
        };

        let mut quote_buf = String::new();

        for c in line.chars() {
            if in_quotes {
                quote_buf.push(c);
                if c == quote_char {
                    colored.push_str(c_quo);
                    colored.push_str(&quote_buf);
                    colored.push_str(reset);
                    quote_buf.clear();
                    in_quotes = false;
                }
            } else if c == '"' || c == '\'' {
                flush_word(&mut word_buf, &mut colored, &mut expect_cmd);
                in_quotes = true;
                quote_char = c;
                quote_buf.push(c);
            } else if c == '[' || c == ']' || c == '{' || c == '}' || c == '(' || c == ')' {
                flush_word(&mut word_buf, &mut colored, &mut expect_cmd);
                colored.push_str(c_bra);
                colored.push(c);
                colored.push_str(reset);
            } else if c.is_whitespace() {
                flush_word(&mut word_buf, &mut colored, &mut expect_cmd);
                colored.push(c);
            } else {
                word_buf.push(c);
            }
        }
        
        flush_word(&mut word_buf, &mut colored, &mut expect_cmd);
        
        if in_quotes {
            // Unclosed quote
            colored.push_str(c_quo);
            colored.push_str(&quote_buf);
            colored.push_str(reset);
        }

        std::borrow::Cow::Owned(colored)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> std::borrow::Cow<'b, str> {
        if default {
            return std::borrow::Cow::Borrowed(prompt);
        }
        
        let mut result = String::with_capacity(prompt.len() + 40);
        let mut in_ansi = false;
        for c in prompt.chars() {
            if c == '\x1b' {
                result.push('\x01');
                in_ansi = true;
            }
            result.push(c);
            if in_ansi && c == 'm' {
                result.push('\x02');
                in_ansi = false;
            }
        }
        std::borrow::Cow::Owned(result)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        std::borrow::Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }
}

impl Validator for SuggestionManager {
    fn validate(&self, _ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for SuggestionManager {}
